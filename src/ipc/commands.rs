use std::{
    sync::Arc,
    time::{Duration, Instant},
};
use tokio::time::sleep;
use tokio::sync::Mutex;

use crate::{
    core::manager::{helpers::{run_action, trigger_pre_suspend}, Manager},
    log::log_message,
};

/// Helper function to strip ac. or battery. prefix from action names
fn strip_action_prefix(name: &str) -> &str {
    name.strip_prefix("ac.")
        .or_else(|| name.strip_prefix("battery."))
        .unwrap_or(name)
}

pub async fn trigger_action_by_name(manager: Arc<Mutex<Manager>>, name: &str) -> Result<String, String> {
    let normalized = name.replace('_', "-").to_lowercase();
    let mut mgr = manager.lock().await;

    if normalized == "pre-suspend" || normalized == "presuspend" {
        trigger_pre_suspend(&mut mgr).await;
        return Ok("pre_suspend".to_string());
    }

    // Check if user is explicitly targeting a specific block (e.g., "ac.dim" or "battery.suspend")
    let (target_block, search_name) = if normalized.starts_with("ac.") {
        (Some("ac"), normalized.strip_prefix("ac.").unwrap())
    } else if normalized.starts_with("battery.") {
        (Some("battery"), normalized.strip_prefix("battery.").unwrap())
    } else {
        (None, normalized.as_str())
    };

    // Determine which block to search
    let block = if let Some(explicit_block) = target_block {
        // User explicitly specified ac. or battery.
        match explicit_block {
            "ac" => &mgr.state.ac_actions,
            "battery" => &mgr.state.battery_actions,
            _ => &mgr.state.default_actions,
        }
    } else if !mgr.state.ac_actions.is_empty() || !mgr.state.battery_actions.is_empty() {
        // Auto-detect based on current power state
        match mgr.state.on_battery() {
            Some(true) => &mgr.state.battery_actions,
            Some(false) => &mgr.state.ac_actions,
            None => &mgr.state.default_actions,
        }
    } else {
        &mgr.state.default_actions
    };

    let action_opt = block.iter().find(|a| {
        let kind_name = format!("{:?}", a.kind).to_lowercase().replace('_', "-");
        let stripped_name = strip_action_prefix(&a.name).to_lowercase();
        kind_name == search_name || stripped_name == search_name || a.name.to_lowercase() == search_name
    });

    let action = match action_opt {
        Some(a) => a.clone(),
        None => {
            let mut available: Vec<String> = block.iter()
                .map(|a| strip_action_prefix(&a.name).to_string())
                .collect();
            if mgr.state.pre_suspend_command.is_some() {
                available.push("pre_suspend".to_string());
            }
            available.sort();
            return Err(format!(
                "Action '{}' not found. Available actions: {}",
                name,
                available.join(", ")
            ));
        }
    };

    log_message(&format!("Action triggered: '{}'", strip_action_prefix(&action.name)));
    let is_lock = matches!(action.kind, crate::config::model::IdleAction::LockScreen);

    if is_lock {
        // Mark lock state and notify watcher
        mgr.state.lock_state.is_locked = true;
        mgr.state.lock_state.post_advanced = false;
        mgr.state.lock_state.command = Some(action.command.clone());
        mgr.state.lock_notify.notify_one();

        // Run the lock command
        run_action(&mut mgr, &action).await;

        // Mark as advanced past lock
        mgr.advance_past_lock().await;

        // ---- Mirror reset() behavior exactly for timers ----
        let now = Instant::now();
        if let Some(cfg) = &mgr.state.cfg {
            let debounce = Duration::from_secs(cfg.debounce_seconds as u64);
            mgr.state.last_activity = now;
            mgr.state.debounce = Some(now + debounce);

            // Clear last_triggered for all actions
            {
                let actions = &mut mgr.state.default_actions;
                for a in actions.iter_mut() {
                    a.last_triggered = None;
                }
            }
            {
                let actions = &mut mgr.state.ac_actions;
                for a in actions.iter_mut() {
                    a.last_triggered = None;
                }
            }
            {
                let actions = &mut mgr.state.battery_actions;
                for a in actions.iter_mut() {
                    a.last_triggered = None;
                }
            }

            // Determine active block name first
            let active_block = if !mgr.state.ac_actions.is_empty() || !mgr.state.battery_actions.is_empty() {
                match mgr.state.on_battery() {
                    Some(true) => "battery",
                    Some(false) => "ac",
                    None => "default",
                }
            } else {
                "default"
            };

            // Now isolate block mutation
            {
                let actions = match active_block {
                    "ac" => &mut mgr.state.ac_actions,
                    "battery" => &mut mgr.state.battery_actions,
                    _ => &mut mgr.state.default_actions,
                };

                // Recalculate action index
                let mut next_index = actions
                    .iter()
                    .position(|a| a.last_triggered.is_none())
                    .unwrap_or_else(|| actions.len().saturating_sub(1));

                // If lock action exists, skip past it so next timer continues properly
                if let Some(lock_index) =
                    actions.iter().position(|a| matches!(a.kind, crate::config::model::IdleAction::LockScreen))
                {
                    if next_index <= lock_index {
                        next_index = lock_index.saturating_add(1);

                        let debounce_end = now + debounce;
                        if next_index < actions.len() {
                            actions[next_index].last_triggered = Some(debounce_end);
                        }

                        mgr.state.lock_state.post_advanced = true;
                    }
                }

                mgr.state.action_index = next_index;
            }
        }

        // Wake idle loop to recalculate timers
        mgr.state.notify.notify_one();
    } else {
        run_action(&mut mgr, &action).await;
    }

    Ok(strip_action_prefix(&action.name).to_string())
}

pub async fn list_available_actions(manager: Arc<Mutex<Manager>>) -> Vec<String> {
    let mgr = manager.lock().await;
    let mut actions = mgr
        .state
        .default_actions
        .iter()
        .map(|a| strip_action_prefix(&a.name).to_string())
        .collect::<Vec<_>>();

    if mgr.state.pre_suspend_command.is_some() {
        actions.push("pre_suspend".to_string());
    }

    actions.sort();
    actions
}

const PAUSE_HELP_MESSAGE: &str = r#"Pause all timers indefinitely or for a specific duration

Usage: 
  stasis pause              Pause indefinitely until 'resume' is called
  stasis pause <DURATION>   Pause for a specific duration, then auto-resume

Duration format:
  You can specify durations using combinations of:
    - s, sec, seconds (e.g., 30s)
    - m, min, minutes (e.g., 5m)
    - h, hr, hours    (e.g., 2h)

Examples:
  stasis pause 5m           Pause for 5 minutes
  stasis pause 1h 30m       Pause for 1 hour and 30 minutes
  stasis pause 2h 15m 30s   Pause for 2 hours, 15 minutes, and 30 seconds
  stasis pause 30s          Pause for 30 seconds

Use 'stasis resume' to manually resume before the timer expires."#;

/// Parse a duration string like "5m", "1h", "30s", or "1h 30m 15s"
fn parse_duration(s: &str) -> Result<Duration, String> {
    // Special case: if someone types "help", show help message
    let trimmed = s.trim();
    if trimmed.eq_ignore_ascii_case("help") || trimmed == "-h" || trimmed == "--help" {
        return Err(PAUSE_HELP_MESSAGE.to_string());
    }

    let parts: Vec<&str> = s.split_whitespace().collect();
    let mut total_secs = 0u64;

    for part in parts {
        let part = part.trim();
        if part.is_empty() {
            continue;
        }

        // Find where the number ends and unit begins
        let split_pos = part
            .chars()
            .position(|c| !c.is_ascii_digit())
            .ok_or_else(|| format!("Invalid duration format: '{}' (missing unit)", part))?;

        let (num_str, unit) = part.split_at(split_pos);
        let num: u64 = num_str
            .parse()
            .map_err(|_| format!("Invalid number: '{}'", num_str))?;

        let multiplier = match unit.to_lowercase().as_str() {
            "s" | "sec" | "secs" | "second" | "seconds" => 1,
            "m" | "min" | "mins" | "minute" | "minutes" => 60,
            "h" | "hr" | "hrs" | "hour" | "hours" => 3600,
            _ => return Err(format!("Unknown time unit: '{}' (use s, m, or h)", unit)),
        };

        total_secs += num * multiplier;
    }

    if total_secs == 0 {
        return Err("Duration must be greater than 0".to_string());
    }

    Ok(Duration::from_secs(total_secs))
}

/// Pause the manager for a specific duration, then automatically resume
pub async fn pause_for_duration(
    manager: Arc<Mutex<Manager>>,
    duration_str: &str,
) -> Result<String, String> {
    let duration = parse_duration(duration_str)?;
    
    // Pause immediately
    {
        let mut mgr = manager.lock().await;
        mgr.pause(true).await;
    }

    let secs = duration.as_secs();
    let hours = secs / 3600;
    let mins = (secs % 3600) / 60;
    let remaining_secs = secs % 60;

    let time_str = if hours > 0 && mins > 0 && remaining_secs > 0 {
        format!("{}h {}m {}s", hours, mins, remaining_secs)
    } else if hours > 0 && mins > 0 {
        format!("{}h {}m", hours, mins)
    } else if hours > 0 && remaining_secs > 0 {
        format!("{}h {}s", hours, remaining_secs)
    } else if mins > 0 && remaining_secs > 0 {
        format!("{}m {}s", mins, remaining_secs)
    } else if hours > 0 {
        format!("{}h", hours)
    } else if mins > 0 {
        format!("{}m", mins)
    } else {
        format!("{}s", remaining_secs)
    };

    log_message(&format!("Idle manager paused for {}", time_str));

    // Clone time_str for the spawned task
    let time_str_clone = time_str.clone();
    
    // Spawn a task to auto-resume after duration
    tokio::spawn(async move {
        sleep(duration).await;
        let mut mgr = manager.lock().await;
        mgr.resume(true).await;
        log_message(&format!("Auto-resuming after {} pause", time_str_clone));
    });

    Ok(format!("Paused for {}", time_str))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_duration() {
        assert_eq!(parse_duration("5m").unwrap(), Duration::from_secs(300));
        assert_eq!(parse_duration("1h").unwrap(), Duration::from_secs(3600));
        assert_eq!(parse_duration("30s").unwrap(), Duration::from_secs(30));
        assert_eq!(parse_duration("1h 30m").unwrap(), Duration::from_secs(5400));
        assert_eq!(parse_duration("1h 30m 15s").unwrap(), Duration::from_secs(5415));
        assert_eq!(parse_duration("2h 15s").unwrap(), Duration::from_secs(7215));
        
        // Test various unit formats
        assert_eq!(parse_duration("5mins").unwrap(), Duration::from_secs(300));
        assert_eq!(parse_duration("1hour").unwrap(), Duration::from_secs(3600));
        assert_eq!(parse_duration("30seconds").unwrap(), Duration::from_secs(30));
        
        // Test errors
        assert!(parse_duration("").is_err());
        assert!(parse_duration("5").is_err());
        assert!(parse_duration("5x").is_err());
        assert!(parse_duration("0m").is_err());
        
        // Test help detection
        assert!(parse_duration("help").is_err());
        assert!(parse_duration("HELP").is_err());
        assert!(parse_duration("-h").is_err());
        assert!(parse_duration("--help").is_err());
    }
}
