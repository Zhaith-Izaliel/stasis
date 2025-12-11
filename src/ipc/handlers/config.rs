use std::sync::Arc;
use tokio::time::Duration;
use crate::{
    config::{self, model::CombinedConfig},
    core::{
        manager::Manager,
        services::app_inhibit::AppInhibitor
    },
    sdebug, serror, sinfo,
};
use super::state_info::{collect_full_state, format_text_response};

/// Handles the "reload" command - reloads configuration from disk
pub async fn handle_reload(
    manager: Arc<tokio::sync::Mutex<Manager>>,
    app_inhibitor: Arc<tokio::sync::Mutex<AppInhibitor>>,
) -> String {
    match config::parser::load_combined_config() {
        Ok(combined) => {
            reload_config_internal(manager, app_inhibitor, combined).await
        }
        Err(e) => {
            serror!("Stasis", "Failed to reload config: {}", e);
            format!("ERROR: Failed to reload config: {e}")
        }
    }
}

async fn reload_config_internal(
    manager: Arc<tokio::sync::Mutex<Manager>>,
    app_inhibitor: Arc<tokio::sync::Mutex<AppInhibitor>>,
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
    
    // Update manager state with new config
    {
        let mut mgr = manager.lock().await;
        mgr.state.update_from_config(&combined.base).await;
        mgr.state.reload_profiles(&combined).await;
    }
    
    // Restart media monitoring if enabled
    if combined.base.monitor_media {
        restart_media_monitoring(Arc::clone(&manager)).await;
    } else {
        let mut mgr = manager.lock().await;
        mgr.trigger_instant_actions().await;
    }
    
    // Get current state for response
    let state_info = collect_full_state(Arc::clone(&manager), Arc::clone(&app_inhibitor)).await;
    
    // Update app inhibitor
    {
        let mut inhibitor = app_inhibitor.lock().await;
        inhibitor.update_from_config(&combined.base).await;
    }
    
    sdebug!("Stasis", "Config reloaded successfully");
    
    format_text_response(&state_info, Some("Config reloaded successfully"))
}

async fn cleanup_media_monitoring(manager: Arc<tokio::sync::Mutex<Manager>>) {
    let mut mgr = manager.lock().await;
    mgr.cleanup_media_monitoring().await;
    
    if mgr.state.media.media_bridge_active {
        drop(mgr);
        crate::core::services::browser_media::stop_browser_monitor(manager).await;
    }
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
