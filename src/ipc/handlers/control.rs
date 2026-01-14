use std::sync::Arc;
use crate::{
    core::manager::{Manager, helpers::set_manually_paused},
    daemon::ShutdownSender,
};
use eventline::{
    event_debug_scoped, event_info_scoped,
};

/// Handles the "stop" command - gracefully shuts down the daemon via shutdown channel
pub async fn handle_stop(
    _manager: Arc<tokio::sync::Mutex<Manager>>,
    shutdown_tx: ShutdownSender,
) -> String {
    event_info_scoped!("Control Stop", "Received stop command");
    
    // Send shutdown signal
    let _ = shutdown_tx.send("IPC stop").await;
    
    "Stopping Stasis...".to_string()
}

/// Handles the "toggle_inhibit" command - toggles manual inhibition
pub async fn handle_toggle_inhibit(manager: Arc<tokio::sync::Mutex<Manager>>) -> String {
    let mut mgr = manager.lock().await;
    let currently_inhibited = mgr.state.is_manually_paused();
    
    if currently_inhibited {
        set_manually_paused(&mut mgr, false).await;
        event_debug_scoped!("Control ToggleInhibit", "Manual inhibit disabled (toggle)");
    } else {
        set_manually_paused(&mut mgr, true).await;
        event_debug_scoped!("Control ToggleInhibit", "Manual inhibit enabled (toggle)");
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
