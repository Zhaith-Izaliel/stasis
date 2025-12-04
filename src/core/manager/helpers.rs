use crate::log::{log_error_message, log_message};
use crate::{
    config::model::IdleActionBlock, 
    core::manager::{
        actions::{is_process_running, is_process_active, prepare_action, run_command_detached, run_command_silent, ActionRequest}, 
        brightness::capture_brightness, 
        state::ManagerState, Manager,
    }
};

pub fn wake_idle_tasks(state: &ManagerState) {
    state.notify.notify_waiters();
}

// Getters and Setters
pub fn update_lock_state(state: &mut ManagerState, locked: bool) {
    state.lock_state.is_locked = locked;
}

pub fn get_compositor_manager(state: &mut ManagerState) -> bool {
    state.compositor_managed
}

pub fn set_compositor_manager(state: &mut ManagerState, value: bool) {
    state.compositor_managed = value;
}

pub fn get_manual_inhibit(state: &mut ManagerState) -> bool {
    state.manually_paused
}

pub async fn set_manual_inhibit(mgr: &mut Manager, inhibit: bool) {
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

pub async fn run_action(mgr: &mut Manager, action: &IdleActionBlock) {
    log_message(&format!(
        "Action triggered: name=\"{}\" kind={:?} timeout={} command=\"{}\"",
        action.name, action.kind, action.timeout, action.command
    ));

    // For lock actions using loginctl, run the command but don't manage state
    // The LoginctlLock event will handle setting up the lock state
    if matches!(action.kind, crate::config::model::IdleAction::LockScreen) {
        if action.command.contains("loginctl lock-session") {
            log_message("Lock uses loginctl lock-session, triggering it (state will be managed by loginctl event)");
            // Run the loginctl command to trigger the Lock signal
            if let Err(e) = run_command_detached(&action.command).await {
                log_message(&format!("Failed to run loginctl lock-session: {}", e));
            }
            return;
        }
        
        if mgr.state.lock_state.is_locked {
            log_message("Lock screen action skipped: already locked");
            return;
        }
    }

    // Brightness capture
    if matches!(action.kind, crate::config::model::IdleAction::Brightness) && mgr.state.previous_brightness.is_none() {
        let _ = capture_brightness(&mut mgr.state).await;
    }

    if matches!(action.kind, crate::config::model::IdleAction::LockScreen) {
        mgr.state.lock_state.is_locked = true;
        mgr.state.lock_notify.notify_one();
        log_message("Lock screen action triggered, notifying lock watcher");
    }

    // Handle pre-suspend for Suspend actions
    if matches!(action.kind, crate::config::model::IdleAction::Suspend) {
        if let Some(cfg) = &mgr.state.cfg {
            if let Some(ref cmd) = cfg.pre_suspend_command {
                log_message(&format!("Running pre-suspend command: {}", cmd));
                let should_wait = match run_command_detached(cmd).await {
                    Ok(pid) => {
                        log_message(&format!("Pre-suspend command started with PID {}", pid.pid));
                        true
                    }
                    Err(e) => {
                        log_error_message(&format!("Pre-suspend command failed: {}", e));
                        true
                    }
                };
                // Wait 500ms before proceeding to suspend
                if should_wait {
                    tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;
                }
            }
        }
    }

    let requests = prepare_action(action).await;
    for req in requests {
        match req {
            ActionRequest::RunCommand(cmd) => {
                run_command_for_action(mgr, action, cmd).await;
            }
            ActionRequest::Skip(_) => {}
        }
    }
}

pub async fn run_command_for_action(
    mgr: &mut crate::core::manager::Manager, 
    action: &crate::config::model::IdleActionBlock, 
    cmd: String
) {
    use crate::config::model::IdleAction;

    let is_lock = matches!(action.kind, IdleAction::LockScreen);

    if is_lock {
        let is_loginctl = cmd.contains("loginctl lock-session");

        if is_loginctl {
            // Case 1: loginctl path
            log_message("Lock triggered via loginctl — running loginctl but not tracking it");

            // Fire loginctl (do not track)
            if let Err(e) = run_command_detached(&cmd).await {
                log_message(&format!("Failed to run loginctl: {}", e));
            }

            // Now run and track the real lock-command
            if let Some(ref lock_cmd) = action.lock_command {
                log_message(&format!("Running and tracking lock-command: {}", lock_cmd));

                match run_command_detached(lock_cmd).await {
                    Ok(process_info) => {
                        mgr.state.lock_state.process_info = Some(process_info.clone());
                        mgr.state.lock_state.is_locked = true;

                        log_message(&format!(
                            "Lock started: PID={} PGID={}",
                            process_info.pid, process_info.pgid
                        ));
                    }
                    Err(e) => log_message(&format!(
                        "Failed to run lock-command '{}': {}",
                        lock_cmd, e
                    )),
                }
            } else {
                log_message("WARNING: loginctl used but no lock-command configured.");
                mgr.state.lock_state.is_locked = true;
            }

            return;
        }

        // Case 2: normal locker (anything except loginctl)
        log_message(&format!("Running lock command: {}", cmd));

        match run_command_detached(&cmd).await {
            Ok(mut process_info) => {
                // lock-command = process name override, not a command to run
                if let Some(ref lock_cmd) = action.lock_command {
                    log_message(&format!(
                        "Using lock-command as process name override: {}",
                        lock_cmd
                    ));
                    process_info.expected_process_name = Some(lock_cmd.clone());
                }

                mgr.state.lock_state.process_info = Some(process_info.clone());
                mgr.state.lock_state.is_locked = true;

                log_message(&format!(
                    "Lock started: PID={} PGID={} tracking={:?}",
                    process_info.pid,
                    process_info.pgid,
                    process_info.expected_process_name
                ));
            }

            Err(e) => log_message(&format!("Failed to run '{}' => {}", cmd, e)),
        }

        return;
    }

    // NON-lock case
    let spawned = tokio::spawn(async move {
        if let Err(e) = run_command_silent(&cmd).await {
            log_message(&format!("Failed to run command '{}': {}", cmd, e));
        }
    });
    mgr.spawned_tasks.push(spawned);
}

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

    let block_name = if !mgr.state.ac_actions.is_empty() || !mgr.state.battery_actions.is_empty() {
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
        "ac" => mgr.state.ac_actions.clone(),
        "battery" => mgr.state.battery_actions.clone(),
        "default" => mgr.state.default_actions.clone(),
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
        "ac" => &mut mgr.state.ac_actions,
        "battery" => &mut mgr.state.battery_actions,
        "default" => &mut mgr.state.default_actions,
        _ => unreachable!(),
    };

    for a in actions_mut.iter_mut() {
        a.last_triggered = Some(now);
    }

    mgr.state.action_index = actions_mut.len().saturating_sub(1);
    log_message("All idle actions triggered manually");
}

pub async fn incr_active_inhibitor(mgr: &mut Manager) {
    let prev = mgr.state.active_inhibitor_count;
    mgr.state.active_inhibitor_count = prev.saturating_add(1);
    let now = mgr.state.active_inhibitor_count;

    if prev == 0 {
        if !mgr.state.manually_paused {
            mgr.state.paused = true;
            log_message(&format!(
                "Inhibitor registered (count: {} → {}): first inhibitor active → idle timers paused",
                prev, now
            ));
        } else {
            log_message(&format!(
                "Inhibitor registered (count: {} → {}): manual pause already active",
                prev, now
            ));
        }
    } else {
        log_message(&format!(
            "Inhibitor registered (count: {} → {})",
            prev, now
        ));
    }

    // wake idle task so it can recalc next timeout (if needed)
    mgr.state.notify.notify_one();
}

pub async fn decr_active_inhibitor(mgr: &mut Manager) {
    let prev = mgr.state.active_inhibitor_count;

    if prev == 0 {
        log_message("decr_active_inhibitor called but count already 0 (possible mismatch)");
        return;
    }

    mgr.state.active_inhibitor_count = prev.saturating_sub(1);
    let now = mgr.state.active_inhibitor_count;

    if now == 0 {
        if !mgr.state.manually_paused {
            mgr.state.paused = false;
            mgr.reset().await;

            log_message(&format!(
                "Inhibitor removed (count: {} → {}): no more inhibitors → idle timers resumed",
                prev, now
            ));

            // fire resume commands queued (if any)
            mgr.fire_resume_queue().await;
        } else {
            log_message(&format!(
                "Inhibitor removed (count: {} → {}): manual pause still active, timers remain paused",
                prev, now
            ));
        }

        // wake idle task so timeouts will be recalculated right away
        mgr.state.notify.notify_one();
    } else {
        log_message(&format!(
            "Inhibitor removed (count: {} → {})",
            prev, now
        ));
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
