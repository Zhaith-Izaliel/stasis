use std::sync::Arc;
use crate::{
    SOCKET_PATH,
    core::manager::{Manager, helpers::set_manually_paused},
};
use eventline::{
    event_debug_scoped, event_info_scoped, event_scope_async,
};

/// Handles the "stop" command - gracefully shuts down the daemon
pub async fn handle_stop(manager: Arc<tokio::sync::Mutex<Manager>>) -> String {
    event_scope_async!("Control Stop", {
        event_info_scoped!("Control Stop", "Received stop command - Shutting down gracefully").await;
        
        let manager_clone = Arc::clone(&manager);
        tokio::spawn(async move {
            event_scope_async!("Control Stop Task", {
                let mut mgr = manager_clone.lock().await;
                mgr.shutdown().await;
                event_info_scoped!("Control Stop Task", "Manager shutdown complete, exiting process").await;
                
                let _ = std::fs::remove_file(SOCKET_PATH);
                std::process::exit(0);
            }).await;
        });
        
        "Stopping Stasis...".to_string()
    }).await
}

/// Handles the "toggle_inhibit" command - toggles manual inhibition
pub async fn handle_toggle_inhibit(manager: Arc<tokio::sync::Mutex<Manager>>) -> String {
    event_scope_async!("Control ToggleInhibit", {
        let mut mgr = manager.lock().await;
        let currently_inhibited = mgr.state.is_manually_paused();
        
        if currently_inhibited {
            set_manually_paused(&mut mgr, false).await;
            event_debug_scoped!("Control ToggleInhibit", "Manual inhibit disabled (toggle)").await;
        } else {
            set_manually_paused(&mut mgr, true).await;
            event_debug_scoped!("Control ToggleInhibit", "Manual inhibit enabled (toggle)").await;
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
    }).await
}
