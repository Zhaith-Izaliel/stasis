use std::sync::Arc;
use tokio::time::Duration;
use crate::{
    config::{self, model::CombinedConfig},
    core::{
        manager::{Manager, helpers::{current_profile, list_profiles}},
        services::app_inhibit::AppInhibitor
    },
    sdebug, serror, sinfo,
};

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
    let state_info = collect_state_info(Arc::clone(&manager), Arc::clone(&app_inhibitor)).await;
    
    // Update app inhibitor
    {
        let mut inhibitor = app_inhibitor.lock().await;
        inhibitor.update_from_config(&combined.base).await;
    }
    
    sdebug!("Stasis", "Config reloaded successfully");
    
    format_reload_response(state_info)
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

struct StateInfo {
    idle_time: Duration,
    uptime: Duration,
    manually_inhibited: bool,
    paused: bool,
    media_blocking: bool,
    media_bridge_active: bool,
    app_blocking: bool,
    cfg: Option<Arc<crate::config::model::StasisConfig>>,
    profile: Option<String>,
    available_profiles: Vec<String>,
}

async fn collect_state_info(
    manager: Arc<tokio::sync::Mutex<Manager>>,
    app_inhibitor: Arc<tokio::sync::Mutex<AppInhibitor>>,
) -> StateInfo {
    let (idle_time, uptime, manually_inhibited, paused, media_blocking, 
         media_bridge_active, cfg, profile, available_profiles) = {
        let mut mgr = manager.lock().await;
        (
            mgr.state.timing.last_activity.elapsed(),
            mgr.state.timing.start_time.elapsed(),
            mgr.state.inhibitors.manually_paused,
            mgr.state.inhibitors.paused,
            mgr.state.media.media_blocking,
            mgr.state.media.media_bridge_active,
            mgr.state.cfg.clone(),
            current_profile(&mut mgr),
            list_profiles(&mut mgr)
        )
    };
    
    let app_blocking = tokio::time::timeout(
        Duration::from_millis(100),
        async {
            let mut inhibitor = app_inhibitor.lock().await;
            inhibitor.is_any_app_running().await
        }
    ).await.unwrap_or(false);
    
    StateInfo {
        idle_time,
        uptime,
        manually_inhibited,
        paused,
        media_blocking,
        media_bridge_active,
        app_blocking,
        cfg,
        profile,
        available_profiles,
    }
}

fn format_reload_response(info: StateInfo) -> String {
    if let Some(cfg) = &info.cfg {
        let profiles = if info.available_profiles.is_empty() {
            None
        } else {
            Some(info.available_profiles.as_slice())
        };
        
        format!(
            "Config reloaded successfully\n\n{}",
            cfg.pretty_print(
                Some(info.idle_time),
                Some(info.uptime),
                Some(info.paused),
                Some(info.manually_inhibited),
                Some(info.app_blocking),
                Some(info.media_blocking),
                Some(info.media_bridge_active),
                info.profile.as_deref(),
                profiles
            )
        )
    } else {
        "Config reloaded successfully".to_string()
    }
}
