use std::sync::Arc;
use crate::{
    core::manager::Manager,
    serror, sinfo,
};
use super::state_info::{collect_manager_state, format_text_response};

/// Handles the "profile" command - switches between profiles
/// 
/// Simple approach: Sleep briefly to let state settle, then show accurate status
pub async fn handle_profile(
    manager: Arc<tokio::sync::Mutex<Manager>>,
    profile_arg: &str,
) -> String {
    if profile_arg.is_empty() {
        serror!("Stasis", "Profile command missing profile name");
        return "ERROR: No profile name provided".to_string();
    }
    
    let profile_name = if profile_arg.eq_ignore_ascii_case("none") {
        None
    } else {
        Some(profile_arg)
    };
    
    // Fast profile switch
    let result = {
        let mut mgr = manager.lock().await;
        mgr.set_profile(profile_name).await
    }; // Lock released immediately
    
    // Spawn background tasks (instant actions, etc.)
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
            sinfo!(
                "Stasis", 
                "Profile switched: {}", 
                profile_name.unwrap_or("base config")
            );
            
            // Wait for background monitors to settle
            // - App monitor checks every 4 seconds
            // - Media monitor checks every 1 second  
            // - Browser bridge checks every 1 second (if active)
            // So 3-4 seconds should be enough for most state to settle
            tokio::time::sleep(tokio::time::Duration::from_secs(4)).await;
            
            // Now collect settled state
            let mut mgr = manager.lock().await;
            let state_info = collect_manager_state(&mut mgr);
            format_text_response(&state_info, Some(&msg))
        }
        Err(e) => {
            serror!("Stasis", "Failed to set profile: {}", e);
            format!("ERROR: {}", e)
        }
    }
}
