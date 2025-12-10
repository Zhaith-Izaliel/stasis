use std::sync::Arc;
use crate::{
    SOCKET_PATH,
    core::manager::{Manager, helpers::set_manually_paused},
    sdebug, sinfo,
};

/// Handles the "stop" command - gracefully shuts down the daemon
pub async fn handle_stop(manager: Arc<tokio::sync::Mutex<Manager>>) -> String {
    sinfo!("Stasis", "Received stop command - Shutting down gracefully");
    
    let manager_clone = Arc::clone(&manager);
    tokio::spawn(async move {
        let mut mgr = manager_clone.lock().await;
        mgr.shutdown().await;
        sinfo!("Stasis", "Manager shutdown complete, exiting process");
        let _ = std::fs::remove_file(SOCKET_PATH);
        std::process::exit(0);
    });
    
    "Stopping Stasis...".to_string()
}

/// Handles the "toggle_inhibit" command - toggles manual inhibition
pub async fn handle_toggle_inhibit(manager: Arc<tokio::sync::Mutex<Manager>>) -> String {
    let mut mgr = manager.lock().await;
    let currently_inhibited = mgr.state.is_manually_paused();
    
    if currently_inhibited {
        set_manually_paused(&mut mgr, false).await;
        sdebug!("Stasis", "Manual inhibit disabled (toggle)");
    } else {
        set_manually_paused(&mut mgr, true).await;
        sdebug!("Stasis", "Manual inhibit enabled (toggle)");
    }
    
    let response = if currently_inhibited {
        serde_json::json!({
            "text": "Active",
            "alt": "idle_active",
            "tooltip": "Idle inhibition cleared"
        })
    } else {
        serde_json::json!({
            "text": "Inhibited",
            "alt": "manually_inhibited",
            "tooltip": "Idle inhibition active"
        })
    };
    
    response.to_string()
}
