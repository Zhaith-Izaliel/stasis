use regex::Regex;
use rune_cfg::{RuneConfig, Value};

use crate::{
    config::model::*,
    serror,
    sinfo,
};

use super::actions::{collect_actions, is_special_key};
use super::pattern::parse_app_pattern;
use super::config::ConfigParseError;

/// Parses a single profile from the configuration
pub fn parse_profile(config: &RuneConfig, profile_name: &str, _base: &StasisConfig) -> Result<Profile, ConfigParseError> {
    let base_path = format!("profiles.{}", profile_name);

    // Actions
    let actions = collect_actions(config, &base_path)?;
    if actions.is_empty() {
        sinfo!("Stasis", "Profile '{}' defines has no actions.", profile_name);
    }

    // Primitive fields: fallback to 'empty' values if undefined
    let debounce_seconds = config
        .get::<u8>(&format!("{}.debounce_seconds", base_path))
        .or_else(|_| config.get::<u8>(&format!("{}.debounce-seconds", base_path)))
        .unwrap_or(0);

    let monitor_media = config
        .get::<bool>(&format!("{}.monitor_media", base_path))
        .or_else(|_| config.get::<bool>(&format!("{}.monitor-media", base_path)))
        .unwrap_or(false);

    let ignore_remote_media = config
        .get::<bool>(&format!("{}.ignore_remote_media", base_path))
        .or_else(|_| config.get::<bool>(&format!("{}.ignore-remote-media", base_path)))
        .unwrap_or(false);
 
    let media_blacklist: Vec<String> = config
        .get::<Vec<String>>(&format!("{}.media_blacklist", base_path))
        .or_else(|_| config.get::<Vec<String>>(&format!("{}.media-blacklist", base_path)))
        .unwrap_or_default()
        .into_iter()
        .map(|s| s.to_lowercase())
        .collect();

    let respect_wayland_inhibitors = config
        .get::<bool>(&format!("{}.respect_wayland_inhibitors", base_path))
        .or_else(|_| config.get::<bool>(&format!("{}.respect-wayland-inhibitors", base_path)))
        .unwrap_or(false);
    
    let pre_suspend_command = config
        .get::<String>(&format!("{}.pre_suspend_command", base_path))
        .or_else(|_| config.get::<String>(&format!("{}.pre-suspend-command", base_path)))
        .ok();

    let notify_on_unpause = config
        .get::<bool>(&format!("{}.notify_on_unpause", base_path))
        .or_else(|_| config.get::<bool>(&format!("{}.notify-on-unpause", base_path)))
        .unwrap_or(false);

    let notify_before_action = config
        .get::<bool>(&format!("{}.notify_before_action", base_path))
        .or_else(|_| config.get::<bool>(&format!("{}.notify-before-action", base_path)))
        .unwrap_or(false);

    let notify_seconds_before = config
        .get::<u64>(&format!("{}.notify_seconds_before", base_path))
        .or_else(|_| config.get::<u64>(&format!("{}.notify-seconds-before", base_path)))
        .unwrap_or(0);

    let lid_close_action = config
        .get::<String>(&format!("{}.lid_close_action", base_path))
        .or_else(|_| config.get::<String>(&format!("{}.lid-close-action", base_path)))
        .ok()
        .map(|s| match s.trim() {
            "ignore" => LidCloseAction::Ignore,
            "lock_screen" | "lock-screen" => LidCloseAction::LockScreen,
            "suspend" => LidCloseAction::Suspend,
            other => LidCloseAction::Custom(other.to_string()),
        })
        .unwrap_or(LidCloseAction::Ignore);

    let lid_open_action = config
        .get::<String>(&format!("{}.lid_open_action", base_path))
        .or_else(|_| config.get::<String>(&format!("{}.lid-open-action", base_path)))
        .ok()
        .map(|s| match s.trim() {
            "ignore" => LidOpenAction::Ignore,
            "wake" => LidOpenAction::Wake,
            other => LidOpenAction::Custom(other.to_string()),
        })
        .unwrap_or(LidOpenAction::Ignore);

    let lock_detection_type = config
        .get::<String>(&format!("{}.lock_detection_type", base_path))
        .or_else(|_| config.get::<String>(&format!("{}.lock-detection-type", base_path)))
        .ok()
        .map(|s| match s.trim().to_lowercase().as_str() {
            "logind" => LockDetectionType::Logind,
            _ => LockDetectionType::Process,
        })
        .unwrap_or(LockDetectionType::Process);

    let inhibit_apps: Vec<AppInhibitPattern> = config
        .get_value(&format!("{}.inhibit_apps", base_path))
        .or_else(|_| config.get_value(&format!("{}.inhibit-apps", base_path)))
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

    Ok(Profile {
        name: profile_name.to_string(),
        actions,
        debounce_seconds,
        inhibit_apps,
        monitor_media,
        ignore_remote_media,
        media_blacklist,
        pre_suspend_command,
        respect_wayland_inhibitors,
        lid_close_action,
        lid_open_action,
        notify_on_unpause,
        notify_before_action,
        notify_seconds_before,
        lock_detection_type,
    })
}

/// Loads all profiles from the configuration
pub fn load_profiles(config: &RuneConfig, base: &StasisConfig) -> Result<Vec<Profile>, ConfigParseError> {
    let profile_keys = config
        .get_keys("profiles")
        .unwrap_or_default();

    let mut profiles = Vec::new();
    
    for profile_name in profile_keys {
        if is_special_key(&profile_name) {
            continue;
        }
        
        match parse_profile(config, &profile_name, base) {
            Ok(profile) => profiles.push(profile),
            Err(e) => {
                serror!("Stasis", "Failed to load profile '{}': {}", profile_name, e);
            }
        }
    }

    Ok(profiles)
}
