use std::{sync::Arc, future::Future, time::{Duration, Instant}};
use tokio::{
    sync::Mutex, 
    time::{Instant as TokioInstant, sleep_until}
};

use crate::{
    core::{
        manager::{
            Manager, 
            processes::{is_process_running, is_process_active, run_command_detached}
        }
    }, 
    log::{log_debug_message, log_message},
};

pub fn spawn_idle_task(manager: Arc<Mutex<Manager>>) -> impl Future<Output = ()> + Send {
    async move {
        loop {
            // Grab both the next timeout and the notify handles
            let (next_instant, notify, shutdown) = {
                let mgr = manager.lock().await;
                (
                    mgr.next_action_instant(),
                    mgr.state.notify.clone(),
                    mgr.state.shutdown_flag.clone(),
                )
            };

            // Compute how long we should sleep using tokio Instant (no drift)
            let now = TokioInstant::now();
            let sleep_deadline = match next_instant {
                Some(next_std) => {
                    let delta = if next_std > Instant::now() {
                        next_std - Instant::now()
                    } else {
                        Duration::from_millis(50)
                    };
                    let max_sleep = Duration::from_secs(60);
                    now + delta.min(max_sleep)
                }
                None => now + Duration::from_secs(60),
            };

            tokio::select! {
                _ = sleep_until(sleep_deadline) => {},
                _ = notify.notified() => {
                    // Woken up by external event (reset, AC change, playback)
                    continue; // recalc immediately
                }
                _ = shutdown.notified() => {
                    break; // exit loop cleanly
                }
            }

            // Now check timeouts only once after wake
            let mut mgr = manager.lock().await;
            if !mgr.state.inhibitors.paused && !mgr.state.inhibitors.manually_paused {
                mgr.check_timeouts().await;
            }
        }

        log_message("Idle loop shutting down...");
    }
}

pub fn spawn_lock_watcher(
    manager: std::sync::Arc<tokio::sync::Mutex<crate::core::manager::Manager>>
) -> impl Future<Output = ()> + Send {
    use std::time::Duration;
    use tokio::time::sleep;
    
    async move {
        loop {
            let shutdown = {
                let mgr = manager.lock().await;
                mgr.state.shutdown_flag.clone()
            };

            // Wait until lock becomes active
            {
                let mut mgr = manager.lock().await;
                while !mgr.state.lock.is_locked {
                    let lock_notify = mgr.state.lock_notify.clone();
                    drop(mgr);
                    tokio::select! {
                        _ = lock_notify.notified() => {},
                        _ = shutdown.notified() => {
                            log_message("Lock watcher shutting down...");
                            return;
                        }
                    }
                    mgr = manager.lock().await;
                }
            }

            log_debug_message("Lock detected — entering lock watcher");

            // Monitor lock until it ends
            loop {
                let (process_info, maybe_cmd, was_locked, shutdown, lock_notify, detection_type) = {
                    let mgr = manager.lock().await;
    
                    
                     let detection_type = mgr.state.cfg.as_ref().map(|cfg| cfg.lock_detection_type.clone());

                    (
                        mgr.state.lock.process_info.clone(),
                        mgr.state.lock.command.clone(),
                        mgr.state.lock.is_locked,
                        mgr.state.shutdown_flag.clone(),
                        mgr.state.lock_notify.clone(),
                        detection_type,
                    )
                };

                if !was_locked {
                    break;
                }

                // Check if lock is still active based on detection type
                use crate::config::model::LockDetectionType;
                let still_active = match detection_type {
                    Some(LockDetectionType::Logind) => {
                        // Use logind's LockedHint property
                        use crate::core::manager::processes::is_session_locked_logind;
                        is_session_locked_logind().await
                    }
                    Some(LockDetectionType::Process) | None => {
                        // Use process detection (default)
                        if let Some(ref info) = process_info {
                            is_process_active(info).await
                        } else if let Some(cmd) = maybe_cmd {
                            is_process_running(&cmd).await
                        } else {
                            sleep(Duration::from_millis(500)).await;
                            true
                        }
                    }
                };

                if !still_active {
                    let mut mgr = manager.lock().await;

                    if !mgr.state.lock.is_locked {
                        break;
                    }

                    // Fire resume command if configured
                    use crate::config::model::IdleAction;
                    if let Some(lock_action) = mgr.state.power.default_actions.iter()
                        .chain(mgr.state.power.ac_actions.iter())
                        .chain(mgr.state.power.battery_actions.iter())
                        .find(|a| matches!(a.kind, IdleAction::LockScreen))
                    {
                        if let Some(resume_cmd) = &lock_action.resume_command {
                            log_message("Firing lockscreen resume command");
                            if let Err(e) = run_command_detached(resume_cmd).await {
                                log_message(&format!("Failed to run lock resume command: {}", e));
                            }
                        }
                    }

                    mgr.state.lock.process_info = None;
                    mgr.state.lock.post_advanced = false;
                    mgr.state.actions.action_index = 0;
                    mgr.state.lock.is_locked = false;

                    // NEW: Fire all pre-lock resume commands on unlock
                    mgr.fire_pre_lock_resume_queue().await;

                    mgr.reset().await;

                    log_message("Lockscreen ended — exiting lock watcher");
                    break;
                }

                tokio::select! {
                    _ = lock_notify.notified() => {},
                    _ = sleep(Duration::from_millis(500)) => {},
                    _ = shutdown.notified() => {
                        log_message("Lock watcher shutting down during active lock...");
                        return;
                    }
                }
            }
        }
    }
}

