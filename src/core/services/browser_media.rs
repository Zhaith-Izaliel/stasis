use std::sync::Arc;
use std::os::unix::net::UnixStream;
use std::io::{Read, Write};
use std::sync::atomic::{AtomicBool, Ordering};
use tokio::sync::Mutex;
use tokio::time::{interval, Duration};
use serde_json::Value;

use crate::core::manager::{
    helpers::{incr_active_inhibitor, decr_active_inhibitor},
    Manager
};
use crate::log::{log_message, log_error_message};

const BRIDGE_SOCKET: &str = "/tmp/media_bridge.sock";
const POLL_INTERVAL_MS: u64 = 1000;

// Use AtomicBool as a shutdown signal instead of just a "running" flag
static SHUTDOWN_SIGNAL: AtomicBool = AtomicBool::new(false);
static MONITOR_RUNNING: AtomicBool = AtomicBool::new(false);

/// Query the browser media state from the Python bridge
fn query_browser_status() -> Result<BrowserMediaState, String> {
    let mut stream = UnixStream::connect(BRIDGE_SOCKET)
        .map_err(|e| format!("Failed to connect: {}", e))?;
    
    stream.write_all(b"status")
        .map_err(|e| format!("Failed to send: {}", e))?;
    
    let mut buffer = vec![0u8; 4096];
    let size = stream.read(&mut buffer)
        .map_err(|e| format!("Failed to read: {}", e))?;
    
    if size == 0 {
        return Err("Empty response".to_string());
    }
    
    let resp_str = String::from_utf8_lossy(&buffer[..size]);
    let json: Value = serde_json::from_str(&resp_str)
        .map_err(|e| format!("Failed to parse JSON: {}", e))?;
    
    Ok(BrowserMediaState {
        playing: json["playing"].as_bool().unwrap_or(false),
        tab_count: json["tab_count"].as_u64().unwrap_or(0) as usize,
        playing_tabs: json["playing_tabs"]
            .as_array()
            .map(|arr| arr.iter().filter_map(|v| v.as_i64()).map(|i| i as i32).collect())
            .unwrap_or_default(),
    })
}

#[derive(Debug, Clone)]
struct BrowserMediaState {
    playing: bool,
    tab_count: usize,
    playing_tabs: Vec<i32>,
}

/// Stop the browser monitor and wait for it to clean up
pub async fn stop_browser_monitor(manager: Arc<Mutex<Manager>>) {
    log_message("Stopping browser media monitor...");
    
    // Signal shutdown
    SHUTDOWN_SIGNAL.store(true, Ordering::SeqCst);
    
    // Wait a bit for the task to notice and exit
    tokio::time::sleep(Duration::from_millis(100)).await;
    
    // Clear all browser inhibitors
    {
        let mut mgr = manager.lock().await;
        let prev_tab_count = mgr.state.browser_playing_tab_count;
        if prev_tab_count > 0 {
            log_message(&format!(
                "Clearing {} browser tab inhibitors",
                prev_tab_count
            ));
            for _ in 0..prev_tab_count {
                decr_active_inhibitor(&mut mgr).await;
            }
        }
        
        // Reset browser state
        mgr.state.browser_playing_tab_count = 0;
        mgr.state.browser_media_playing = false;
        mgr.state.media_bridge_active = false;
    }
    
    // Reset flags for next spawn
    MONITOR_RUNNING.store(false, Ordering::SeqCst);
    SHUTDOWN_SIGNAL.store(false, Ordering::SeqCst);
    
    log_message("Browser media monitor stopped");
}

/// Spawn a background task that polls the browser media bridge
pub async fn spawn_browser_media_monitor(manager: Arc<Mutex<Manager>>) {
    // Prevent multiple monitors from running
    if MONITOR_RUNNING.swap(true, Ordering::SeqCst) {
        log_message("Browser media monitor already running, skipping spawn");
        return;
    }

    // Mark media bridge as active and initialize tracking
    {
        let mut mgr = manager.lock().await;
        mgr.state.media_bridge_active = true;
        // Initialize browser tab count to 0 when starting
        mgr.state.browser_playing_tab_count = 0;
        mgr.state.browser_media_playing = false;
    }

    tokio::spawn(async move {
        let mut poll_interval = interval(Duration::from_millis(POLL_INTERVAL_MS));
        let mut last_state: Option<BrowserMediaState> = None;
        let mut connected = false;
        
        log_message("Browser media monitor started");
        
        loop {
            // Check for shutdown signal
            if SHUTDOWN_SIGNAL.load(Ordering::SeqCst) {
                log_message("Browser media monitor received shutdown signal, exiting");
                break;
            }
            
            poll_interval.tick().await;
            
            match query_browser_status() {
                Ok(state) => {
                    if !connected {
                        log_message("Connected to MPRIS bridge");
                        connected = true;
                    }
                    
                    // Check if state changed 
                    let state_changed = last_state.as_ref().map(|last| {
                        last.playing != state.playing ||
                        last.tab_count != state.tab_count ||
                        last.playing_tabs != state.playing_tabs
                    }).unwrap_or(true);

                    
                    if state_changed {
                        handle_browser_media_state(manager.clone(), &state).await;
                        
                        if state.playing {
                            log_message(&format!(
                                "Browser media active: {}/{} tabs playing (IDs: {:?})",
                                state.playing_tabs.len(),
                                state.tab_count,
                                state.playing_tabs
                            ));
                        } else if state.tab_count > 0 {
                            log_message(&format!(
                                "Browser media stopped ({} tabs with media, none playing)",
                                state.tab_count
                            ));
                        } else if last_state.is_some() {
                            log_message("Browser media stopped (no tabs with media)");
                        }
                    }
                    
                    last_state = Some(state);
                }
                Err(_e) => {
                    if connected {
                        log_error_message("Lost connection to Firefox MPRIS bridge");
                        connected = false;
                        
                        // Treat as "no media playing"
                        let empty_state = BrowserMediaState {
                            playing: false,
                            tab_count: 0,
                            playing_tabs: vec![],
                        };
                        handle_browser_media_state(manager.clone(), &empty_state).await;
                        last_state = None;
                    }
                }
            }
        }
        
        log_message("Browser media monitor task exited");
    });
}

/// Handle state changes from the browser
async fn handle_browser_media_state(
    manager: Arc<Mutex<Manager>>,
    state: &BrowserMediaState,
) {
    let mut mgr = manager.lock().await;

    // Track number of tabs playing
    let prev_tab_count = mgr.state.browser_playing_tab_count;
    let new_tab_count = state.playing_tabs.len();

    // Calculate the delta
    let delta = new_tab_count as i32 - prev_tab_count as i32;

    if delta != 0 {
        log_message(&format!(
            "Browser tab count change: {} → {} (delta: {})",
            prev_tab_count, new_tab_count, delta
        ));
    }

    // Update the tab count
    mgr.state.browser_playing_tab_count = new_tab_count;

    // Apply inhibitor changes based on delta
    if delta > 0 {
        // Tabs started playing
        for _ in 0..delta {
            incr_active_inhibitor(&mut mgr).await;
        }
        mgr.state.media_playing = true;
        mgr.state.media_blocking = true;
    } else if delta < 0 {
        // Tabs stopped playing
        for _ in 0..delta.abs() {
            decr_active_inhibitor(&mut mgr).await;
        }
        if new_tab_count == 0 {
            mgr.state.media_playing = false;
            mgr.state.media_blocking = false;
        }
    }

    // Update browser media playing flag
    mgr.state.browser_media_playing = new_tab_count > 0;
}
