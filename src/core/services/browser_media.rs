use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use tokio::sync::Mutex;
use tokio::time::{interval, Duration};

use crate::core::manager::{
    inhibitors::{incr_active_inhibitor, decr_active_inhibitor},
    Manager
};
use crate::{media_bridge, sdebug, serror, sinfo};

const POLL_INTERVAL_MS: u64 = 1000;
const BRIDGE_CHECK_INTERVAL_SECS: u64 = 10;

static MONITOR_SHUTDOWN: AtomicBool = AtomicBool::new(false);
static MONITOR_RUNNING: AtomicBool = AtomicBool::new(false);

pub fn is_bridge_available() -> bool {
    media_bridge::is_available()
}

pub async fn spawn_browser_bridge_detector(manager: Arc<Mutex<Manager>>) {
    let manager_clone = Arc::clone(&manager);
    
    tokio::spawn(async move {
        let mut interval = tokio::time::interval(Duration::from_secs(BRIDGE_CHECK_INTERVAL_SECS));
        let mut was_available = false;
        
        if is_bridge_available() {
            sinfo!("Stasis", "Browser bridge detected at startup");
            activate_browser_monitor(Arc::clone(&manager_clone)).await;
            was_available = true;
        } else {
            sinfo!("Stasis", "Browser bridge not detected, will check periodically");
        }
        
        loop {
            interval.tick().await;
            let is_available = is_bridge_available();
            
            if is_available && !was_available {

                sinfo!("Stasis", "Browser bridge now available, activating monitor");
                activate_browser_monitor(Arc::clone(&manager_clone)).await;
                was_available = true;
            } else if !is_available && was_available {
                sinfo!("Stasis", "Browser bridge no longer available, deactivating monitor");
                deactivate_browser_monitor(Arc::clone(&manager_clone)).await;
                was_available = false;
            }
        }
    });
}

/// Activate the browser monitor and update manager state
async fn activate_browser_monitor(manager: Arc<Mutex<Manager>>) {
    {
        let mut mgr = manager.lock().await;
        
        if mgr.state.media.mpris_media_playing {
            let _bridge_active = false; // temporarily set to false to check Firefox
            let ignore_remote = mgr.state.cfg
                .as_ref()
                .map(|c| c.ignore_remote_media)
                .unwrap_or(false);
            let blacklist = mgr.state.cfg
                .as_ref()
                .map(|c| c.media_blacklist.clone())
                .unwrap_or_default();
            
            // Check non-Firefox players
            let non_ff_playing = crate::core::services::media::check_media_playing(
                ignore_remote,
                &blacklist,
                true // skip Firefox
            );
            
            if !non_ff_playing {
                sinfo!("Stasis", "Clearing MPRIS inhibitors (Firefox transitioning to bridge)");
                decr_active_inhibitor(&mut mgr).await;
                mgr.state.media.mpris_media_playing = false;
            }
        }
        
        mgr.state.media.media_bridge_active = true;
    }
    
    spawn_browser_media_monitor(Arc::clone(&manager)).await;
}

async fn deactivate_browser_monitor(manager: Arc<Mutex<Manager>>) {
    stop_browser_monitor(Arc::clone(&manager)).await;
}

/// Stop the browser monitor and clean up state
pub async fn stop_browser_monitor(manager: Arc<Mutex<Manager>>) {
    sinfo!("Stasis", "Stopping browser media monitor...");
    
    // Signal the monitor task to shut down
    MONITOR_SHUTDOWN.store(true, Ordering::SeqCst);
    
    // Give the task time to exit gracefully
    tokio::time::sleep(Duration::from_millis(150)).await;
    
    // Clean up all browser-related inhibitors and state
    {
        let mut mgr = manager.lock().await;
        let prev_tab_count = mgr.state.media.browser_playing_tab_count;
        
        if prev_tab_count > 0 {
            sinfo!("Stasis", "Clearing {} browser tab inhibitors", prev_tab_count);
            
            // Remove all tab inhibitors
            for _ in 0..prev_tab_count {
                decr_active_inhibitor(&mut mgr).await;
            }
        }
        
        mgr.state.media.browser_playing_tab_count = 0;
        mgr.state.media.browser_media_playing = false;
        mgr.state.media.media_bridge_active = false;
        
        update_combined_state(&mut mgr);
    }
    
    MONITOR_RUNNING.store(false, Ordering::SeqCst);
    MONITOR_SHUTDOWN.store(false, Ordering::SeqCst);
   
    sinfo!("Stasis", "Browser media monitor stopped");
}

async fn spawn_browser_media_monitor(manager: Arc<Mutex<Manager>>) {
    if MONITOR_RUNNING.swap(true, Ordering::SeqCst) {
        sinfo!("Stasis", "Browser media monitor already running, skipping spawn");
        return;
    }

    {
        let mut mgr = manager.lock().await;
        mgr.state.media.browser_playing_tab_count = 0;
        mgr.state.media.browser_media_playing = false;
    }

    tokio::spawn(async move {
        let mut poll_interval = interval(Duration::from_millis(POLL_INTERVAL_MS));
        let mut last_state: Option<media_bridge::BrowserMediaState> = None;
        let mut connected = false;
       
        sinfo!("Media Bridge", "Browser media monitor started");
        
        loop {
            if MONITOR_SHUTDOWN.load(Ordering::SeqCst) {
                sinfo!("Media Bridge", "Browser media monitor received shutdown sigal, exiting");
                break;
            }
            
            poll_interval.tick().await;
            
            match media_bridge::query_status() {
                Ok(state) => {
                    if !connected {
                        sinfo!("Stasis", "Connected to media bridge");
                        connected = true;
                    }
                    
                    let state_changed = last_state
                        .as_ref()
                        .map(|last| state.has_changed_from(last))
                        .unwrap_or(true); // First poll always counts as changed
                    
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
                        
                        // Treat loss of connection as "no media playing"
                        let empty_state = media_bridge::BrowserMediaState::empty();
                        update_manager_state(manager.clone(), &empty_state, last_state.as_ref()).await;
                        last_state = None;
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

    // Update the stored tab count
    mgr.state.media.browser_playing_tab_count = new_tab_count;

    // Adjust inhibitor count based on delta
    if delta > 0 {
        // More tabs started playing
        for _ in 0..delta {
            incr_active_inhibitor(&mut mgr).await;
        }
    } else if delta < 0 {
        // Tabs stopped playing
        for _ in 0..delta.abs() {
            decr_active_inhibitor(&mut mgr).await;
        }
    }

    mgr.state.media.browser_media_playing = new_tab_count > 0; 
    update_combined_state(&mut mgr);
}

/// Update the combined media_playing and media_blocking flags based on all sources
fn update_combined_state(mgr: &mut Manager) {
    let combined = mgr.state.media.mpris_media_playing || mgr.state.media.browser_media_playing;
    mgr.state.media.media_playing = combined;
    mgr.state.media.media_blocking = combined;
}

/// Log state changes for debugging
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

