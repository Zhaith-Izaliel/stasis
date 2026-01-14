use std::sync::Arc;
use crate::core::manager::Manager;
use eventline::{event_info_scoped, event_error_scoped, runtime};

/// Handles the "profile" command - switches between profiles
///
/// Fast profile switching with immediate inhibitor rechecks
pub async fn handle_profile(
    manager: Arc<tokio::sync::Mutex<Manager>>,
    profile_arg: &str,
) -> String {
    // Make an owned copy for async move
    let profile_arg_owned = profile_arg.to_owned();

    runtime::scoped_async(Some("ProfileCommand"), || async move {
        if profile_arg_owned.is_empty() {
            event_error_scoped!("ProfileCommand", "Profile command missing profile name");
            return "ERROR: No profile name provided".to_string();
        }

        // Determine profile to switch to
        let profile_name_opt: Option<String> = if profile_arg_owned.eq_ignore_ascii_case("none") {
            None
        } else {
            Some(profile_arg_owned.clone()) // clone for macro usage
        };

        // Switch profile and trigger immediate rechecks
        let result = {
            let mut mgr = manager.lock().await;
            mgr.set_profile(profile_name_opt.as_deref()).await
        };

        match result {
            Ok(msg) => {
                // Clone for logging so original can be used after macro
                let profile_name_for_log = profile_name_opt.clone().unwrap_or_else(|| "base config".to_string());
                event_info_scoped!(
                    "ProfileCommand",
                    "Profile switched: {}",
                    profile_name_for_log
                );
                msg
            }
            Err(e) => {
                let e_for_log = e.clone(); // clone for macro
                event_error_scoped!("ProfileCommand", "Failed to set profile: {}", e_for_log);
                format!("ERROR: {}", e)
            }
        }
    })
    .await
}
