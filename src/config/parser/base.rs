use eyre::Result;
use regex::Regex;
use rune_cfg::{RuneConfig, Value};

use crate::{
    config::model::*,
    core::utils::{ChassisKind, detect_chassis},
    sdebug,
    sinfo,
};

use super::actions::collect_actions;
use super::pattern::parse_app_pattern;

/// Parses the base stasis configuration from a RuneConfig
pub fn parse_base_stasis_config(config: &RuneConfig) -> Result<StasisConfig> {
    let pre_suspend_command = config
        .get::<String>("stasis.pre_suspend_command")
        .or_else(|_| config.get::<String>("stasis.pre-suspend-command"))
        .ok();

    let monitor_media = config
        .get::<bool>("stasis.monitor_media")
        .or_else(|_| config.get::<bool>("stasis.monitor-media"))
        .unwrap_or(true);

    let ignore_remote_media = config
        .get::<bool>("stasis.ignore_remote_media")
        .or_else(|_| config.get::<bool>("stasis.ignore-remote-media"))
        .unwrap_or(true);

    let media_blacklist: Vec<String> = config
        .get("stasis.media_blacklist")
        .or_else(|_| config.get("stasis.media-blacklist"))
        .unwrap_or_default();
    
    let media_blacklist: Vec<String> = media_blacklist
        .into_iter()
        .map(|s| s.to_lowercase())
        .collect();

    let respect_wayland_inhibitors = config
        .get::<bool>("stasis.respect_wayland_inhibitors")
        .or_else(|_| config.get::<bool>("stasis.respect-wayland-inhibitors"))
        .unwrap_or(true);

    let notify_on_unpause = config
        .get::<bool>("stasis.notify_on_unpause")
        .or_else(|_| config.get::<bool>("stasis.notify-on-unpause"))
        .unwrap_or(false);

    let lid_close_action = config
        .get::<String>("stasis.lid_close_action")
        .or_else(|_| config.get::<String>("stasis.lid-close-action"))
        .ok()
        .map(|s| match s.trim() {
            "ignore" => LidCloseAction::Ignore,
            "lock_screen" | "lock-screen" => LidCloseAction::LockScreen,
            "suspend" => LidCloseAction::Suspend,
            other => LidCloseAction::Custom(other.to_string()),
        })
        .unwrap_or(LidCloseAction::Ignore);

    let lid_open_action = config
        .get::<String>("stasis.lid_open_action")
        .or_else(|_| config.get::<String>("stasis.lid-open-action"))
        .ok()
        .map(|s| match s.trim() {
            "ignore" => LidOpenAction::Ignore,
            "wake" => LidOpenAction::Wake,
            other => LidOpenAction::Custom(other.to_string()),
        })
        .unwrap_or(LidOpenAction::Ignore);

    let debounce_seconds = config
        .get::<u8>("stasis.debounce_seconds")
        .or_else(|_| config.get::<u8>("stasis.debounce-seconds"))
        .unwrap_or(0u8);

    let notify_before_action = config
        .get::<bool>("stasis.notify_before_action")
        .or_else(|_| config.get::<bool>("stasis.notify-before-action"))
        .unwrap_or(false);

    let notify_seconds_before = config
        .get::<u64>("stasis.notify_seconds_before")
        .or_else(|_| config.get::<u64>("stasis.notify-seconds-before"))
        .unwrap_or(0);

    let lock_detection_type = config
        .get::<String>("stasis.lock_detection_type")
        .or_else(|_| config.get::<String>("stasis.lock-detection-type"))
        .ok()
        .map(|s| match s.trim().to_lowercase().as_str() {
            "logind" => LockDetectionType::Logind,
            _ => LockDetectionType::Process,
        })
        .unwrap_or(LockDetectionType::Process);

    let inhibit_apps: Vec<AppInhibitPattern> = config
        .get_value("stasis.inhibit_apps")
        .or_else(|_| config.get_value("stasis.inhibit-apps"))
        .ok()
        .and_then(|v| match v {
            Value::Array(arr) => Some(
                arr.iter()
                    .filter_map(|v| match v {
                        Value::String(s) => parse_app_pattern(s).ok(),
                        Value::Regex(s) => Regex::new(s).ok().map(AppInhibitPattern::Regex),
                        _ => None,
                    })
                    .collect(),
            ),
            _ => None,
        })
        .unwrap_or_default();

    let chassis = detect_chassis();
    let actions = match chassis {
        ChassisKind::Laptop => {
            let mut all = Vec::new();
            
            let ac_actions = collect_actions(config, "stasis.on_ac")?
                .into_iter()
                .map(|mut a| {
                    a.name = format!("ac.{}", a.name);
                    a
                });
            all.extend(ac_actions);
            
            let battery_actions = collect_actions(config, "stasis.on_battery")?
                .into_iter()
                .map(|mut a| {
                    a.name = format!("battery.{}", a.name);
                    a
                });
            all.extend(battery_actions);
            
            all
        }
        ChassisKind::Desktop => collect_actions(config, "stasis")?,
    };
  
   

    log_config_debug(
        &pre_suspend_command,
        monitor_media,
        ignore_remote_media,
        &media_blacklist,
        respect_wayland_inhibitors,
        notify_on_unpause,
        notify_before_action,
        notify_seconds_before,
        debounce_seconds,
        &lid_close_action,
        &lid_open_action,
        &lock_detection_type,
        &inhibit_apps,
        &actions,
    );

    let mut actions = actions;
    actions.sort_by_key(|a| a.timeout != 0);

    let mut actions = actions;
    actions.sort_by_key(|a| a.timeout != 0);

    // Non-debug message
    if !actions.is_empty() {
        let action_names: Vec<String> = actions.iter().map(|a| a.name.clone()).collect();
        sinfo!("Stasis", "Config loaded with actions: [{}]", action_names.join(", "));
    } else {
        sinfo!("Stasis", "Config loaded with no actions.");
    }

    Ok(StasisConfig {
        actions,
        pre_suspend_command,
        monitor_media,
        media_blacklist,
        ignore_remote_media,
        respect_wayland_inhibitors,
        inhibit_apps,
        debounce_seconds,
        lid_close_action,
        lid_open_action,
        notify_on_unpause,
        notify_before_action,
        notify_seconds_before,
        lock_detection_type,
    })
}

fn log_config_debug(
    pre_suspend_command: &Option<String>,
    monitor_media: bool,
    ignore_remote_media: bool,
    media_blacklist: &[String],
    respect_wayland_inhibitors: bool,
    notify_on_unpause: bool,
    notify_before_action: bool,
    notify_seconds_before: u64,
    debounce_seconds: u8,
    lid_close_action: &LidCloseAction,
    lid_open_action: &LidOpenAction,
    lock_detection_type: &LockDetectionType,
    inhibit_apps: &[AppInhibitPattern],
    actions: &[IdleActionBlock],
) {
    sdebug!("Config", "Parsed Config:");
    sdebug!("Config", "  pre_suspend_command = {:?}", pre_suspend_command);
    sdebug!("Config", "  monitor_media = {:?}", monitor_media);
    sdebug!("Config", "  ignore_remote_media = {:?}", ignore_remote_media);
    sdebug!(
        "Config",
        "  media_blacklist = [{}]",
        media_blacklist.join(", ")
    );
    sdebug!("Config", "  respect_wayland_inhibitors = {:?}", respect_wayland_inhibitors);
    sdebug!("Config", "  notify_on_unpause = {:?}", notify_on_unpause);
    sdebug!("Config", "  notify_before_action = {:?}", notify_before_action);
    sdebug!("Config", "  notify_seconds_before = {:?}", notify_seconds_before);
    sdebug!("Config", "  debounce_seconds = {:?}", debounce_seconds);
    sdebug!("Config", "  lid_close_action = {:?}", lid_close_action);
    sdebug!("Config", "  lid_open_action = {:?}", lid_open_action);
    sdebug!("Config", "  lock_detection_type = {:?}", lock_detection_type);
    sdebug!(
        "Config",
        "  inhibit_apps = [{}]",
        inhibit_apps.iter().map(|p| p.to_string()).collect::<Vec<_>>().join(", ")
    );
    sdebug!("Stasis", "  actions:");
    for action in actions {
        let mut details = format!(
            "    {}: timeout={}s, command=\"{}\"",
            action.name, action.timeout, action.command
        );

        if let Some(lock_cmd) = &action.lock_command {
            details.push_str(&format!(", lock_command=\"{}\"", lock_cmd));
        } 
        if let Some(resume_cmd) = &action.resume_command {
            details.push_str(&format!(", resume_command=\"{}\"", resume_cmd));
        }
        if let Some(notification) = &action.notification {
            details.push_str(&format!(", notification=\"{}\"", notification));
        }
        sdebug!("Stasis", "{}", &details);
    }
}
