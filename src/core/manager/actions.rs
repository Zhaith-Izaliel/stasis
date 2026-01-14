use crate::config::model::{IdleActionBlock, IdleAction};
use crate::core::manager::{
    Manager, 
    processes::{run_command_detached, run_command_silent, is_process_running}
};
use eventline::{event_info_scoped, event_debug_scoped, event_warn_scoped, event_error_scoped};

#[derive(Debug, Clone)]
pub enum ActionRequest {
    RunCommand(String),
    Skip(String),
}

/// Prepare action for execution
pub async fn prepare_action(action: &IdleActionBlock) -> Vec<ActionRequest> {
    let cmd = action.command.clone();
    match action.kind {
        IdleAction::Suspend => {
            if !cmd.trim().is_empty() {
                vec![ActionRequest::RunCommand(cmd)]
            } else {
                vec![]
            }
        }
        IdleAction::LockScreen => {
            let probe_cmd = action.lock_command.clone().unwrap_or(action.command.clone());
            
            if is_process_running(&probe_cmd).await {
                event_info_scoped!("Stasis", "Lockscreen already running, skipping action.");
                vec![ActionRequest::Skip(probe_cmd)]
            } else {
                vec![ActionRequest::RunCommand(cmd)]
            }
        }
        _ => {
            if cmd.trim().is_empty() {
                vec![]
            } else {
                vec![ActionRequest::RunCommand(cmd)]
            }
        }
    }
}

pub async fn run_action(mgr: &mut Manager, action: &IdleActionBlock) {
    let name = action.name.clone();
    let command = action.command.clone();
    let kind = action.kind.clone(); 
    let timeout = action.timeout;

    // --- FIX FOR ERRORS 1 & 2 (kind and command) ---
    // We create clones specifically for the macro to "consume"
    let kind_for_log = kind.clone();
    let cmd_for_log = command.clone();

    event_debug_scoped!(
        "Stasis",
        "Action triggered: name=\"{}\" kind={:?} timeout={} command=\"{}\"",
        name,
        kind_for_log, // Macro moves 'kind_for_log'
        timeout,
        cmd_for_log   // Macro moves 'cmd_for_log'
    ); 
    // 'kind' and 'command' are still safe to use below!

    // Lock screen handling
    if matches!(kind, IdleAction::LockScreen) {
        use crate::config::model::LockDetectionType;
        let use_logind = mgr.state.cfg
            .as_ref()
            .map(|cfg| matches!(cfg.lock_detection_type, LockDetectionType::Logind))
            .unwrap_or(false);

        if use_logind && command.contains("loginctl lock-session") {
            if let Err(e) = run_command_detached(&command).await {
                event_error_scoped!("Stasis", "Failed loginctl lock-session '{}': {}", command, e);
            }
            return;
        }

        if mgr.state.lock.is_locked {
            event_debug_scoped!("Stasis", "Lock screen action skipped: already locked");
            return;
        }

        mgr.state.lock.is_locked = true;
        mgr.state.lock_notify.notify_one();
        event_info_scoped!("Stasis", "Lock screen action triggered");
    }

    // Pre-suspend
    if matches!(kind, IdleAction::Suspend) {
        if let Some(cfg) = &mgr.state.cfg {
            if let Some(pre_suspend) = &cfg.pre_suspend_command {
                let pre_suspend_cmd = pre_suspend.clone();
                
                // --- FIX FOR ERROR 3 (pre_suspend_cmd) ---
                let pre_suspend_for_log = pre_suspend_cmd.clone();
                event_info_scoped!("Stasis", "Running pre-suspend command: {}", pre_suspend_for_log);
                
                // Now we can safely borrow the original
                if let Err(e) = run_command_detached(&pre_suspend_cmd).await {
                    event_error_scoped!("Stasis", "Pre-suspend command failed: {}", e);
                } else {
                    tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;
                }
            }
        }
    }

    // Prepare and run actual action
    let requests = prepare_action(action).await;
    for req in requests {
        match req {
            ActionRequest::RunCommand(cmd) => run_command_for_action(mgr, action, cmd).await,
            ActionRequest::Skip(_) => {}
        }
    }
}

pub async fn run_command_for_action(
    mgr: &mut Manager, 
    action: &IdleActionBlock, 
    cmd: String
) {
    use crate::config::model::IdleAction;

    let is_lock = matches!(action.kind, IdleAction::LockScreen);

    if is_lock {
        let is_loginctl = cmd.contains("loginctl lock-session");

        if is_loginctl {
            event_info_scoped!("Stasis", "Lock triggered via loginctl");

            if let Err(e) = run_command_detached(&cmd).await {
                let cmd_owned = cmd.clone();
                event_error_scoped!("Stasis", "Failed to run loginctl '{}': {}", cmd_owned, e);
            }

            if let Some(lock_cmd) = &action.lock_command {
                let lock_cmd_owned = lock_cmd.clone();
                event_info_scoped!("Stasis", "Running and tracking lock-command: {}", lock_cmd_owned);

                match run_command_detached(lock_cmd).await {
                    Ok(process_info) => {
                        mgr.state.lock.process_info = Some(process_info.clone());
                        mgr.state.lock.is_locked = true;
                        event_info_scoped!(
                            "Stasis",
                            "Lock started: PID={}, PGID={}",
                            process_info.pid,
                            process_info.pgid
                        );
                    }
                    Err(e) => {
                        let lock_cmd_owned = lock_cmd.clone();
                        event_error_scoped!(
                            "Stasis",
                            "Failed to run lock-command '{}': {}",
                            lock_cmd_owned,
                            e
                        );
                    }
                }
            } else {
                event_warn_scoped!("Stasis", "loginctl used but no lock-command configured.");
                mgr.state.lock.is_locked = true;
            }

            return;
        }

        // Normal locker
        let cmd_owned = cmd.clone();
        event_info_scoped!("Stasis", "Running lock command: {}", cmd_owned);

        match run_command_detached(&cmd).await {
            Ok(mut process_info) => {
                if let Some(lock_cmd) = &action.lock_command {
                    let lock_cmd_owned = lock_cmd.clone();
                    process_info.expected_process_name = Some(lock_cmd_owned.clone());
                    event_info_scoped!("Stasis", "Using lock-command as process name override: {}", lock_cmd_owned);
                }

                mgr.state.lock.process_info = Some(process_info.clone());
                mgr.state.lock.is_locked = true;

                event_info_scoped!(
                    "Stasis",
                    "Lock started: PID={}, PGID={}, Tracking={:?}",
                    process_info.pid,
                    process_info.pgid,
                    process_info.expected_process_name
                );
            }
            Err(e) => {
                let cmd_owned = cmd.clone();
                event_error_scoped!("Stasis", "Failed to run '{}': {}", cmd_owned, e);
            }
        }

        return;
    }

    // NON-lock actions
    let cmd_owned = cmd.clone();
    let action_type = match action.kind {
        IdleAction::Suspend => "suspend",
        IdleAction::Brightness => "brightness",
        IdleAction::Dpms => "DPMS",
        _ => "action",
    };

    event_info_scoped!(
        "Stasis",
        "Running {} command: {}",
        action_type,
        cmd_owned
    );

    let cmd_owned_for_spawn = cmd.clone();
    let spawned = tokio::spawn(async move {
        if let Err(e) = run_command_silent(&cmd_owned_for_spawn).await {
            let cmd_for_err = cmd_owned_for_spawn.clone();
            event_error_scoped!("Stasis", "Failed to run command '{}': {}", cmd_for_err, e);
        }
    });

    mgr.tasks.spawned_tasks.push(spawned);
}
