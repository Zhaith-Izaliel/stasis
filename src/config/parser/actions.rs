use eyre::Result;
use rune_cfg::RuneConfig;

use crate::config::model::{IdleActionBlock, IdleAction};

/// Checks if a key is a special configuration key that shouldn't be treated as an action
pub fn is_special_key(key: &str) -> bool {
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

/// Collects all idle action blocks from a given configuration path
pub fn collect_actions(config: &RuneConfig, path: &str) -> Result<Vec<IdleActionBlock>> {
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

        let notify_seconds_before = config
            .get::<u64>(&format!("{}.{}.notify_seconds_before", path, key))
            .ok()
            .or_else(|| config.get::<u64>(&format!("{}.{}.notify-seconds-before", path, key)).ok());

        actions.push(IdleActionBlock {
            name: key.clone(),
            timeout,
            command,
            kind,
            resume_command,
            lock_command,
            last_triggered: None,
            notification,
            notify_seconds_before,
        });
    }

    Ok(actions)
}
