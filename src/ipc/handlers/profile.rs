use std::sync::Arc;
use crate::{
    core::manager::Manager,
    serror, sinfo,
};

/// Handles the "profile" command - switches between profiles
pub async fn handle_profile(
    manager: Arc<tokio::sync::Mutex<Manager>>,
    profile_arg: &str,
) -> String {
    if profile_arg.is_empty() {
        return "ERROR: No profile name provided".to_string();
    }
    
    let profile_name = if profile_arg.eq_ignore_ascii_case("none") {
        None
    } else {
        Some(profile_arg)
    };
    
    let result = {
        let mut mgr = manager.lock().await;
        mgr.set_profile(profile_name).await
    };
    
    // Spawn background tasks
    tokio::spawn({
        let manager = Arc::clone(&manager);
        async move {
            tokio::time::sleep(tokio::time::Duration::from_millis(10)).await;
            let mut mgr = manager.lock().await;
            mgr.trigger_instant_actions().await;
        }
    });
    
    match result {
        Ok(msg) => {
            sinfo!("Stasis", "Profile switched: {}", profile_name.unwrap_or("base config"));
            msg  // Just "Switched to profile: gaming"
        }
        Err(e) => {
            serror!("Stasis", "Failed to set profile: {}", e);
            format!("ERROR: {}", e)
        }
    }
}

