use std::{
    sync::Arc,
    time::{Duration, Instant},
};
use tokio::sync::Mutex;

use crate::{
    core::manager::{
        actions::run_action, 
        helpers::trigger_pre_suspend, 
        Manager, 
        processes::{run_command_detached}},
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
            "ac" => &mgr.state.power.ac_actions,
            "battery" => &mgr.state.power.battery_actions,
            _ => &mgr.state.power.default_actions,
        }
    } else if !mgr.state.power.ac_actions.is_empty() || !mgr.state.power.battery_actions.is_empty() {
        // Auto-detect based on current power state
        match mgr.state.on_battery() {
            Some(true) => &mgr.state.power.battery_actions,
            Some(false) => &mgr.state.power.ac_actions,
            None => &mgr.state.power.default_actions,
        }
    } else {
        &mgr.state.power.default_actions
    };

    let action_opt = block.iter().find(|a| {
        let kind_name = format!("{:?}", a.kind).to_lowercase().replace('_', "-");
        let kind_name_no_hyphen = kind_name.replace('-', "");
        let search_name_no_hyphen = search_name.replace('-', "");
        let stripped_name = strip_action_prefix(&a.name).to_lowercase();
        let stripped_name_no_hyphen = stripped_name.replace('-', "");
        
        kind_name == search_name 
            || kind_name_no_hyphen == search_name_no_hyphen
            || stripped_name == search_name 
            || stripped_name_no_hyphen == search_name_no_hyphen
            || a.name.to_lowercase() == search_name
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

    log_message(&format!("Action triggered via IPC: '{}'", strip_action_prefix(&action.name)));
    let is_lock = matches!(action.kind, crate::config::model::IdleAction::LockScreen);

    if is_lock {
        // Check if this uses loginctl lock-session
        let uses_loginctl = action.command.contains("loginctl lock-session");
        
        if uses_loginctl {
            // For loginctl-based locks, just trigger the command
            // The LoginctlLock event will handle the rest
            log_message("Lock uses loginctl lock-session, triggering it via IPC");
            if let Err(e) = run_command_detached(&action.command).await {
                return Err(format!("Failed to trigger lock: {}", e));
            }
        } else {
            // For non-loginctl locks, do the full lock setup
            mgr.state.lock_state.is_locked = true;
            mgr.state.lock_state.post_advanced = false;
            mgr.state.lock_state.command = Some(action.command.clone());
            mgr.state.lock_notify.notify_one();

            // Run the lock command
            run_action(&mut mgr, &action).await;

            // Mark as advanced past lock
            mgr.advance_past_lock().await;

            // Reset timers
            let now = Instant::now();
            if let Some(cfg) = &mgr.state.cfg {
                let debounce = Duration::from_secs(cfg.debounce_seconds as u64);
                mgr.state.last_activity = now;
                mgr.state.debounce = Some(now + debounce);

                // Clear last_triggered for all actions
                {
                    let actions = &mut mgr.state.power.default_actions;
                    for a in actions.iter_mut() {
                        a.last_triggered = None;
                    }
                }
                {
                    let actions = &mut mgr.state.power.ac_actions;
                    for a in actions.iter_mut() {
                        a.last_triggered = None;
                    }
                }
                {
                    let actions = &mut mgr.state.power.battery_actions;
                    for a in actions.iter_mut() {
                        a.last_triggered = None;
                    }
                }

                // Determine active block name first
                let active_block = if !mgr.state.power.ac_actions.is_empty() || !mgr.state.power.battery_actions.is_empty() {
                    match mgr.state.on_battery() {
                        Some(true) => "battery",
                        Some(false) => "ac",
                        None => "default",
                    }
                } else {
                    "default"
                };

                // Now isolate block mutation
                let actions = match active_block {
                    "ac" => &mut mgr.state.power.ac_actions,
                    "battery" => &mut mgr.state.power.battery_actions,
                    _ => &mut mgr.state.power.default_actions,
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

            // Wake idle loop to recalculate timers
            mgr.state.notify.notify_one();
        }
    } else {
        run_action(&mut mgr, &action).await;
    }

    Ok(strip_action_prefix(&action.name).to_string())
}

pub async fn list_available_actions(manager: Arc<Mutex<Manager>>) -> Vec<String> {
    let mgr = manager.lock().await;
    let mut actions = mgr
        .state
        .power
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
