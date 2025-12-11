use std::sync::Arc;
use tokio::time::Duration;
use crate::{
    config::{self, model::CombinedConfig},
    core::manager::{Manager, helpers::profile_to_stasis_config},
    sdebug, serror, sinfo, swarn,
};
use super::state_info::{collect_full_state, format_text_response};

/// Handles the "reload" command - reloads configuration from disk
pub async fn handle_reload(
    manager: Arc<tokio::sync::Mutex<Manager>>,
) -> String {
    match config::parser::load_combined_config() {
        Ok(combined) => {
            reload_config_internal(manager, combined).await
        }
        Err(e) => {
            serror!("Stasis", "Failed to reload config: {}", e);
            format!("ERROR: Failed to reload config: {e}")
        }
    }
}

async fn reload_config_internal(
    manager: Arc<tokio::sync::Mutex<Manager>>,
    combined: CombinedConfig,
) -> String {
    // Check if we need to cleanup media monitoring
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
    
    let (config_to_apply, profile_status) = {
        let mgr = manager.lock().await;
        let current_profile = mgr.state.profile.active_profile.clone();
        
        match current_profile {
            None => {
                // Using base config, keep using base
                (combined.base.clone(), "Staying on base configuration".to_string())
            }
            Some(ref profile_name) => {
                // Check if current profile still exists in reloaded config
                if let Some(profile) = combined.profiles.iter().find(|p| &p.name == profile_name) {
                    // Profile still exists, use it
                    let profile_config = profile_to_stasis_config(profile);
                    (profile_config, format!("Staying on profile: {}", profile_name))
                } else {
                    // Profile was removed, fall back to base
                    swarn!("Stasis", "Profile '{}' no longer exists, falling back to base config", profile_name);
                    (combined.base.clone(), format!("Profile '{}' removed, switched to base config", profile_name))
                }
            }
        }
    };
    
    // Update manager state with the appropriate config
    {
        let mut mgr = manager.lock().await;
        
        // First reload the profiles list
        mgr.state.reload_profiles(&combined).await;
        
        // Then apply the appropriate config (base or current profile)
        mgr.state.update_from_config(&config_to_apply).await;
        
        // Update active profile tracking (None if fell back to base, Some if staying on profile)
        if profile_status.starts_with("Profile") && profile_status.contains("removed") {
            mgr.state.profile.set_active(None);
        }
        // If staying on profile, active_profile is already set correctly
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
    
    sdebug!("Stasis", "Config reloaded successfully: {}", profile_status);
    
    format_text_response(&state_info, Some(&format!("Config reloaded successfully\n{}", profile_status)))
}

async fn cleanup_media_monitoring(manager: Arc<tokio::sync::Mutex<Manager>>) {
    let mut mgr = manager.lock().await;
    mgr.cleanup_media_monitoring().await;
}

async fn restart_media_monitoring(manager: Arc<tokio::sync::Mutex<Manager>>) {
    sinfo!("Stasis", "Restarting media monitoring after config reload...");
    
    if let Err(e) = crate::core::services::media::spawn_media_monitor_dbus(
        Arc::clone(&manager)
    ).await {
        serror!("Stasis", "Failed to restart media monitor: {}", e);
    }
    
    tokio::time::sleep(Duration::from_millis(100)).await;
    
    let mut mgr = manager.lock().await;
    mgr.recheck_media().await;
    mgr.trigger_instant_actions().await;
}
