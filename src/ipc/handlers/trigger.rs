use std::{sync::Arc, time::{Duration, Instant}};
use tokio::sync::Mutex;

use crate::core::manager::{
    Manager,
    actions::run_action,
    helpers::{advance_past_lock, trigger_all_idle_actions, trigger_pre_suspend},
    processes::run_command_detached,
};
use eventline::{event_info_scoped, event_debug_scoped, event_error_scoped, runtime};

/// Handles the "trigger" command - triggers actions by name
pub async fn handle_trigger(
    manager: Arc<Mutex<Manager>>,
    action: &str,
) -> String {
    let action_owned = action.to_owned();

    runtime::scoped_async(Some("TriggerCommand"), || async move {
        if action_owned.is_empty() {
            event_error_scoped!("TriggerCommand", "Trigger command missing action name");
            return "ERROR: No action name provided".to_string();
        }

        if action_owned.eq_ignore_ascii_case("all") {
            return trigger_all(manager).await;
        }

        match trigger_action_by_name(manager, &action_owned).await {
            Ok(name) => format!("Action '{}' triggered successfully", name),
            Err(e) => format!("ERROR: {}", e),
        }
    })
    .await
}

async fn trigger_all(manager: Arc<Mutex<Manager>>) -> String {
    runtime::scoped_async(Some("TriggerAll"), || async move {
        let mut mgr = manager.lock().await;
        trigger_all_idle_actions(&mut mgr).await;
        event_debug_scoped!("TriggerAll", "Triggered all idle actions");
        "All idle actions triggered".to_string()
    })
    .await
}

/// Helper to remove `ac.` or `battery.` prefixes
fn strip_action_prefix(name: &str) -> &str {
    name.strip_prefix("ac.")
        .or_else(|| name.strip_prefix("battery."))
        .unwrap_or(name)
}

/// Triggers a specific action by name
pub async fn trigger_action_by_name(
    manager: Arc<Mutex<Manager>>,
    name: &str,
) -> Result<String, String> {
    let name_owned = name.to_owned();

    runtime::scoped_async(Some("TriggerAction"), || async move {
        let normalized = name_owned.replace('_', "-").to_lowercase();
        let mut mgr = manager.lock().await;

        if normalized == "pre-suspend" || normalized == "presuspend" {
            trigger_pre_suspend(&mut mgr).await;
            return Ok("pre_suspend".to_string());
        }

        // Determine block and search name
        let (target_block, search_name) = if normalized.starts_with("ac.") {
            (Some("ac"), normalized.strip_prefix("ac.").unwrap())
        } else if normalized.starts_with("battery.") {
            (Some("battery"), normalized.strip_prefix("battery.").unwrap())
        } else {
            (None, normalized.as_str())
        };

        // Select the appropriate block for searching (immutable borrow)
        let block = if let Some(explicit_block) = target_block {
            match explicit_block {
                "ac" => &mgr.state.power.ac_actions,
                "battery" => &mgr.state.power.battery_actions,
                _ => &mgr.state.power.default_actions,
            }
        } else if !mgr.state.power.ac_actions.is_empty() || !mgr.state.power.battery_actions.is_empty() {
            match mgr.state.on_battery() {
                Some(true) => &mgr.state.power.battery_actions,
                Some(false) => &mgr.state.power.ac_actions,
                None => &mgr.state.power.default_actions,
            }
        } else {
            &mgr.state.power.default_actions
        };

        // Find the action
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
                    name_owned,
                    available.join(", ")
                ));
            }
        };

        // Clone name for logging separately to avoid moving original
        let action_name_for_log = action.name.clone();
        event_info_scoped!(
            "TriggerAction",
            "Action triggered via IPC '{}'",
            strip_action_prefix(&action_name_for_log)
        );

        let is_lock = matches!(action.kind, crate::config::model::IdleAction::LockScreen);

        if is_lock {
            let uses_loginctl = action.command.contains("loginctl lock-session");

            if uses_loginctl {
                event_info_scoped!("TriggerAction", "Lock uses loginctl lock-session, triggering via IPC");
                if let Err(e) = run_command_detached(&action.command).await {
                    return Err(format!("Failed to trigger lock: {}", e));
                }
            } else {
                mgr.state.lock.is_locked = true;
                mgr.state.lock.post_advanced = false;
                mgr.state.lock.command = Some(action.command.clone());
                mgr.state.lock_notify.notify_one();

                run_action(&mut mgr, &action).await;
                advance_past_lock(&mut mgr).await;

                let now = Instant::now();
                if let Some(cfg) = &mgr.state.cfg {
                    let debounce = Duration::from_secs(cfg.debounce_seconds as u64);
                    mgr.state.timing.last_activity = now;
                    mgr.state.debounce.main_debounce = Some(now + debounce);

                    // Reset last_triggered safely, one mutable borrow at a time
                    {
                        let power = &mut mgr.state.power;
                        for a in &mut power.default_actions {
                            a.last_triggered = None;
                        }
                        for a in &mut power.ac_actions {
                            a.last_triggered = None;
                        }
                        for a in &mut power.battery_actions {
                            a.last_triggered = None;
                        }
                    }

                    // Determine active block and borrow only once
                    let active_block = if !mgr.state.power.ac_actions.is_empty() || !mgr.state.power.battery_actions.is_empty() {
                        match mgr.state.on_battery() {
                            Some(true) => "battery",
                            Some(false) => "ac",
                            None => "default",
                        }
                    } else {
                        "default"
                    };

                    let actions: &mut Vec<_> = match active_block {
                        "ac" => &mut mgr.state.power.ac_actions,
                        "battery" => &mut mgr.state.power.battery_actions,
                        _ => &mut mgr.state.power.default_actions,
                    };

                    let mut next_index = actions.iter()
                        .position(|a| a.last_triggered.is_none())
                        .unwrap_or_else(|| actions.len().saturating_sub(1));

                    if let Some(lock_index) = actions.iter()
                        .position(|a| matches!(a.kind, crate::config::model::IdleAction::LockScreen)) {
                        if next_index <= lock_index {
                            next_index = lock_index.saturating_add(1);
                            let debounce_end = now + debounce;
                            if next_index < actions.len() {
                                actions[next_index].last_triggered = Some(debounce_end);
                            }
                            mgr.state.lock.post_advanced = true;
                        }
                    }

                    mgr.state.actions.action_index = next_index;
                }

                mgr.state.notify.notify_one();
            }
        } else {
            run_action(&mut mgr, &action).await;
        }

        // Return value uses original `action.name`, safe to borrow now
        Ok(strip_action_prefix(&action.name).to_string())
    })
    .await
}
