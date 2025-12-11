use std::sync::Arc;
use tokio::sync::Mutex;

use crate::{core::manager::{Manager, actions::run_action, helpers::advance_past_lock, processes::run_command_detached}, sdebug, serror, sinfo};
use crate::{config::model::{IdleAction, LidCloseAction, LidOpenAction}};

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
            sinfo!("Stasis", "System resumed from suspend - resetting state");
            
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
            mgr.state.wake_idle_tasks()
        }

        Event::MediaPlaybackEnded => {
            let mut mgr = manager.lock().await;
            mgr.resume(false).await;
            mgr.state.wake_idle_tasks();
        }

        Event::LidClosed => {
            let mut mgr = manager.lock().await;
            sinfo!("Stasis", "Lid closed - handling event...");

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
                        sinfo!("Stasis", "Running custom lid-close command: {}", cmd);
                        match run_command_detached(&cmd).await {
                            Ok(pid) => sdebug!("Stasis", "Custom lid-close command started with PID {}", pid.pid),
                            Err(e) => serror!("Stasis", "Failed to run custom lid-close command: {}", e),
                        }
                    }
                    LidCloseAction::Ignore => {
                        sdebug!("Stasis", "Lid close ignored by config");
                    }
                }
            }
        }

        Event::LidOpened => {
            let mut mgr = manager.lock().await;
            sinfo!("Stasis", "Lid opened - handling event...");

            if let Some(cfg) = &mgr.state.cfg {
                match &cfg.lid_open_action {
                    LidOpenAction::Wake => {
                        mgr.resume(false).await;
                        mgr.reset().await;
                        mgr.state.wake_idle_tasks();
                    }                   
                    LidOpenAction::Custom(cmd) => {
                        sinfo!("Stasis", "Running custom lid-open command: {}", cmd);
                        let _ = run_command_detached(cmd).await;
                    }
                    LidOpenAction::Ignore => {
                        sdebug!("Stasis", "Lid open ignored by config");
                    }
                }
            }
        }

        Event::LoginctlLock => {
            let mut mgr = manager.lock().await;
            sdebug!("Stasis", "loginctl lock-session received - handling lock...");

            if mgr.state.lock.is_locked {
                sinfo!("Stasis", "Already locked, ignoring loginctl lock-session event");
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
            
            // Run the lock-command if it exists
            if let Some(lock_cmd) = lock_cmd_opt {
                sinfo!("Stasis", "Running lock-command: {}", lock_cmd);
                match run_command_detached(&lock_cmd).await {
                    Ok(pid) => mgr.state.lock.process_info = Some(pid.clone()),
                    Err(e) => serror!("Stasis", "Failed to run lock-command: {}", e),
                }
            } else {
                sinfo!("Stasis", "No lock-command configured");
            }
            
            advance_past_lock(&mut mgr).await; 
            mgr.state.wake_idle_tasks();
        }

        Event::LoginctlUnlock => {
            let mut mgr = manager.lock().await;
            sdebug!("Stasis", "loginctl unlock-session received - resetting state...");
            
            mgr.reset().await;
            mgr.state.lock_notify.notify_waiters();
            mgr.state.wake_idle_tasks();
        }
    }
}
