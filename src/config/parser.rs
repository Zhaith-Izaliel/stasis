use eyre::{Result, WrapErr};
use regex::Regex;
use rune_cfg::{RuneConfig, Value};
use std::path::PathBuf;

use crate::{
    config::model::*, 
    core::utils::{ChassisKind, detect_chassis}, 
    log::{log_debug_message, log_message},
};

fn parse_app_pattern(s: &str) -> Result<AppInhibitPattern> {
    let regex_meta = ['.', '*', '+', '?', '(', ')', '[', ']', '{', '}', '|', '\\', '^', '$'];
    if s.chars().any(|c| regex_meta.contains(&c)) {
        Ok(AppInhibitPattern::Regex(Regex::new(s).wrap_err("invalid regex in inhibit_apps")?))
    } else {
        Ok(AppInhibitPattern::Literal(s.to_string()))
    }
}

fn is_special_key(key: &str) -> bool {
    matches!(
        key,
        "resume_command" | "resume-command"
            | "pre_suspend_command" | "pre-suspend-command"
            | "monitor_media" | "monitor-media"
            | "ignore_remote_media" | "ignore-remote-media"
            | "respect_wayland_inhibitors" | "respect-wayland-inhibitors"
            | "inhibit_apps" | "inhibit-apps"
            | "debounce_seconds" | "debounce-seconds"
            | "notify_on_unpause" | "notify-on-unpause"
            | "notify_before_action" | "notify-before-action"
            | "notify_seconds_before" | "notify-seconds-before"
            | "lid_close_action" | "lid-close-action"
            | "lid_open_action" | "lid-open-action"
            | "media_blacklist" | "media-blacklist"
            | "lock_detection_type" | "lock-detection-type"
    )
}

fn collect_actions(config: &RuneConfig, path: &str) -> Result<Vec<IdleActionBlock>> {
    let mut actions = Vec::new();

    let keys = config
        .get_keys(path)
        .or_else(|_| config.get_keys(&path.replace('-', "_")))
        .unwrap_or_default();

    for key in keys {
        if is_special_key(&key) {
            continue;
        }

        let command_path = format!("{}.{}.command", path, key);
        let command = match config
            .get::<String>(&command_path)
            .or_else(|_| config.get::<String>(&command_path.replace('-', "_")))
        {
            Ok(c) => c,
            Err(_) => continue,
        };

        let timeout_path = format!("{}.{}.timeout", path, key);
        let timeout = match config
            .get::<u64>(&timeout_path)
            .or_else(|_| config.get::<u64>(&timeout_path.replace('-', "_")))
        {
            Ok(t) => t,
            Err(_) => continue,
        };

        let kind = match key.as_str() {
            "lock_screen" | "lock-screen" => IdleAction::LockScreen,
            "suspend" => IdleAction::Suspend,
            "dpms" => IdleAction::Dpms,
            "brightness" => IdleAction::Brightness,
            _ => IdleAction::Custom,
        };

        let resume_command = config
            .get::<String>(&format!("{}.{}.resume_command", path, key))
            .ok()
            .or_else(|| config.get::<String>(&format!("{}.{}.resume-command", path, key)).ok());

        let lock_command = if kind == IdleAction::LockScreen {
            config
                .get::<String>(&format!("{}.{}.lock_command", path, key))
                .ok()
                .or_else(|| config.get::<String>(&format!("{}.{}.lock-command", path, key)).ok())
        } else {
            None
        };

        let notification = config
            .get::<String>(&format!("{}.{}.notification", path, key))
            .ok();

        actions.push(IdleActionBlock {
            name: key.clone(),
            timeout,
            command,
            kind,
            resume_command,
            lock_command,
            last_triggered: None,
            notification,
        });
    }

    Ok(actions)
}

fn load_merged_config() -> Result<RuneConfig> {
    let internal_default = include_str!("../../examples/stasis.rune");
    let mut config = RuneConfig::from_str(internal_default)
        .wrap_err("failed to parse internal default config")?;

    let user_path = dirs::home_dir()
        .map(|mut p| {
            p.push(".config/stasis/stasis.rune");
            p
        });
    
    let system_path = PathBuf::from("/etc/stasis/stasis.rune");
    let share_path = PathBuf::from("/usr/share/stasis/examples/stasis.rune");

    if let Some(user_path) = user_path {
        if user_path.exists() {
            config = RuneConfig::from_file(&user_path)
                .wrap_err_with(|| format!("failed to load user config from {}", user_path.display()))?;
            log_debug_message(&format!("Loaded config from: {}", user_path.display()));
            return Ok(config);
        }
    }

    if system_path.exists() {
        config = RuneConfig::from_file(&system_path)
            .wrap_err_with(|| format!("failed to load system config from {}", system_path.display()))?;
        log_debug_message(&format!("Loaded config from: {}", system_path.display()));
        return Ok(config);
    }

    if share_path.exists() {
        config = RuneConfig::from_file(&share_path)
            .wrap_err_with(|| format!("failed to load shared example config from {}", share_path.display()))?;
        log_debug_message(&format!("Loaded config from: {}", share_path.display()));
        return Ok(config);
    }

    log_debug_message("Using internal default configuration");
    Ok(config)
}

fn parse_base_stasis_config(config: &RuneConfig) -> Result<StasisConfig> {
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
  
    if actions.is_empty() {
        // don't fail — load an empty action list and let the runtime decide what to do
        log_message("No valid idle actions found in base config; continuing with empty actions.");
        // optionally: return Err if you want to force at least one action in some modes
    }

    log_debug_message("Parsed Config:");
    log_debug_message(&format!("  pre_suspend_command = {:?}", pre_suspend_command));
    log_debug_message(&format!("  monitor_media = {:?}", monitor_media));
    log_debug_message(&format!("  ignore_remote_media = {:?}", ignore_remote_media));
    log_debug_message(&format!(
        "  media_blacklist = [{}]",
        media_blacklist.join(", ")
    ));
    log_debug_message(&format!("  respect_wayland_inhibitors = {:?}", respect_wayland_inhibitors));
    log_debug_message(&format!("  notify_on_unpause = {:?}", notify_on_unpause));
    log_debug_message(&format!("  notify_before_action = {:?}", notify_before_action));
    log_debug_message(&format!("  notify_seconds_before = {:?}", notify_seconds_before));
    log_debug_message(&format!("  debounce_seconds = {:?}", debounce_seconds));
    log_debug_message(&format!("  lid_close_action = {:?}", lid_close_action));
    log_debug_message(&format!("  lid_open_action = {:?}", lid_open_action));
    log_debug_message(&format!("  lock_detection_type = {:?}", lock_detection_type));
    log_debug_message(&format!(
        "  inhibit_apps = [{}]",
        inhibit_apps.iter().map(|p| p.to_string()).collect::<Vec<_>>().join(", ")
    ));
    log_debug_message("  actions:");
    for action in &actions {
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
        log_debug_message(&details);
    }

    let mut actions = actions;
    actions.sort_by_key(|a| a.timeout != 0);

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

fn parse_profile(config: &RuneConfig, profile_name: &str, _base: &StasisConfig) -> Result<Profile> {
    let base_path = format!("profiles.{}", profile_name);

    // Actions
    let actions = collect_actions(config, &base_path)?;
    if actions.is_empty() {
        log_debug_message(&format!("Profile '{}' defines no actions; proceeding with empty actions.", profile_name));
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

fn load_profiles(config: &RuneConfig, base: &StasisConfig) -> Result<Vec<Profile>> {
    let profile_keys = config
        .get_keys("profiles")
        .unwrap_or_default();

    let mut profiles = Vec::new();
    
    for profile_name in profile_keys {
        if is_special_key(&profile_name) {
            continue;
        }
        
        match parse_profile(config, &profile_name, base) {
            Ok(profile) => {
                log_debug_message(&format!("Loaded profile: {}", profile_name));
                profiles.push(profile);
            }
            Err(e) => {
                log_debug_message(&format!("Failed to load profile '{}': {}", profile_name, e));
            }
        }
    }

    Ok(profiles)
}

pub fn load_config() -> Result<StasisConfig> {
    let config = load_merged_config().wrap_err("failed to load configuration")?;
    parse_base_stasis_config(&config)
}

pub fn load_combined_config() -> Result<CombinedConfig> {
    let config = load_merged_config().wrap_err("failed to load configuration")?;
    let base = parse_base_stasis_config(&config)?;
    let profiles = load_profiles(&config, &base)?;
    
    if !profiles.is_empty() {
        let profile_names: Vec<_> = profiles.iter().map(|p| p.name.as_str()).collect();
        log_debug_message(&format!("  profiles: {}", profile_names.join(", ")));
    }
    
    Ok(CombinedConfig {
        base,
        profiles,
        active_profile: None,
    })
}
