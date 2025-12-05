use crate::{
    config::model::IdleActionBlock, 
    core::manager::{
        actions::run_action,
        processes::{is_process_active, is_process_running, run_command_silent},
        Manager,
    },
    log::log_message,
};

pub async fn lock_still_active(state: &crate::core::manager::state::ManagerState) -> bool {
    if let Some(ref info) = state.lock_state.process_info {
        is_process_active(info).await
    } else if let Some(cmd) = &state.lock_state.command {
        // Fallback to old method if no ProcessInfo
        is_process_running(cmd).await
    } else {
        false
    }
}

pub async fn trigger_all_idle_actions(mgr: &mut Manager) {
    use crate::config::model::IdleAction;

    let block_name = if !mgr.state.power.ac_actions.is_empty() || !mgr.state.power.battery_actions.is_empty() {
        match mgr.state.on_battery() {
            Some(true) => "battery",
            Some(false) => "ac",
            None => "default",
        }
    } else {
        "default"
    };

    // Clone the actions so we don't borrow mgr mutably while iterating
    let actions_to_trigger: Vec<IdleActionBlock> = match block_name {
        "ac" => mgr.state.power.ac_actions.clone(),
        "battery" => mgr.state.power.battery_actions.clone(),
        "default" => mgr.state.power.default_actions.clone(),
        _ => unreachable!(),
    };

    if actions_to_trigger.is_empty() {
        log_message("No actions defined to trigger");
        return;
    }

    log_message(&format!("Triggering all idle actions for '{}'", block_name));

    for action in actions_to_trigger {
        // Skip lockscreen if already locked
        if matches!(action.kind, IdleAction::LockScreen) && mgr.state.lock_state.is_locked {
            log_message("Skipping lock action: already locked");
            continue;
        }

        log_message(&format!("Triggering idle action '{}'", action.name));
        run_action(mgr, &action).await;
    }

    // Now update `last_triggered` after all actions are done
    let now = std::time::Instant::now();
    let actions_mut: &mut Vec<IdleActionBlock> = match block_name {
        "ac" => &mut mgr.state.power.ac_actions,
        "battery" => &mut mgr.state.power.battery_actions,
        "default" => &mut mgr.state.power.default_actions,
        _ => unreachable!(),
    };

    for a in actions_mut.iter_mut() {
        a.last_triggered = Some(now);
    }

    mgr.state.action_index = actions_mut.len().saturating_sub(1);
    log_message("All idle actions triggered manually");
}

pub async fn set_manually_paused(mgr: &mut Manager, inhibit: bool) {
    if inhibit {
        // Enable manual pause
        mgr.pause(true).await;
        mgr.state.manually_paused = true;
    } else {
        // Disable manual pause
        mgr.resume(true).await;
        mgr.state.manually_paused = false;
    }
}

pub async fn trigger_pre_suspend(mgr: &mut Manager) {
    if let Some(cmd) = &mgr.state.pre_suspend_command {
        log_message(&format!("Running pre-suspend command: {}", cmd));

        // Wait for it to finish (synchronous)
        match run_command_silent(cmd).await {
            Ok(_) => log_message("Pre-suspend command finished"),
            Err(e) => log_message(&format!("Pre-suspend command failed: {}", e)),
        }
    }
}
