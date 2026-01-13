use std::time::{Duration, Instant};
use crate::{
    config::model::{IdleAction, IdleActionBlock, StasisConfig}, 
    core::manager::{
        Manager, actions::run_action, processes::{is_process_active, is_process_running, run_command_silent}
    }
};
use eventline::{event_info_scoped, event_debug_scoped, event_error_scoped};

pub async fn lock_still_active(state: &crate::core::manager::state::ManagerState) -> bool {
    if let Some(ref info) = state.lock.process_info {
        is_process_active(info).await
    } else if let Some(cmd) = &state.lock.command {
        is_process_running(cmd).await
    } else {
        false
    }
}

pub async fn trigger_all_idle_actions(mgr: &mut Manager) {
    let block_name = if !mgr.state.power.ac_actions.is_empty() || !mgr.state.power.battery_actions.is_empty() {
        match mgr.state.on_battery() {
            Some(true) => "battery",
            Some(false) => "ac",
            None => "default",
        }
    } else {
        "default"
    };

    // Clone the actions to avoid borrowing issues
    let actions_to_trigger: Vec<IdleActionBlock> = match block_name {
        "ac" => mgr.state.power.ac_actions.clone(),
        "battery" => mgr.state.power.battery_actions.clone(),
        "default" => mgr.state.power.default_actions.clone(),
        _ => unreachable!(),
    };

    if actions_to_trigger.is_empty() {
        event_info_scoped!("Stasis", "No actions defined to trigger").await;
        return;
    }

    event_info_scoped!("Stasis", "Triggering all idle actions for '{}'", block_name).await;

    for action in actions_to_trigger {
        // Skip lockscreen if already locked
        if matches!(action.kind, IdleAction::LockScreen) && mgr.state.lock.is_locked {
            event_debug_scoped!("Stasis", "Skipping lock action: already locked").await;
            continue;
        }

        // Clone name for logging
        let action_name_for_log = action.name.clone();
        event_info_scoped!("Stasis", "Triggering idle action '{}'", action_name_for_log).await;

        run_action(mgr, &action).await;
    }

    // Update last_triggered timestamps
    let now = Instant::now();
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
    event_info_scoped!("Stasis", "All idle actions triggered").await;
}

pub async fn set_manually_paused(mgr: &mut Manager, inhibit: bool) {
    if inhibit {
        mgr.pause(true).await;
        mgr.state.inhibitors.manually_paused = true;
    } else {
        mgr.resume(true).await;
        mgr.state.inhibitors.manually_paused = false;
    }
}

pub async fn trigger_pre_suspend(mgr: &mut Manager) {
    if let Some(cmd) = &mgr.state.pre_suspend_command {
        let cmd_owned = cmd.clone();
        event_info_scoped!("Stasis", "Running pre-suspend command: {}", cmd_owned).await;

        match run_command_silent(cmd).await {
            Ok(_) => event_info_scoped!("Stasis", "Pre-suspend command finished").await,
            Err(e) => event_error_scoped!("Stasis", "Pre-suspend command failed: {}", e).await,
        }
    }
}

pub async fn advance_past_lock(mgr: &mut Manager) {
    event_debug_scoped!("Stasis", "Advancing state past lock stage...").await;

    let now = Instant::now();
    mgr.state.lock.post_advanced = true;
    mgr.state.lock.last_advanced = Some(now);

    let debounce = mgr.state.cfg
        .as_ref()
        .map(|cfg| Duration::from_secs(cfg.debounce_seconds as u64))
        .unwrap_or(Duration::from_secs(0));

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

    let active_block = if !mgr.state.power.ac_actions.is_empty() || !mgr.state.power.battery_actions.is_empty() {
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

    if let Some(lock_index) = actions.iter().position(|a| matches!(a.kind, IdleAction::LockScreen)) {
        let next_index = lock_index.saturating_add(1);
        mgr.state.actions.action_index = next_index;

        if next_index < actions.len() {
            actions[next_index].last_triggered = Some(now);
            // Clone name for macro
            let action_name_for_log = actions[next_index].name.clone();
            let timeout = actions[next_index].timeout;
            event_debug_scoped!(
                "Stasis",
                "Advanced to action index {} ({}), will fire in {}s",
                next_index,
                action_name_for_log,
                timeout
            ).await;
        } else {
            event_debug_scoped!("Stasis", "Advanced past all actions (at end of sequence)").await;
        }
    } else {
        event_debug_scoped!("Stasis", "No lock action found in active block").await;
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

pub fn list_profiles(mgr: &mut Manager) -> Vec<String> {
    mgr.state.profile.profile_names()
}

pub fn current_profile(mgr: &mut Manager) -> Option<String> {
    mgr.state.profile.active_profile.clone()
}

pub fn profile_to_stasis_config(profile: &crate::config::model::Profile) -> StasisConfig {
    let mut cfg = StasisConfig::default();

    if !profile.actions.is_empty() {
        cfg.actions = profile.actions.clone();
    }
    if profile.debounce_seconds != 0 {
        cfg.debounce_seconds = profile.debounce_seconds;
    }
    if !profile.inhibit_apps.is_empty() {
        cfg.inhibit_apps = profile.inhibit_apps.clone();
    }
    cfg.monitor_media = profile.monitor_media;
    cfg.ignore_remote_media = profile.ignore_remote_media;
    if !profile.media_blacklist.is_empty() {
        cfg.media_blacklist = profile.media_blacklist.clone();
    }
    cfg.pre_suspend_command = profile.pre_suspend_command.clone();
    cfg.respect_wayland_inhibitors = profile.respect_wayland_inhibitors;
    cfg.lid_close_action = profile.lid_close_action.clone();
    cfg.lid_open_action = profile.lid_open_action.clone();
    cfg.notify_on_unpause = profile.notify_on_unpause;
    cfg.notify_before_action = profile.notify_before_action;
    cfg.notify_seconds_before = profile.notify_seconds_before;
    cfg.lock_detection_type = profile.lock_detection_type.clone();

    cfg
}
