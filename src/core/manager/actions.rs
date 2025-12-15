use crate::config::model::{IdleActionBlock, IdleAction};
use crate::core::manager::{
    Manager, 
    processes::{run_command_detached, run_command_silent, is_process_running}
};
use crate::{sdebug, serror, sinfo, swarn};

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
            let probe_cmd = if let Some(ref lock_cmd) = action.lock_command {
                lock_cmd
            } else {
                &action.command
            };
            
            if is_process_running(probe_cmd).await {
                sinfo!("Stasis", "Lockscreen already running, skipping action.");
                vec![ActionRequest::Skip(probe_cmd.to_string())]
            } else {
                vec![ActionRequest::RunCommand(action.command.clone())]
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
    sdebug!("Stasis", "Action triggered: name=\"{}\" kind={:?} timeout={} command=\"{}\"", action.name, action.kind, action.timeout, action.command);

    // For lock actions using loginctl, run the command but don't manage state
    // The LoginctlLock event will handle setting up the lock state
    if matches!(action.kind, crate::config::model::IdleAction::LockScreen) {
        // Check if using logind detection
        use crate::config::model::LockDetectionType;
        let use_logind = if let Some(cfg) = &mgr.state.cfg {
            matches!(cfg.lock_detection_type, LockDetectionType::Logind)
        } else {
            false
        };

        if use_logind {
            sdebug!("Stasis", "Using logind detection for lock stase");
            
            if action.command.contains("loginctl lock-session") {
                if let Err(e) = run_command_detached(&action.command).await {
                    serror!("Stasis", "Failed to run loginctl lock-session: {}", e);
                }
                return;
            }
        } else if action.command.contains("loginctl lock-session") {
            // Legacy behavior for process detection with loginctl
            if let Err(e) = run_command_detached(&action.command).await {
                serror!("Stasis", "Failed to run loginctl lock-session: {}", e);
            }
            return;
        }
        
        if mgr.state.lock.is_locked {
            sdebug!("Stasis", "Lock screen action skipped: already locked");
            return;
        }
    }

    if matches!(action.kind, crate::config::model::IdleAction::LockScreen) {
        mgr.state.lock.is_locked = true;
        mgr.state.lock_notify.notify_one();
        sinfo!("Stasis", "Lock screen action trigerred, notifying lock watcher");
    }

    // Handle pre-suspend for Suspend actions
    if matches!(action.kind, crate::config::model::IdleAction::Suspend) {
        if let Some(cfg) = &mgr.state.cfg {
            if let Some(ref cmd) = cfg.pre_suspend_command {
                sinfo!("Stasis", "Running pre-suspend command: {}", cmd); 
                let should_wait = match run_command_detached(cmd).await {
                    Ok(pid) => {
                        sdebug!("Stasis", "Pre-suspend command started with PID {}", pid.pid);
                        true
                    }
                    Err(e) => {
                        serror!("Stasis", "Pre-suspend command failed: {}", e);
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
            sinfo!("Stasis", "Lock triggered via loginctl");

            // Fire loginctl (do not track)
            if let Err(e) = run_command_detached(&cmd).await {
                sinfo!("Stasis", "Failed to run loginctl: {}", e);
            }

            // Now run and track the real lock-command
            if let Some(ref lock_cmd) = action.lock_command {
                sinfo!("Stasis", "Running and tracking lock-command: {}", lock_cmd);

                match run_command_detached(lock_cmd).await {
                    Ok(process_info) => {
                        mgr.state.lock.process_info = Some(process_info.clone());
                        mgr.state.lock.is_locked = true;

                        sinfo!("Stasis", "Lock started: PID={}, PGID={}", process_info.pid, process_info.pgid);
                    }
                    Err(e) => serror!("Stasis", "Failed to run lock-command '{}': {}", lock_cmd, e),
                }
            } else {
                swarn!("Stasis", "loginctl used but no lock-command configured.");
                mgr.state.lock.is_locked = true;
            }

            return;
        }

        // Case 2: normal locker (anything except loginctl)
        sinfo!("Stasis", "Running lock command: {}", cmd);

        match run_command_detached(&cmd).await {
            Ok(mut process_info) => {
                // lock-command = process name override, not a command to run
                if let Some(ref lock_cmd) = action.lock_command {
                    sinfo!("Stasis", "Using lock-command as process name override: {}", lock_cmd);
                    process_info.expected_process_name = Some(lock_cmd.clone());
                }

                mgr.state.lock.process_info = Some(process_info.clone());
                mgr.state.lock.is_locked = true;

                sinfo!("Stasis", "Lock started: PID={}, PGID={}, Tracking={:?}", process_info.pid, process_info.pgid, process_info.expected_process_name);
            }

            Err(e) => serror!("Stasis", "Failed to run '{}' => {}", cmd, e), 
        }

        return;
    }

    // NON-lock Case    
    sinfo!(
        "Stasis",
        "Running {} command: {}",
        match action.kind {
            IdleAction::Suspend => "suspend",
            IdleAction::Brightness => "brightness",
            IdleAction::Dpms => "DPMS",
            _ => "action",
        },
        cmd
    );

    let spawned = tokio::spawn(async move {
        if let Err(e) = run_command_silent(&cmd).await {
            serror!("Stasis", "Failed to run command '{}': {}", cmd, e);
        }
    });
    mgr.tasks.spawned_tasks.push(spawned);
}
