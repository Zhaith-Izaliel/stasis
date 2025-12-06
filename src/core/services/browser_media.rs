use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use tokio::sync::Mutex;
use tokio::time::{interval, Duration};

use crate::core::manager::{
    inhibitors::{incr_active_inhibitor, decr_active_inhibitor},
    Manager
};
use crate::log::{log_error_message, log_media_bridge_message, log_message};
use crate::media_bridge;

const POLL_INTERVAL_MS: u64 = 1000;
const BRIDGE_CHECK_INTERVAL_SECS: u64 = 10;

// Coordination state for the monitor task
static MONITOR_SHUTDOWN: AtomicBool = AtomicBool::new(false);
static MONITOR_RUNNING: AtomicBool = AtomicBool::new(false);

/// Check if the external Firefox media bridge is available
pub fn is_bridge_available() -> bool {
    media_bridge::is_available()
}

/// Start monitoring for browser bridge availability and spawn monitor when detected
pub async fn spawn_browser_bridge_detector(manager: Arc<Mutex<Manager>>) {
    let manager_clone = Arc::clone(&manager);
    
    tokio::spawn(async move {
        let mut interval = tokio::time::interval(Duration::from_secs(BRIDGE_CHECK_INTERVAL_SECS));
        let mut was_available = false;
        
        // Check immediately on startup
        if is_bridge_available() {
            log_media_bridge_message("Browser bridge detected at startup");
            activate_browser_monitor(Arc::clone(&manager_clone)).await;
            was_available = true;
        } else {
            log_message("Browser bridge not detected, will check periodically");
        }
        
        loop {
            interval.tick().await;
            let is_available = is_bridge_available();
            
            if is_available && !was_available {
                log_media_bridge_message("Browser bridge now available, activating monitor");
                activate_browser_monitor(Arc::clone(&manager_clone)).await;
                was_available = true;
            } else if !is_available && was_available {
                log_message("Browser bridge no longer available, deactivating monitor");
                deactivate_browser_monitor(Arc::clone(&manager_clone)).await;
                was_available = false;
            }
        }
    });
}

/// Activate the browser monitor and update manager state
async fn activate_browser_monitor(manager: Arc<Mutex<Manager>>) {
    // Set bridge active flag so MPRIS monitor skips Firefox
    {
        let mut mgr = manager.lock().await;
        
        // If there was MPRIS-detected Firefox media, clear it since bridge will handle it now
        if mgr.state.media.mpris_media_playing {
            // Check if the only thing playing was Firefox
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
                // Only Firefox was playing, clear the MPRIS inhibitor
                log_message("Clearing MPRIS inhibitor (Firefox transitioning to bridge)");
                decr_active_inhibitor(&mut mgr).await;
                mgr.state.media.mpris_media_playing = false;
            }
        }
        
        mgr.state.media.media_bridge_active = true;
    }
    
    // Spawn the actual monitor task
    spawn_browser_media_monitor(Arc::clone(&manager)).await;
}

/// Deactivate the browser monitor and clean up state
async fn deactivate_browser_monitor(manager: Arc<Mutex<Manager>>) {
    // Stop the monitor task and clear browser inhibitors
    stop_browser_monitor(Arc::clone(&manager)).await;
    
    // MPRIS monitor will naturally pick up Firefox again since bridge_active is now false
}

/// Stop the browser monitor and clean up state
pub async fn stop_browser_monitor(manager: Arc<Mutex<Manager>>) {
    log_media_bridge_message("Stopping browser media monitor...");
    
    // Signal the monitor task to shut down
    MONITOR_SHUTDOWN.store(true, Ordering::SeqCst);
    
    // Give the task time to exit gracefully
    tokio::time::sleep(Duration::from_millis(150)).await;
    
    // Clean up all browser-related inhibitors and state
    {
        let mut mgr = manager.lock().await;
        let prev_tab_count = mgr.state.media.browser_playing_tab_count;
        
        if prev_tab_count > 0 {
            log_message(&format!(
                "Clearing {} browser tab inhibitors",
                prev_tab_count
            ));
            
            // Remove all tab inhibitors
            for _ in 0..prev_tab_count {
                decr_active_inhibitor(&mut mgr).await;
            }
        }
        
        // Reset all browser-related state
        mgr.state.media.browser_playing_tab_count = 0;
        mgr.state.media.browser_media_playing = false;
        mgr.state.media.media_bridge_active = false;
        
        // Update combined state
        update_combined_state(&mut mgr);
    }
    
    // Reset coordination flags for next spawn
    MONITOR_RUNNING.store(false, Ordering::SeqCst);
    MONITOR_SHUTDOWN.store(false, Ordering::SeqCst);
    
    log_message("Browser media monitor stopped");
}

/// Spawn a background task that polls the external browser media bridge
async fn spawn_browser_media_monitor(manager: Arc<Mutex<Manager>>) {
    // Prevent multiple monitors from running simultaneously
    if MONITOR_RUNNING.swap(true, Ordering::SeqCst) {
        log_media_bridge_message("Browser media monitor already running, skipping spawn");
        return;
    }

    // Initialize manager state for bridge monitoring
    {
        let mut mgr = manager.lock().await;
        mgr.state.media.browser_playing_tab_count = 0;
        mgr.state.media.browser_media_playing = false;
    }

    tokio::spawn(async move {
        let mut poll_interval = interval(Duration::from_millis(POLL_INTERVAL_MS));
        let mut last_state: Option<media_bridge::BrowserMediaState> = None;
        let mut connected = false;
        
        log_media_bridge_message("Browser media monitor started");
        
        loop {
            // Check for shutdown signal
            if MONITOR_SHUTDOWN.load(Ordering::SeqCst) {
                log_message("Browser media monitor received shutdown signal, exiting");
                break;
            }
            
            poll_interval.tick().await;
            
            // Query the external bridge
            match media_bridge::query_status() {
                Ok(state) => {
                    if !connected {
                        log_media_bridge_message("Connected to media bridge");
                        connected = true;
                    }
                    
                    // Check if state changed since last poll
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
                        log_error_message("Lost connection to media bridge");
                        connected = false;
                        
                        // Treat loss of connection as "no media playing"
                        let empty_state = media_bridge::BrowserMediaState::empty();
                        update_manager_state(manager.clone(), &empty_state, last_state.as_ref()).await;
                        last_state = None;
                    }
                }
            }
        }
        
        log_media_bridge_message("Browser media monitor task exited");
    });
}

/// Update manager state based on browser media changes
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
        log_media_bridge_message(&format!(
            "Browser tab count change: {} → {} (delta: {})",
            prev_tab_count, new_tab_count, delta
        ));
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

    // Update the browser media playing flag
    mgr.state.media.browser_media_playing = new_tab_count > 0;
    
    // Update combined state
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
        log_media_bridge_message(&format!(
            "Browser media active: {}/{} tabs playing (IDs: {:?})",
            new_state.playing_tab_count(),
            new_state.tab_count,
            new_state.playing_tabs
        ));
    } else if new_state.tab_count > 0 {
        log_media_bridge_message(&format!(
            "Browser media stopped ({} tabs with media, none playing)",
            new_state.tab_count
        ));
    } else if old_state.is_some() {
        log_media_bridge_message("Browser media stopped (no tabs with media)");
    }
}
