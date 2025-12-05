use std::sync::Arc;
use tokio::sync::Mutex;

use crate::core::{
    manager::{actions::run_action, Manager, processes::{run_command_detached}}
};
use crate::{config::model::{IdleAction, LidCloseAction, LidOpenAction}};
use crate::log::{log_debug_message, log_error_message, log_message};

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
            log_message("System resumed from suspend - resetting state");
            
            let mut mgr = manager.lock().await;
            mgr.resume(false).await;
            mgr.reset().await;
            mgr.state.wake_idle_tasks();
        }
                
        Event::LockScreenDetected => {
            let mut mgr = manager.lock().await;
            mgr.advance_past_lock().await;
            mgr.state.wake_idle_tasks();
        }

        Event::MediaPlaybackActive => {
            let mut mgr = manager.lock().await;
            mgr.pause(false).await;
            mgr.state.wake_idle_tasks()
        }

        Event::MediaPlaybackEnded => {
            let mut mgr = manager.lock().await;
            mgr.resume(false).await;
            mgr.state.wake_idle_tasks();
        }

        Event::LidClosed => {
            let mut mgr = manager.lock().await;
            log_message("Lid closed — handling event...");

            // clone the lid_close_action and lock_action before mutably borrowing
            if let Some(cfg) = &mgr.state.cfg {
                let lid_close = cfg.lid_close_action.clone();
                let suspend_action_opt = cfg.actions.iter().find(|a| a.kind == IdleAction::Suspend).cloned();
                let lock_action_opt = cfg.actions.iter().find(|a| a.kind == IdleAction::LockScreen).cloned();
                let _ = cfg;

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
                        log_message(&format!("Running custom lid-close command: {}", cmd));
                        match run_command_detached(&cmd).await {
                            Ok(pid) => log_debug_message(&format!("Custom lid-close command started with PID {}", pid.pid)),
                            Err(e) => log_error_message(&format!("Failed to run custom lid-close command: {}", e)),
                        }
                    }
                    LidCloseAction::Ignore => {
                        log_message("Lid close ignored by config");
                    }
                }
            }
        }

        Event::LidOpened => {
            let mut mgr = manager.lock().await;
            log_message("Lid opened — handling event...");

            if let Some(cfg) = &mgr.state.cfg {
                match &cfg.lid_open_action {
                    LidOpenAction::Wake => {
                        mgr.resume(false).await;
                        mgr.reset().await;
                        mgr.state.wake_idle_tasks();
                    }                   
                    LidOpenAction::Custom(cmd) => {
                        log_message(&format!("Running custom lid-open command: {}", cmd));
                        let _ = run_command_detached(cmd).await;
                    }
                    LidOpenAction::Ignore => {
                        log_message("Lid open ignored by config");
                    }
                }
            }
        }

        Event::LoginctlLock => {
            let mut mgr = manager.lock().await;
            log_debug_message("loginctl lock-session received — handling lock...");

            // Skip if already locked
            if mgr.state.lock.is_locked {
                log_message("Already locked, ignoring loginctl lock-session event");
                return;
            }

            // Clone the lock-command before mutably borrowing
            let lock_cmd_opt = if let Some(cfg) = &mgr.state.cfg {
                cfg.actions.iter()
                    .find(|a| a.kind == IdleAction::LockScreen)
                    .and_then(|a| a.lock_command.clone())
            } else {
                None
            };

            // Now we can mutably borrow
            mgr.state.lock.is_locked = true;
            mgr.state.lock_notify.notify_one();
            
            // Run the lock-command if it exists
            if let Some(lock_cmd) = lock_cmd_opt {
                log_message(&format!("Running lock-command: {}", lock_cmd));
                match run_command_detached(&lock_cmd).await {
                    Ok(pid) => {
                        mgr.state.lock.process_info = Some(pid.clone());
                    }
                    Err(e) => {
                        log_message(&format!("Failed to run lock-command: {}", e));
                    }
                }
            } else {
                log_message("No lock-command configured");
            }
            
            // Advance past lock so subsequent actions (like DPMS/suspend) can trigger
            mgr.advance_past_lock().await;
            
            // Wake the lock watcher loop
            mgr.state.wake_idle_tasks();
        }

        Event::LoginctlUnlock => {
            let mut mgr = manager.lock().await;
            log_debug_message("loginctl unlock-session received — resetting state...");
            
            // Reset the manager state as if user activity occurred
            mgr.reset().await;
            mgr.state.lock_notify.notify_waiters();
            mgr.state.wake_idle_tasks();
        }
    }
}
