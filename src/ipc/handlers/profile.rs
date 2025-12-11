use std::sync::Arc;
use crate::{
    core::manager::Manager,
    serror, sinfo,
};
use super::state_info::{collect_manager_state, format_text_response};

/// Handles the "profile" command - switches between profiles
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
    
    let mut mgr = manager.lock().await;
    
    match mgr.set_profile(profile_name).await {
        Ok(msg) => {
            sinfo!(
                "Stasis", 
                "Profile switched: {}", 
                profile_name.unwrap_or("base config")
            );
            mgr.trigger_instant_actions().await;
            
            // Collect state and format response
            let state_info = collect_manager_state(&mut mgr);
            format_text_response(&state_info, Some(&msg))
        }
        Err(e) => {
            serror!("Stasis", "Failed to set profile: {}", e);
            format!("ERROR: {}", e)
        }
    }
}
