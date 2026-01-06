use std::sync::Arc;
use tokio::time::Duration;
use crate::{
    core::{
        manager::{Manager, helpers::{current_profile, list_profiles}},
        utils::format_duration,
    },
    config::info::InfoSections,
};

pub struct StateInfo {
    pub idle_time: Duration,
    pub uptime: Duration,
    pub manually_inhibited: bool,
    pub paused: bool,
    pub media_blocking: bool,
    pub media_bridge_active: bool,
    pub app_blocking: bool,
    pub cfg: Option<Arc<crate::config::model::StasisConfig>>,
    pub profile: Option<String>,
    pub available_profiles: Vec<String>,
}

/// Collects comprehensive state information from manager (async)
pub async fn collect_full_state(
    manager: Arc<tokio::sync::Mutex<Manager>>,
) -> StateInfo {
    let mut mgr = manager.lock().await;
    collect_manager_state(&mut mgr)
}

/// Collects state information from manager only (sync)
pub fn collect_manager_state(mgr: &mut Manager) -> StateInfo {
    let app_blocking = mgr.state.inhibitors.active_app_inhibitors > 0;
    
    StateInfo {
        idle_time: mgr.state.timing.last_activity.elapsed(),
        uptime: mgr.state.timing.start_time.elapsed(),
        manually_inhibited: mgr.state.inhibitors.manually_paused,
        paused: mgr.state.inhibitors.paused,
        media_blocking: mgr.state.media.media_blocking,
        media_bridge_active: mgr.state.media.media_bridge_active,
        app_blocking,
        cfg: mgr.state.cfg.clone(),
        profile: current_profile(mgr),
        available_profiles: list_profiles(mgr),
    }
}

/// Formats state info into a pretty-printed text string
pub fn format_text_response(info: &StateInfo, sections: InfoSections) -> String {  // CHANGED SIGNATURE
    if let Some(cfg) = &info.cfg {
        let profiles = if info.available_profiles.is_empty() {
            None
        } else {
            Some(info.available_profiles.as_slice())
        };
        
        cfg.pretty_print(
            Some(info.idle_time),
            Some(info.uptime),
            Some(info.paused),
            Some(info.manually_inhibited),
            Some(info.app_blocking),
            Some(info.media_blocking),
            Some(info.media_bridge_active),
            info.profile.as_deref(),
            profiles,
            sections,  // ADD THIS
        )
    } else {
        "No configuration loaded".to_string()
    }
}

/// Formats state info into JSON (for Waybar, etc.)
pub fn format_json_response(info: &StateInfo) -> String {
    let idle_inhibited = info.paused || info.app_blocking || info.manually_inhibited;
    
    let (text, icon) = if info.manually_inhibited {
        ("Inhibited", "manually_inhibited")
    } else if idle_inhibited {
        ("Blocked", "idle_inhibited")
    } else {
        ("Active", "idle_active")
    };
    
    let profile_display = info.profile.as_deref().unwrap_or("base");
    
    serde_json::json!({
        "text": text,
        "alt": icon,
        "tooltip": format!(
            "{}\nIdle time: {}\nUptime: {}\nPaused: {}\nManually paused: {}\nApp blocking: {}\nMedia blocking: {}\nProfile: {}",
            if idle_inhibited { "Idle inhibited" } else { "Idle active" },
            format_duration(info.idle_time),
            format_duration(info.uptime),
            info.paused,
            info.manually_inhibited,
            info.app_blocking,
            info.media_blocking,
            profile_display
        ),
        "profile": profile_display,
        "idle_time_secs": info.idle_time.as_secs(),
        "uptime_secs": info.uptime.as_secs(),
        "paused": info.paused,
        "manually_inhibited": info.manually_inhibited,
        "app_blocking": info.app_blocking,
        "media_blocking": info.media_blocking,
        "idle_inhibited": idle_inhibited
    })
    .to_string()
}
