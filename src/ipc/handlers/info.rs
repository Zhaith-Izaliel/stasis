use std::sync::Arc;
use tokio::time::Duration;
use crate::{
    core::{
        manager::{Manager, helpers::current_profile},
        services::app_inhibit::AppInhibitor,
        utils::format_duration,
    },
};

/// Handles the "info" command - displays current state
pub async fn handle_info(
    manager: Arc<tokio::sync::Mutex<Manager>>,
    app_inhibitor: Arc<tokio::sync::Mutex<AppInhibitor>>,
    as_json: bool,
) -> String {
    let mut retry_count = 0;
    let max_retries = 5;
    
    loop {
        match manager.try_lock() {
            Ok(mut mgr) => {
                let state = collect_state(&mut mgr).await;
                drop(mgr);
                
                let app_blocking = check_app_blocking(Arc::clone(&app_inhibitor)).await;
                
                return if as_json {
                    format_json_response(&state, app_blocking)
                } else {
                    format_text_response(&state, app_blocking)
                };
            }
            Err(_) => {
                retry_count += 1;
                if retry_count >= max_retries {
                    return if as_json {
                        serde_json::json!({
                            "text": "",
                            "alt": "not_running",
                            "tooltip": "Busy, try again",
                            "profile": null
                        }).to_string()
                    } else {
                        "Manager is busy, try again".to_string()
                    };
                }
                tokio::time::sleep(Duration::from_millis(20)).await;
            }
        }
    }
}

struct StateSnapshot {
    idle_time: Duration,
    uptime: Duration,
    manually_inhibited: bool,
    paused: bool,
    media_blocking: bool,
    media_bridge_active: bool,
    cfg: Option<Arc<crate::config::model::StasisConfig>>,
    profile: Option<String>,
}

async fn collect_state(mgr: &mut Manager) -> StateSnapshot {
    StateSnapshot {
        idle_time: mgr.state.timing.last_activity.elapsed(),
        uptime: mgr.state.timing.start_time.elapsed(),
        manually_inhibited: mgr.state.inhibitors.manually_paused,
        paused: mgr.state.inhibitors.paused,
        media_blocking: mgr.state.media.media_blocking,
        media_bridge_active: mgr.state.media.media_bridge_active,
        cfg: mgr.state.cfg.clone(),
        profile: current_profile(mgr),
    }
}

async fn check_app_blocking(app_inhibitor: Arc<tokio::sync::Mutex<AppInhibitor>>) -> bool {
    tokio::time::timeout(
        Duration::from_millis(100),
        async {
            let mut inhibitor = app_inhibitor.lock().await;
            inhibitor.is_any_app_running().await
        }
    ).await.unwrap_or(false)
}

fn format_json_response(state: &StateSnapshot, app_blocking: bool) -> String {
    let idle_inhibited = state.paused || app_blocking || state.manually_inhibited;
    
    let (text, icon) = if state.manually_inhibited {
        ("Inhibited", "manually_inhibited")
    } else if idle_inhibited {
        ("Blocked", "idle_inhibited")
    } else {
        ("Active", "idle_active")
    };
    
    // Format profile for display
    let profile_display = state.profile.as_deref().unwrap_or("base");
    
    serde_json::json!({
        "text": text,
        "alt": icon,
        "tooltip": format!(
            "{}\nIdle time: {}\nUptime: {}\nPaused: {}\nManually paused: {}\nApp blocking: {}\nMedia blocking: {}\nProfile: {}",
            if idle_inhibited { "Idle inhibited" } else { "Idle active" },
            format_duration(state.idle_time),
            format_duration(state.uptime),
            state.paused,
            state.manually_inhibited,
            app_blocking,
            state.media_blocking,
            profile_display
        ),
        // Add profile as separate field for easy Waybar access
        "profile": state.profile.as_deref().unwrap_or("base"),
        // Add other useful fields for Waybar
        "idle_time_secs": state.idle_time.as_secs(),
        "uptime_secs": state.uptime.as_secs(),
        "paused": state.paused,
        "manually_inhibited": state.manually_inhibited,
        "app_blocking": app_blocking,
        "media_blocking": state.media_blocking,
        "idle_inhibited": idle_inhibited
    })
    .to_string()
}

fn format_text_response(state: &StateSnapshot, app_blocking: bool) -> String {
    let idle_inhibited = state.paused || app_blocking || state.manually_inhibited;
    
    if let Some(cfg) = &state.cfg {
        let mut info = cfg.pretty_print(
            Some(state.idle_time),
            Some(state.uptime),
            Some(idle_inhibited),
            Some(state.manually_inhibited),
            Some(app_blocking),
            Some(state.media_blocking),
            Some(state.media_bridge_active)
        );
        
        if let Some(p) = &state.profile {
            info.push_str(&format!("\n\nActive profile: {}", p));
        } else {
            info.push_str("\n\nActive profile: base config");
        }
        
        info
    } else {
        "No configuration loaded".to_string()
    }
}
