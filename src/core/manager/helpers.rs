use std::time::{Duration, Instant};
use crate::{
    config::model::IdleActionBlock, 
    core::manager::{
        actions::run_action,
        processes::{is_process_active, is_process_running, run_command_silent},
        Manager,
    },
    log::{log_debug_message, log_message},
};

pub async fn lock_still_active(state: &crate::core::manager::state::ManagerState) -> bool {
    if let Some(ref info) = state.lock.process_info {
        is_process_active(info).await
    } else if let Some(cmd) = &state.lock.command {
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
        if matches!(action.kind, IdleAction::LockScreen) && mgr.state.lock.is_locked {
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

    mgr.state.actions.action_index = actions_mut.len().saturating_sub(1);
    log_message("All idle actions triggered manually");
}

pub async fn set_manually_paused(mgr: &mut Manager, inhibit: bool) {
    if inhibit {
        // Enable manual pause
        mgr.pause(true).await;
        mgr.state.inhibitors.manually_paused = true;
    } else {
        // Disable manual pause
        mgr.resume(true).await;
        mgr.state.inhibitors.manually_paused = false;
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

pub async fn advance_past_lock(mgr: &mut Manager) {
    log_debug_message("Advancing state past lock stage...");
    
    let now = Instant::now();
    mgr.state.lock.post_advanced = true;
    mgr.state.lock.last_advanced = Some(now);
    
    // Get debounce from config
    let debounce = if let Some(cfg) = &mgr.state.cfg {
        Duration::from_secs(cfg.debounce_seconds as u64)
    } else {
        Duration::from_secs(5) // fallback
    };
    
    // Reset timing state
    mgr.state.timing.last_activity = now;
    mgr.state.debounce.main_debounce = Some(now + debounce);
    
    // Clear last_triggered for all actions
    for actions in [
        &mut mgr.state.power.default_actions,
        &mut mgr.state.power.ac_actions,
        &mut mgr.state.power.battery_actions
    ] {
        for a in actions.iter_mut() {
            a.last_triggered = None;
        }
    }
    
    // Determine active block
    let active_block = if !mgr.state.power.ac_actions.is_empty() 
        || !mgr.state.power.battery_actions.is_empty() 
    {
        match mgr.state.on_battery() {
            Some(true) => "battery",
            Some(false) => "ac",
            None => "default",
        }
    } else {
        "default"
    };
    
    // Get mutable reference to active actions
    let actions = match active_block {
        "ac" => &mut mgr.state.power.ac_actions,
        "battery" => &mut mgr.state.power.battery_actions,
        _ => &mut mgr.state.power.default_actions,
    };
    
    // Find lock index and advance past it
    if let Some(lock_index) = actions.iter()
        .position(|a| matches!(a.kind, crate::config::model::IdleAction::LockScreen))
    {
        let next_index = lock_index.saturating_add(1);
        mgr.state.actions.action_index = next_index;
        
        // CRITICAL: Set the next action's last_triggered so timeout calculation works
        if next_index < actions.len() {
            actions[next_index].last_triggered = Some(now);
            log_debug_message(&format!(
                "Advanced to action index {} ({}), will fire in {}s",
                next_index,
                actions[next_index].name,
                actions[next_index].timeout
            ));
        } else {
            log_debug_message("Advanced past all actions (at end of chain)");
        }
    } else {
        log_debug_message("No lock action found in active block");
    }
}
