use std::sync::Arc;
use tokio::sync::Mutex;
use tokio::time::{interval, Duration};

use crate::core::manager::inhibitors::InhibitorSource;
use crate::core::manager::{
    inhibitors::{incr_active_inhibitor, decr_active_inhibitor},
    Manager
};
use crate::media_bridge;
use eventline::{event_info_scoped, event_debug_scoped, event_error_scoped};

const POLL_INTERVAL_MS: u64 = 1000;
const BRIDGE_CHECK_INTERVAL_SECS: u64 = 4;

pub fn is_bridge_available() -> bool {
    media_bridge::is_available()
}

pub async fn spawn_browser_bridge_detector(manager: Arc<Mutex<Manager>>) {
    let manager_clone = Arc::clone(&manager);
    
    spawn_browser_media_monitor(Arc::clone(&manager_clone)).await;
    
    tokio::spawn(async move {
        let mut check_interval = tokio::time::interval(Duration::from_secs(BRIDGE_CHECK_INTERVAL_SECS));
        let mut was_available = false;
        
        // Check initial state
        let (monitor_enabled, bridge_available) = {
            let mgr = manager_clone.lock().await;
            let enabled = mgr.state.cfg.as_ref().map(|c| c.monitor_media).unwrap_or(true);
            (enabled, is_bridge_available())
        };
        
        if monitor_enabled && bridge_available {
            event_info_scoped!("Stasis", "Browser bridge detected at startup").await;
            activate_browser_monitor(Arc::clone(&manager_clone)).await;
            was_available = true;
        } else if bridge_available {
            event_info_scoped!("Stasis", "Browser bridge available but media monitoring disabled").await;
            was_available = true;
        } else {
            event_info_scoped!("Stasis", "Browser bridge not detected, will check periodically").await;
        }
        
        // Get shutdown flag
        let shutdown = {
            let mgr = manager_clone.lock().await;
            mgr.state.shutdown_flag.clone()
        };
        
        loop {
            tokio::select! {
                _ = shutdown.notified() => {
                    event_info_scoped!("Stasis", "Browser bridge detector shutting down...").await;
                    break;
                }
                
                _ = check_interval.tick() => {
                    let monitor_enabled = {
                        let mgr = manager_clone.lock().await;
                        mgr.state.cfg.as_ref().map(|c| c.monitor_media).unwrap_or(true)
                    };
                    
                    let is_available = is_bridge_available();
                    
                    if monitor_enabled && is_available && !was_available {
                        event_info_scoped!("Stasis", "Browser bridge now available, activating").await;
                        activate_browser_monitor(Arc::clone(&manager_clone)).await;
                        was_available = true;
                    } else if (!monitor_enabled || !is_available) && was_available {
                        let reason = if !monitor_enabled { "monitoring disabled" } else { "bridge unavailable" };
                        event_info_scoped!("Stasis", "Browser bridge deactivating ({})", reason).await;
                        deactivate_browser_monitor(Arc::clone(&manager_clone)).await;
                        was_available = false;
                    }
                }
            }
        }
    });
}

/// Activate bridge monitoring - check if we need to clear MPRIS inhibitor
async fn activate_browser_monitor(manager: Arc<Mutex<Manager>>) {
    event_info_scoped!("Media Bridge", "Taking over Firefox monitoring from MPRIS").await;
    
    // Check if there are non-Firefox players still playing
    let (ignore_remote, blacklist, mpris_active) = {
        let mgr = manager.lock().await;
        let ignore = mgr.state.cfg
            .as_ref()
            .map(|c| c.ignore_remote_media)
            .unwrap_or(false);
        let blacklist = mgr.state.cfg
            .as_ref()
            .map(|c| c.media_blacklist.clone())
            .unwrap_or_default();
        (ignore, blacklist, mgr.state.media.mpris_media_playing)
    };
    
    // Check for non-Firefox players (skip_firefox=true means only check non-Firefox)
    let non_firefox_playing = crate::core::services::media::check_media_playing(
        ignore_remote,
        &blacklist,
        true  // skip_firefox=true, so we only detect non-Firefox players
    ).await;
    
    let mut mgr = manager.lock().await;
    
    // Only clear MPRIS inhibitor if ONLY Firefox was playing (no other players)
    if mpris_active && !non_firefox_playing {
        event_info_scoped!("Media Bridge", "Only Firefox was playing, clearing MPRIS inhibitor").await;
        decr_active_inhibitor(&mut mgr, InhibitorSource::Media).await;
        mgr.state.media.mpris_media_playing = false;
    } else if mpris_active && non_firefox_playing {
        event_info_scoped!("Media Bridge", "Non-Firefox players still active, keeping MPRIS inhibitor").await;
        // MPRIS will continue monitoring non-Firefox players
    }
    
    // Activate bridge - this signals MPRIS to skip Firefox checks
    mgr.state.media.media_bridge_active = true;
    
    update_combined_state(&mut mgr);
    
    event_debug_scoped!("Media Bridge", "Bridge activated, MPRIS will skip Firefox").await;
}

/// Deactivate bridge and hand back to MPRIS
async fn deactivate_browser_monitor(manager: Arc<Mutex<Manager>>) {
    event_info_scoped!("Media Bridge", "Handing Firefox monitoring back to MPRIS").await;
    
    // First, clear all bridge inhibitors
    {
        let mut mgr = manager.lock().await;
        let tab_count = mgr.state.media.browser_playing_tab_count;
        
        if tab_count > 0 {
            event_debug_scoped!("Media Bridge", "Clearing {} browser tab inhibitors", tab_count).await;
            for _ in 0..tab_count {
                decr_active_inhibitor(&mut mgr, InhibitorSource::Media).await;
            }
        }
        
        mgr.state.media.browser_playing_tab_count = 0;
        mgr.state.media.browser_media_playing = false;
        
        // Deactivate bridge - this allows MPRIS to resume checking Firefox
        mgr.state.media.media_bridge_active = false;
        
        update_combined_state(&mut mgr);
    }
    
    // Now let MPRIS check Firefox and set its own state
    event_debug_scoped!("Media Bridge", "Bridge deactivated, triggering MPRIS recheck").await;
    trigger_mpris_recheck(Arc::clone(&manager)).await;
}

/// Trigger a synchronous recheck of MPRIS media state
async fn trigger_mpris_recheck(manager: Arc<Mutex<Manager>>) {
    {
        let mgr = manager.lock().await;
        if !mgr.state.cfg.as_ref().map(|c| c.monitor_media).unwrap_or(true) {
            event_debug_scoped!("MPRIS", "Recheck skipped: media monitoring disabled").await;
            return;
        }
    }
    let (ignore_remote, media_blacklist, bridge_active) = {
        let mgr = manager.lock().await;
        let ignore = mgr.state.cfg
            .as_ref()
            .map(|c| c.ignore_remote_media)
            .unwrap_or(false);
        let blacklist = mgr.state.cfg
            .as_ref()
            .map(|c| c.media_blacklist.clone())
            .unwrap_or_default();
        (ignore, blacklist, mgr.state.media.media_bridge_active)
    };
    
    // Bridge should be inactive at this point
    if bridge_active {
        event_error_scoped!("MPRIS", "Bridge still active during recheck - this shouldn't happen").await;
        return;
    }
    
    let playing = crate::core::services::media::check_media_playing(
        ignore_remote,
        &media_blacklist,
        bridge_active  // Should be false, so Firefox will be checked
    ).await;
    
    let mut mgr = manager.lock().await;
    
    if playing && !mgr.state.media.mpris_media_playing {
        event_info_scoped!("MPRIS", "Recheck: Media playing detected (including Firefox)").await;
        incr_active_inhibitor(&mut mgr, InhibitorSource::Media).await;
        mgr.state.media.mpris_media_playing = true;
    } else if !playing && mgr.state.media.mpris_media_playing {
        event_info_scoped!("MPRIS", "Recheck: No media playing").await;
        decr_active_inhibitor(&mut mgr, InhibitorSource::Media).await;
        mgr.state.media.mpris_media_playing = false;
    }
    
    update_combined_state(&mut mgr);
}

async fn spawn_browser_media_monitor(manager: Arc<Mutex<Manager>>) {
    // Don't check if already active - just spawn fresh
    {
        let mut mgr = manager.lock().await;
        mgr.state.media.browser_playing_tab_count = 0;
        mgr.state.media.browser_media_playing = false;
    }

    tokio::spawn(async move {
        let mut poll_interval = interval(Duration::from_millis(POLL_INTERVAL_MS));
        let mut last_state: Option<media_bridge::BrowserMediaState> = None;
        let mut connected = false;
        let mut was_monitoring = false;
       
        event_info_scoped!("Media Bridge", "Browser media monitor started").await;
        
        let shutdown = {
            let mgr = manager.lock().await;
            mgr.state.shutdown_flag.clone()
        };
        
        loop {
            tokio::select! {
                _ = shutdown.notified() => {
                    event_info_scoped!("Media Bridge", "Browser media monitor shutting down...").await;
                    break;
                }
                
                _ = poll_interval.tick() => {
                    let (monitor_enabled, bridge_active) = {
                        let mgr = manager.lock().await;
                        let enabled = mgr.state.cfg.as_ref().map(|c| c.monitor_media).unwrap_or(true);
                        (enabled, mgr.state.media.media_bridge_active)
                    };
                    
                    if !monitor_enabled && was_monitoring {
                        event_info_scoped!("Media Bridge", "Media monitoring disabled, cleaning up state").await;
                        
                        let mut mgr = manager.lock().await;
                        let prev_tab_count = mgr.state.media.browser_playing_tab_count;
                        
                        if prev_tab_count > 0 {
                            for _ in 0..prev_tab_count {
                                decr_active_inhibitor(&mut mgr, InhibitorSource::Media).await;
                            }
                        }
                        
                        mgr.state.media.browser_playing_tab_count = 0;
                        mgr.state.media.browser_media_playing = false;
                        update_combined_state(&mut mgr);
                        
                        last_state = None;
                        connected = false;
                        was_monitoring = false;
                        continue;
                    }
                    
                    // Skip processing if monitoring is disabled or bridge not active
                    if !monitor_enabled || !bridge_active {
                        was_monitoring = false;
                        continue;
                    }
                    
                    was_monitoring = true;
                    
                    // Normal bridge monitoring
                    match media_bridge::query_status() {
                        Ok(state) => {
                            if !connected {
                                event_info_scoped!("Stasis", "Connected to media bridge").await;
                                connected = true;
                            }
                            
                            let state_changed = last_state
                                .as_ref()
                                .map(|last| state.has_changed_from(last))
                                .unwrap_or(true);
                            
                            if state_changed {
                                update_manager_state(manager.clone(), &state, last_state.as_ref()).await;
                                log_state_change(&state, last_state.as_ref());
                            }
                            
                            last_state = Some(state);
                        }
                        Err(_e) => {
                            if connected {
                                event_error_scoped!("Stasis", "Lost connection to media bridge").await;
                                connected = false;
                                
                                // Clear bridge state before handing off to MPRIS
                                let mut mgr = manager.lock().await;
                                let tab_count = mgr.state.media.browser_playing_tab_count;
                                
                                if tab_count > 0 {
                                    event_debug_scoped!("Media Bridge", "Clearing {} inhibitors due to disconnection", tab_count).await;
                                    for _ in 0..tab_count {
                                        decr_active_inhibitor(&mut mgr, InhibitorSource::Media).await;
                                    }
                                }
                                
                                mgr.state.media.browser_playing_tab_count = 0;
                                mgr.state.media.browser_media_playing = false;
                                update_combined_state(&mut mgr);
                                
                                last_state = None;
                                
                                // Drop the lock before triggering recheck
                                drop(mgr);
                                
                                // Trigger MPRIS recheck since bridge is now unavailable
                                event_info_scoped!("Stasis", "Bridge disconnected, rechecking MPRIS media state").await;
                                trigger_mpris_recheck(Arc::clone(&manager)).await;
                            }
                        }
                    }
                }
            }
        }
       
        event_info_scoped!("Media Bridge", "Browser media monitor task exited").await;
    });
}

async fn update_manager_state(
    manager: Arc<Mutex<Manager>>,
    new_state: &media_bridge::BrowserMediaState,
    old_state: Option<&media_bridge::BrowserMediaState>,
) {
    let mut mgr = manager.lock().await;

    let prev_tab_count = old_state
        .map(|s| s.playing_tab_count())
        .unwrap_or(mgr.state.media.browser_playing_tab_count);
    
    let new_tab_count = new_state.playing_tab_count();
    let delta = new_tab_count as i32 - prev_tab_count as i32;

    if delta != 0 {
        event_debug_scoped!("Media Bridge", "Browser tab count change: {} → {} (delta: {})", prev_tab_count, new_tab_count, delta).await;
    }

    mgr.state.media.browser_playing_tab_count = new_tab_count;

    if delta > 0 {
        for _ in 0..delta {
            incr_active_inhibitor(&mut mgr, InhibitorSource::Media).await;
        }
    } else if delta < 0 {
        for _ in 0..delta.abs() {
            decr_active_inhibitor(&mut mgr, InhibitorSource::Media).await;
        }
    }

    mgr.state.media.browser_media_playing = new_tab_count > 0; 
    update_combined_state(&mut mgr);
}

fn update_combined_state(mgr: &mut Manager) {
    let combined = mgr.state.media.mpris_media_playing || mgr.state.media.browser_media_playing;
    mgr.state.media.media_playing = combined;
    mgr.state.media.media_blocking = combined;
}

fn log_state_change(
    new_state: &media_bridge::BrowserMediaState,
    old_state: Option<&media_bridge::BrowserMediaState>,
) {
    let playing_tab_count = new_state.playing_tab_count();
    let tab_count = new_state.tab_count;
    let playing_tabs = new_state.playing_tabs.clone(); // clone Vec<String> or whatever type

    if new_state.playing {
        tokio::spawn(event_debug_scoped!(
            "Media Bridge",
            "Browser media active: {}/{} tabs playing (IDs: {:?})",
            playing_tab_count,
            tab_count,
            playing_tabs
        ));
    } else if tab_count > 0 {
        tokio::spawn(event_debug_scoped!(
            "Media Bridge",
            "Browser media stopped ({} tabs have media but none are playing)",
            tab_count
        ));
    } else if old_state.is_some() {
        tokio::spawn(event_debug_scoped!(
            "Media Bridge",
            "Browser media stopped (no tabs with media)"
        ));
    }
}

