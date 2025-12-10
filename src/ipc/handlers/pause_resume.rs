use std::sync::Arc;
use crate::core::manager::Manager;

/// Handles the "pause" command with optional arguments
pub async fn handle_pause(
    manager: Arc<tokio::sync::Mutex<Manager>>,
    args: &str,
) -> String {
    if args.eq_ignore_ascii_case("help") 
        || args == "-h" 
        || args == "--help" {
        return crate::ipc::pause::PAUSE_HELP_MESSAGE.to_string();
    }
    
    match crate::ipc::pause::handle_pause_command(manager, args).await {
        Ok(msg) => msg,
        Err(e) => format!("ERROR: {}", e),
    }
}

/// Handles the "resume" command
pub async fn handle_resume(manager: Arc<tokio::sync::Mutex<Manager>>) -> String {
    let mut mgr = manager.lock().await;
    mgr.resume(true).await;
    "Idle manager resumed".to_string()
}
