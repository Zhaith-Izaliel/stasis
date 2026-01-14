use std::sync::Arc;
use tokio::time::Duration;
use crate::{
    config::{self, model::CombinedConfig, info::InfoSections},
    core::manager::{Manager, helpers::profile_to_stasis_config},
};
use super::state_info::{collect_full_state, format_text_response};
use eventline::{
    event_debug_scoped, event_error_scoped, event_info_scoped, event_warn_scoped, scoped_eventline
};

/// Handles the "reload" command - reloads configuration from disk
pub async fn handle_reload(
    manager: Arc<tokio::sync::Mutex<Manager>>,
) -> String {
    match config::parser::load_combined_config().await {
        Ok(combined) => reload_config_internal(manager, combined).await,
        Err(e) => {
            // Build a temporary owned string for the logger (logger may take ownership).
            // Then build the return string separately using `e` again.
            let log_str = format!("Failed to reload config: {}", e);
            event_error_scoped!("Config", "{}", log_str);
            format!("ERROR: Failed to reload config: {}", e)
        }
    }
}

async fn reload_config_internal(
    manager: Arc<tokio::sync::Mutex<Manager>>,
    combined: CombinedConfig,
) -> String {
    scoped_eventline!("Config Reload", {
        // Determine if media monitoring should be cleaned up
        let should_cleanup = {
            let mgr = manager.lock().await;
            let old_monitor = mgr.state.cfg
                .as_ref()
                .map(|c| c.monitor_media)
                .unwrap_or(true);
            let new_monitor = combined.base.monitor_media;
            old_monitor && !new_monitor
        };

        if should_cleanup {
            cleanup_media_monitoring(Arc::clone(&manager)).await;
        }

        // Determine which config to apply and status message
        let (config_to_apply, profile_status) = {
            let mgr = manager.lock().await;
            let current_profile = mgr.state.profile.active_profile.clone();

            match current_profile {
                None => (combined.base.clone(), "Staying on base configuration".to_string()),
                Some(ref profile_name) => {
                    if let Some(profile) = combined.profiles.iter().find(|p| &p.name == profile_name) {
                        let profile_config = profile_to_stasis_config(profile);
                        (profile_config, format!("Staying on profile: {}", profile_name))
                    } else {
                        let profile_name_clone = profile_name.clone();
                        event_warn_scoped!(
                            "Config Reload",
                            "Profile '{}' no longer exists, falling back to base config",
                            profile_name_clone
                        );
                        (
                            combined.base.clone(),
                            format!("Profile '{}' removed, switched to base config", profile_name)
                        )
                    }
                }
            }
        };

        // Apply config to manager
        {
            let mut mgr = manager.lock().await;
            mgr.state.reload_profiles(&combined).await;
            mgr.state.update_from_config(&config_to_apply).await;

            if profile_status.starts_with("Profile") && profile_status.contains("removed") {
                mgr.state.profile.set_active(None);
            }
        }

        // Restart media monitoring if enabled
        if config_to_apply.monitor_media {
            restart_media_monitoring(Arc::clone(&manager)).await;
        } else {
            let mut mgr = manager.lock().await;
            mgr.trigger_instant_actions().await;
        }

        // Get current state for response
        let state_info = collect_full_state(Arc::clone(&manager)).await;

        // Log debug without moving the original profile_status
        let profile_status_clone = profile_status.clone();
        event_debug_scoped!("Config Reload", "Config reloaded successfully: {}", profile_status_clone);

        let info_output = format_text_response(&state_info, InfoSections::default());
        format!("Config reloaded successfully\n{}\n\n{}", profile_status, info_output)
    })
}

async fn cleanup_media_monitoring(manager: Arc<tokio::sync::Mutex<Manager>>) {
    let mut mgr = manager.lock().await;
    mgr.cleanup_media_monitoring().await;
}

async fn restart_media_monitoring(manager: Arc<tokio::sync::Mutex<Manager>>) {
    event_info_scoped!("Config Reload", "Restarting media monitoring after config reload...");

    if let Err(e) = crate::core::services::media::spawn_media_monitor_dbus(
        Arc::clone(&manager)
    ).await {
        // Build a temporary owned string for the logger; we don't need it after the log.
        let log_str = format!("Failed to restart media monitor: {}", e);
        event_error_scoped!("Config Reload", "{}", log_str);
    }

    tokio::time::sleep(Duration::from_millis(100)).await;

    let mut mgr = manager.lock().await;
    mgr.recheck_media().await;
    mgr.trigger_instant_actions().await;
}
