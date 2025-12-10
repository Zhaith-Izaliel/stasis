use std::sync::Arc;
use crate::{
    core::manager::{Manager, helpers::trigger_all_idle_actions},
    ipc::commands::trigger_action_by_name,
    sdebug, serror,
};

/// Handles the "trigger" command - triggers actions by name
pub async fn handle_trigger(
    manager: Arc<tokio::sync::Mutex<Manager>>,
    action: &str,
) -> String {
    if action.is_empty() {
        serror!("Stasis", "Trigger command missing action name");
        return "ERROR: No action name provided".to_string();
    }
    
    if action == "all" {
        return trigger_all(manager).await;
    }
    
    match trigger_action_by_name(manager, action).await {
        Ok(action_name) => format!("Action '{}' triggered successfully", action_name),
        Err(e) => format!("ERROR: {e}"),
    }
}

async fn trigger_all(manager: Arc<tokio::sync::Mutex<Manager>>) -> String {
    let mut mgr = manager.lock().await;
    trigger_all_idle_actions(&mut mgr).await;
    sdebug!("Stasis", "Triggered all idle actions");
    "All idle actions triggered".to_string()
}
