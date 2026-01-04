use std::sync::Arc;
use crate::{
    core::manager::Manager,
    serror, sinfo,
};

/// Handles the "profile" command - switches between profiles
/// 
/// Fast profile switching with immediate inhibitor rechecks
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
    
    // Switch profile and trigger immediate rechecks
    let result = {
        let mut mgr = manager.lock().await;
        mgr.set_profile(profile_name).await
    };
    
    match result {
        Ok(msg) => {
            sinfo!(
                "Stasis", 
                "Profile switched: {}", 
                profile_name.unwrap_or("base config")
            );
            msg
        }
        Err(e) => {
            serror!("Stasis", "Failed to set profile: {}", e);
            format!("ERROR: {}", e)
        }
    }
}
