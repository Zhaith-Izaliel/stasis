use std::time::{Duration, Instant};
use crate::{
    config::model::{IdleAction, IdleActionBlock, StasisConfig}, core::manager::{
        Manager, actions::run_action, processes::{is_process_active, is_process_running, run_command_silent}
    }, sdebug, serror, sinfo
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
        sinfo!("Stasis", "No actions defined to trigger");
        return;
    }

    sinfo!("Stasis", "Triggering all idle actions for '{}'", block_name);

    for action in actions_to_trigger {
        // Skip lockscreen if already locked
        if matches!(action.kind, IdleAction::LockScreen) && mgr.state.lock.is_locked {
            sdebug!("Stasis", "Skipping lock action: already locked");
            continue;
        }

        sinfo!("Stasis", "Triggering idle action '{}'", action.name);
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
    sinfo!("Stasis", "All idle actions triggered");
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
        sinfo!("Stasis", "Running pre-suspend command: {}", cmd);

        // Wait for it to finish (synchronous)
        match run_command_silent(cmd).await {
            Ok(_) => sinfo!("Stasis", "Pre-suspend command finished"), 
            Err(e) => serror!("Stasis", "Pre-suspend command failed: {}", e), 
        }
    }
}

pub async fn advance_past_lock(mgr: &mut Manager) {
    sdebug!("Stasis", "Advancing state past lock stage...");
    
    let now = Instant::now();
    mgr.state.lock.post_advanced = true;
    mgr.state.lock.last_advanced = Some(now);
    
    // Get debounce from config
    let debounce = if let Some(cfg) = &mgr.state.cfg {
        Duration::from_secs(cfg.debounce_seconds as u64)
    } else {
        Duration::from_secs(0)
    };
    
    mgr.state.timing.last_activity = now;
    mgr.state.debounce.main_debounce = Some(now + debounce);
    
    for actions in [
        &mut mgr.state.power.default_actions,
        &mut mgr.state.power.ac_actions,
        &mut mgr.state.power.battery_actions
    ] {
        for a in actions.iter_mut() {
            a.last_triggered = None;
        }
    }
    
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
    
    let actions = match active_block {
        "ac" => &mut mgr.state.power.ac_actions,
        "battery" => &mut mgr.state.power.battery_actions,
        _ => &mut mgr.state.power.default_actions,
    };
    
    if let Some(lock_index) = actions.iter()
        .position(|a| matches!(a.kind, crate::config::model::IdleAction::LockScreen))
    {
        let next_index = lock_index.saturating_add(1);
        mgr.state.actions.action_index = next_index;
        
        if next_index < actions.len() {
            actions[next_index].last_triggered = Some(now);
            sdebug!("Stasis", "Advanced to action index {} ({}), will fire in {}s", next_index, actions[next_index].name, actions[next_index].timeout);
        } else {
            sdebug!("Stasis", "Advanced past all actions (at end of sequence)");
        }
    } else {
        sdebug!("Stasis", "No lock action found in active block");
    }
}

pub fn has_lock_action(mgr: &mut Manager) -> bool {
    let actions = mgr.state.get_active_actions();
    actions.iter().any(|a| matches!(a.kind, IdleAction::LockScreen))
}

pub fn get_lock_index(mgr: &mut Manager) -> Option<usize> {
    let actions = mgr.state.get_active_actions();
    actions.iter().position(|a| matches!(a.kind, IdleAction::LockScreen))
}

/// List available profile names
pub fn list_profiles(mgr: &mut Manager) -> Vec<String> {
    mgr.state.profile.profile_names()
}


/// Get current profile name (None if using base)
pub fn current_profile(mgr: &mut Manager) -> Option<String> {
    mgr.state.profile.active_profile.clone()
}

pub fn profile_to_stasis_config(profile: &crate::config::model::Profile) -> StasisConfig {
    
    StasisConfig {
        actions: profile.actions.clone(),
        debounce_seconds: profile.debounce_seconds,
        inhibit_apps: profile.inhibit_apps.clone(),
        monitor_media: profile.monitor_media,
        ignore_remote_media: profile.ignore_remote_media,
        media_blacklist: profile.media_blacklist.clone(),
        pre_suspend_command: profile.pre_suspend_command.clone(),
        respect_wayland_inhibitors: profile.respect_wayland_inhibitors.clone(),
        lid_close_action: profile.lid_close_action.clone(),
        lid_open_action: profile.lid_open_action.clone(),
        notify_on_unpause: profile.notify_on_unpause,
        notify_before_action: profile.notify_before_action,
        notify_seconds_before: profile.notify_seconds_before,
        lock_detection_type: profile.lock_detection_type.clone(),
    }
}

