use std::sync::Arc;
use tokio::sync::Mutex;

use crate::core::manager::{Manager, actions::run_action, helpers::advance_past_lock, processes::run_command_detached};
use crate::{config::model::{IdleAction, LidCloseAction, LidOpenAction}};
use eventline::{event_info_scoped, event_debug_scoped, event_error_scoped};

pub enum Event {
    InputActivity,
    MediaPlaybackActive,
    MediaPlaybackEnded,
    ACConnected,
    ACDisconnected,
    LockScreenDetected,
    Suspend,
    Wake,
    Resume,
    LidClosed,
    LidOpened,
    LoginctlLock,
    LoginctlUnlock,
}

pub async fn handle_event(manager: &Arc<Mutex<Manager>>, event: Event) {
    match event {
        Event::ACConnected => {
            let mut mgr = manager.lock().await;
            mgr.state.set_on_battery(false);
            mgr.state.actions.action_index = 0;

            mgr.reset_instant_actions();
            mgr.trigger_instant_actions().await;
            mgr.state.wake_idle_tasks();
        }

        Event::ACDisconnected => {
            let mut mgr = manager.lock().await;
            mgr.state.set_on_battery(true);
            mgr.state.actions.action_index = 0;

            mgr.reset_instant_actions();
            mgr.trigger_instant_actions().await;
            mgr.state.wake_idle_tasks();
        }

        Event::InputActivity => {
            let mut mgr = manager.lock().await;
            mgr.reset().await;
            mgr.state.lock_notify.notify_waiters();
            mgr.state.wake_idle_tasks();
        }

        Event::Suspend => {
            let mut mgr = manager.lock().await;
            mgr.pause(false).await;
        }

        Event::Resume => {
            let mut mgr = manager.lock().await;
            mgr.resume(false).await;
            mgr.state.wake_idle_tasks();
        }

        Event::Wake => {
            event_info_scoped!("Stasis", "System resumed from suspend - resetting state");

            let mut mgr = manager.lock().await;
            mgr.resume(false).await;
            mgr.reset().await;
            mgr.state.wake_idle_tasks();
        }

        Event::LockScreenDetected => {
            let mut mgr = manager.lock().await;
            advance_past_lock(&mut mgr).await;
            mgr.state.wake_idle_tasks();
        }

        Event::MediaPlaybackActive => {
            let mut mgr = manager.lock().await;
            mgr.pause(false).await;
            mgr.state.wake_idle_tasks();
        }

        Event::MediaPlaybackEnded => {
            let mut mgr = manager.lock().await;
            mgr.resume(false).await;
            mgr.state.wake_idle_tasks();
        }

        Event::LidClosed => {
            let mut mgr = manager.lock().await;
            event_info_scoped!("Stasis", "Lid closed - handling event...");

            if let Some(cfg) = &mgr.state.cfg {
                let lid_close = cfg.lid_close_action.clone();
                let suspend_action_opt = cfg.actions.iter().find(|a| a.kind == IdleAction::Suspend).cloned();
                let lock_action_opt = cfg.actions.iter().find(|a| a.kind == IdleAction::LockScreen).cloned();

                match lid_close {
                    LidCloseAction::Suspend => {
                        if let Some(suspend_action) = suspend_action_opt {
                            run_action(&mut mgr, &suspend_action).await;
                        }
                    }
                    LidCloseAction::LockScreen => {
                        if let Some(lock_action) = lock_action_opt {
                            run_action(&mut mgr, &lock_action).await;
                        }
                    }
                    LidCloseAction::Custom(cmd) => {
                        let cmd_clone = cmd.clone();
                        event_info_scoped!("Stasis", "Running custom lid-close command: {}", cmd_clone);

                        match run_command_detached(&cmd).await {
                            Ok(pid) => {
                                let pid_val = pid.pid;
                                event_debug_scoped!("Stasis", "Custom lid-close command started with PID {}", pid_val);
                            }
                            Err(e) => event_error_scoped!("Stasis", "Failed to run custom lid-close command: {}", e),
                        }
                    }
                    LidCloseAction::Ignore => {
                        event_debug_scoped!("Stasis", "Lid close ignored by config");
                    }
                }
            }
        }

        Event::LidOpened => {
            let mut mgr = manager.lock().await;
            event_info_scoped!("Stasis", "Lid opened - handling event...");

            if let Some(cfg) = &mgr.state.cfg {
                match &cfg.lid_open_action {
                    LidOpenAction::Wake => {
                        let _ = cfg; // free immutable borrow
                        mgr.resume(false).await;
                        mgr.reset().await;
                        mgr.state.wake_idle_tasks();
                    }
                    LidOpenAction::Custom(cmd) => {
                        // Separate clone for macro
                        let cmd_for_macro = cmd.clone();
                        event_info_scoped!("Stasis", "Running custom lid-open command: {}", cmd_for_macro);

                        // Use original cmd for running
                        let _ = run_command_detached(cmd).await;
                    }
                    LidOpenAction::Ignore => {
                        event_debug_scoped!("Stasis", "Lid open ignored by config");
                    }
                }
            }
        }

        Event::LoginctlLock => {
            let mut mgr = manager.lock().await;
            event_debug_scoped!("Stasis", "loginctl lock-session received - handling lock...");

            if mgr.state.lock.is_locked {
                event_info_scoped!("Stasis", "Already locked, ignoring loginctl lock-session event");
                return;
            }

            let lock_cmd_opt = if let Some(cfg) = &mgr.state.cfg {
                cfg.actions.iter()
                    .find(|a| a.kind == IdleAction::LockScreen)
                    .and_then(|a| a.lock_command.clone())
            } else {
                None
            };

            mgr.state.lock.is_locked = true;
            mgr.state.lock_notify.notify_one();

            if let Some(lock_cmd) = lock_cmd_opt {
                let lock_cmd_clone = lock_cmd.clone();
                event_info_scoped!("Stasis", "Running lock-command: {}", lock_cmd_clone);

                match run_command_detached(&lock_cmd).await {
                    Ok(pid) => mgr.state.lock.process_info = Some(pid.clone()),
                    Err(e) => event_error_scoped!("Stasis", "Failed to run lock-command: {}", e),
                }
            } else {
                event_info_scoped!("Stasis", "No lock-command configured");
            }

            advance_past_lock(&mut mgr).await;
            mgr.state.wake_idle_tasks();
        }

        Event::LoginctlUnlock => {
            let mut mgr = manager.lock().await;
            event_debug_scoped!("Stasis", "loginctl unlock-session received - resetting state...");

            mgr.reset().await;
            mgr.state.lock_notify.notify_waiters();
            mgr.state.wake_idle_tasks();
        }
    }
}
