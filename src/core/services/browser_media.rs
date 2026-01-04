use std::sync::Arc;
use tokio::sync::Mutex;
use tokio::time::{interval, Duration};

use crate::core::manager::inhibitors::InhibitorSource;
use crate::core::manager::{
    inhibitors::{incr_active_inhibitor, decr_active_inhibitor},
    Manager
};
use crate::{media_bridge, sdebug, serror, sinfo};

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
            sinfo!("Stasis", "Browser bridge detected at startup");
            activate_browser_monitor(Arc::clone(&manager_clone)).await;
            was_available = true;
        } else if bridge_available {
            sinfo!("Stasis", "Browser bridge available but media monitoring disabled");
            was_available = true;
        } else {
            sinfo!("Stasis", "Browser bridge not detected, will check periodically");
        }
        
        // Get shutdown flag
        let shutdown = {
            let mgr = manager_clone.lock().await;
            mgr.state.shutdown_flag.clone()
        };
        
        loop {
            tokio::select! {
                _ = shutdown.notified() => {
                    sinfo!("Stasis", "Browser bridge detector shutting down...");
                    break;
                }
                
                _ = check_interval.tick() => {
                    let monitor_enabled = {
                        let mgr = manager_clone.lock().await;
                        mgr.state.cfg.as_ref().map(|c| c.monitor_media).unwrap_or(true)
                    };
                    
                    let is_available = is_bridge_available();
                    
                    if monitor_enabled && is_available && !was_available {
                        sinfo!("Stasis", "Browser bridge now available, activating");
                        activate_browser_monitor(Arc::clone(&manager_clone)).await;
                        was_available = true;
                    } else if (!monitor_enabled || !is_available) && was_available {
                        let reason = if !monitor_enabled { "monitoring disabled" } else { "bridge unavailable" };
                        sinfo!("Stasis", "Browser bridge deactivating ({})", reason);
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
    sinfo!("Media Bridge", "Taking over Firefox monitoring from MPRIS");
    
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
        sinfo!("Media Bridge", "Only Firefox was playing, clearing MPRIS inhibitor");
        decr_active_inhibitor(&mut mgr, InhibitorSource::Media).await;
        mgr.state.media.mpris_media_playing = false;
    } else if mpris_active && non_firefox_playing {
        sinfo!("Media Bridge", "Non-Firefox players still active, keeping MPRIS inhibitor");
        // MPRIS will continue monitoring non-Firefox players
    }
    
    // Activate bridge - this signals MPRIS to skip Firefox checks
    mgr.state.media.media_bridge_active = true;
    
    update_combined_state(&mut mgr);
    
    sdebug!("Media Bridge", "Bridge activated, MPRIS will skip Firefox");
}

/// Deactivate bridge and hand back to MPRIS
async fn deactivate_browser_monitor(manager: Arc<Mutex<Manager>>) {
    sinfo!("Media Bridge", "Handing Firefox monitoring back to MPRIS");
    
    // First, clear all bridge inhibitors
    {
        let mut mgr = manager.lock().await;
        let tab_count = mgr.state.media.browser_playing_tab_count;
        
        if tab_count > 0 {
            sdebug!("Media Bridge", "Clearing {} browser tab inhibitors", tab_count);
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
    sdebug!("Media Bridge", "Bridge deactivated, triggering MPRIS recheck");
    trigger_mpris_recheck(Arc::clone(&manager)).await;
}

/// Trigger a synchronous recheck of MPRIS media state
async fn trigger_mpris_recheck(manager: Arc<Mutex<Manager>>) {
    {
        let mgr = manager.lock().await;
        if !mgr.state.cfg.as_ref().map(|c| c.monitor_media).unwrap_or(true) {
            sdebug!("MPRIS", "Recheck skipped: media monitoring disabled");
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
        serror!("MPRIS", "Bridge still active during recheck - this shouldn't happen");
        return;
    }
    
    let playing = crate::core::services::media::check_media_playing(
        ignore_remote,
        &media_blacklist,
        bridge_active  // Should be false, so Firefox will be checked
    ).await;
    
    let mut mgr = manager.lock().await;
    
    if playing && !mgr.state.media.mpris_media_playing {
        sinfo!("MPRIS", "Recheck: Media playing detected (including Firefox)");
        incr_active_inhibitor(&mut mgr, InhibitorSource::Media).await;
        mgr.state.media.mpris_media_playing = true;
    } else if !playing && mgr.state.media.mpris_media_playing {
        sinfo!("MPRIS", "Recheck: No media playing");
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
       
        sinfo!("Media Bridge", "Browser media monitor started");
        
        let shutdown = {
            let mgr = manager.lock().await;
            mgr.state.shutdown_flag.clone()
        };
        
        loop {
            tokio::select! {
                _ = shutdown.notified() => {
                    sinfo!("Media Bridge", "Browser media monitor shutting down...");
                    break;
                }
                
                _ = poll_interval.tick() => {
                    let (monitor_enabled, bridge_active) = {
                        let mgr = manager.lock().await;
                        let enabled = mgr.state.cfg.as_ref().map(|c| c.monitor_media).unwrap_or(true);
                        (enabled, mgr.state.media.media_bridge_active)
                    };
                    
                    if !monitor_enabled && was_monitoring {
                        sinfo!("Media Bridge", "Media monitoring disabled, cleaning up state");
                        
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
                                sinfo!("Stasis", "Connected to media bridge");
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
                                serror!("Stasis", "Lost connection to media bridge");
                                connected = false;
                                
                                // Clear bridge state before handing off to MPRIS
                                let mut mgr = manager.lock().await;
                                let tab_count = mgr.state.media.browser_playing_tab_count;
                                
                                if tab_count > 0 {
                                    sdebug!("Media Bridge", "Clearing {} inhibitors due to disconnection", tab_count);
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
                                sinfo!("Stasis", "Bridge disconnected, rechecking MPRIS media state");
                                trigger_mpris_recheck(Arc::clone(&manager)).await;
                            }
                        }
                    }
                }
            }
        }
       
        sinfo!("Media Bridge", "Browser media monitor task exited");
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
        sdebug!("Media Bridge", "Browser tab count change: {} → {} (delta: {})", prev_tab_count, new_tab_count, delta);
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
    if new_state.playing {
        sdebug!(
            "Media Bridge",
            "Browser media active: {}/{} tabs playing (IDs: {:?})",
            new_state.playing_tab_count(),
            new_state.tab_count,
            new_state.playing_tabs
        );
    } else if new_state.tab_count > 0 {
        sdebug!(
            "Media Bridge",
            "Browser media stopped ({} tabs have media but none are playing)",
            new_state.tab_count
        );
    } else if old_state.is_some() {
        sdebug!(
            "Media Bridge",
            "Browser media stopped (no tabs with media)"
        );
    }
}
