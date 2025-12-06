pub mod actions;
pub mod brightness;
pub mod helpers;
pub mod idle_loops;
pub mod inhibitors;
pub mod processes;
pub mod state;
pub mod tasks;

use std::{sync::Arc, time::{Duration, Instant}};
use tokio::{
    time::sleep
};

pub use self::state::ManagerState;
use crate::{
    config::model::{IdleAction, StasisConfig}, 
    core::manager::{
        state::media::MediaState,
        actions::run_action,
        brightness::restore_brightness,
        inhibitors::{decr_active_inhibitor, incr_active_inhibitor},
        processes::{is_process_running, run_command_detached},
        tasks::TaskManager,
    }, 
    log::{log_debug_message, log_error_message, log_message}
};

pub struct Manager {
    pub state: ManagerState,
    pub tasks: TaskManager,
}

impl Manager {
    pub fn new(cfg: Arc<StasisConfig>) -> Self {
        Self {
            state: ManagerState::new(cfg),
            tasks: TaskManager::new(), 
        }
    }

    pub async fn trigger_instant_actions(&mut self) {
        if self.state.actions.instants_triggered {
            return;
        }

        let instant_actions = self.state.get_active_instant_actions();

        log_debug_message("Triggering instant actions at startup...");
        for action in instant_actions {
            run_action(self, &action).await;
        }

        self.state.actions.instants_triggered = true;
    }

    pub fn reset_instant_actions(&mut self) {
        self.state.actions.instants_triggered = false;
        log_debug_message("Instant actions reset; they can trigger again");
    }

    // Called when libinput service resets (on user activity)
    pub async fn reset(&mut self) {
        let cfg = match &self.state.cfg {
            Some(cfg) => Arc::clone(cfg),
            None => {
                log_debug_message("No configuration available, skipping reset");
                return;
            }
        };

        // Restore brightness if needed
        if self.state.brightness.previous_brightness.is_some() {
            if let Err(e) = restore_brightness(&mut self.state).await {
                log_message(&format!("Failed to restore brightness: {}", e));
            }
        }
        
        let now = Instant::now();
        let debounce = Duration::from_secs(cfg.debounce_seconds as u64);
        self.state.debounce.main_debounce = Some(now + debounce);
        self.state.timing.last_activity = now;

        // Reset notification state ONLY if not locked
        // When locked, we're just resetting for post-lock actions
        if !self.state.lock.is_locked {
            self.state.notifications.reset();
        }

        // Store values we need before borrowing
        let is_locked = self.state.lock.is_locked;
        let cmd_to_check = self.state.lock.command.clone();

        // Clear only actions that are before or equal to the current stage
        for actions in [&mut self.state.power.default_actions, &mut self.state.power.ac_actions, &mut self.state.power.battery_actions] {
            let mut past_lock = false;
            for a in actions.iter_mut() {
                if matches!(a.kind, crate::config::model::IdleAction::LockScreen) {
                    past_lock = true;
                }
                // if locked, preserve stages past lock (so dpms/suspend remain offset correctly)
                if is_locked && past_lock {
                    continue;
                }
                a.last_triggered = None;
            }
        }

        // Use the helper method to get active actions
        let (is_instant, lock_index) = {
            let actions = self.state.get_active_actions_mut();

            // Skip instant actions here. handled elsewhere
            let index = actions.iter()
                .position(|a| a.last_triggered.is_none())
                .unwrap_or(actions.len().saturating_sub(1));

            let is_instant = !actions.is_empty() && actions[index].is_instant();

            // Find lock index if needed
            let lock_index = if is_locked {
                actions.iter().position(|a| matches!(a.kind, crate::config::model::IdleAction::LockScreen))
            } else {
                None
            };

            (is_instant, lock_index)
        }; // Borrow ends here

        // Reset action_index
        if !is_locked {
            self.state.actions.action_index = 0;
        }

        if is_instant {
            return;
        }

        if is_locked {
            if let Some(lock_index) = lock_index {
                // Check if lock process is still running
                let still_active = if let Some(cmd) = cmd_to_check {
                    is_process_running(&cmd).await
                } else {
                    true // Assume lock is active if no command is specified
                };

                if still_active {
                    // Always advance to one past lock when locked
                    self.state.actions.action_index = lock_index.saturating_add(1);
                    
                    let debounce_end = now + debounce;
                    let new_action_index = self.state.actions.action_index;
                    let actions = self.state.get_active_actions_mut();
                    if new_action_index < actions.len() {
                        actions[new_action_index].last_triggered = Some(debounce_end); 
                    } else {
                        // If at the end, reset last_triggered for the last action
                        if lock_index < actions.len() {
                            actions[lock_index].last_triggered = Some(debounce_end);
                        } 
                    }
                    
                    self.state.lock.post_advanced = true;
                } 
            } 
        }
        
        self.fire_resume_queue().await;
        self.state.notify.notify_one();
    }

    // Check whether we have been idle enough to elapse one of the timeouts
    pub async fn check_timeouts(&mut self) {
        if self.state.inhibitors.paused || self.state.inhibitors.manually_paused {
            return;
        }

        let now = Instant::now();
        
        //log_debug_message(&format!("check_timeouts called at t={:?}", now.duration_since(self.state.start_time).as_secs()));

        // Store values we need before borrowing actions
        let action_index = self.state.actions.action_index;
        let is_locked = self.state.lock.is_locked;
        let last_activity = self.state.timing.last_activity;
        let debounce = self.state.debounce.main_debounce;
        let notification_sent = self.state.notifications.notification_sent;
        
        // Extract config values before borrowing
        let (notify_enabled, notify_seconds) = if let Some(ref cfg) = self.state.cfg {
            (cfg.notify_before_action, cfg.notify_seconds_before)
        } else {
            (false, 0)
        };

        // Get reference to the right actions Vec using helper method
        let actions = self.state.get_active_actions_mut();

        if actions.is_empty() {
            return;
        }

        let index = action_index.min(actions.len() - 1);

        // Skip lock if already locked
        if matches!(actions[index].kind, IdleAction::LockScreen) && is_locked {
            return;
        }

        // Calculate the base timeout (when notification fires OR when action fires if no notification)
        let timeout = Duration::from_secs(actions[index].timeout as u64);
        let base_timeout_instant = if let Some(last_trig) = actions[index].last_triggered {
            // Already triggered: timeout from when it last fired
            last_trig + timeout
        } else if index > 0 {
            // Not first action: fire relative to previous action
            if let Some(prev_trig) = actions[index - 1].last_triggered {
                prev_trig + timeout
            } else {
                // Previous hasn't fired yet, shouldn't happen but fallback
                last_activity + timeout
            }
        } else {
            // First action: apply debounce + timeout from last_activity
            let base = debounce.unwrap_or(last_activity);
            base + timeout
        };

        // Check if this action has a notification configured
        let has_notification = actions[index].notification.is_some();

        // Calculate when the action should actually fire
        // If notification configured: action fires at base_timeout + notify_seconds
        // If no notification: action fires at base_timeout
        let notify_duration = Duration::from_secs(notify_seconds);
        let actual_action_fire_instant = if notify_enabled && has_notification {
            base_timeout_instant + notify_duration
        } else {
            base_timeout_instant
        };

        // Handle notification phase
        if notify_enabled && has_notification && !notification_sent {
            // Time to send notification is at base_timeout_instant
            if now >= base_timeout_instant {
                log_debug_message("Notification block: time to send notification!");
                // Send notification
                if let Some(ref notification_msg) = actions[index].notification {
                    let notify_cmd = format!("notify-send -a Stasis '{}'", notification_msg);
                    log_message(&format!("Sending pre-action notification: {}", notification_msg));
                    
                    if let Err(e) = run_command_detached(&notify_cmd).await {
                        log_message(&format!("Failed to send notification: {}", e));
                    }
                    
                    self.state.notifications.mark_sent();
                    self.state.notify.notify_one();
                }
                
                return;
            } else {
                // Not time for notification yet
                return;
            }
        }

        // Check if it's time to fire the action
        if now < actual_action_fire_instant {
            // Not ready yet
            //log_debug_message(&format!("Action not ready yet, waiting {} more seconds", 
            //    (actual_action_fire_instant - now).as_secs()));
            return;
        }

        let (action_clone, actions_len) = {
            let actions = self.state.get_active_actions_mut();
            let action_clone = actions[index].clone();
            actions[index].last_triggered = Some(now);
            (action_clone, actions.len())
        };

        // Reset notification flag after firing action
        self.state.notifications.reset();

        // Advance index
        self.state.actions.action_index += 1;
        if self.state.actions.action_index < actions_len {
            // Only mark next action triggered after it actually fires
            self.state.actions.resume_commands_fired = false;
        } else {
            self.state.actions.action_index = actions_len - 1;
        }

        // Add to resume queue if needed
        if !matches!(action_clone.kind, IdleAction::LockScreen) && action_clone.resume_command.is_some() {
            self.state.actions.resume_queue.push(action_clone.clone());
        }

        // Fire the action
        run_action(self, &action_clone).await;
    }

    pub async fn fire_resume_queue(&mut self) {
        if self.state.actions.resume_queue.is_empty() {
            return;
        }

        log_message(&format!("Firing {} queued resume command(s)...", self.state.actions.resume_queue.len()));

        for action in self.state.actions.resume_queue.drain(..) {
            if let Some(resume_cmd) = &action.resume_command {
                log_message(&format!("Running resume command for action: {}", action.name));
                if let Err(e) = run_command_detached(resume_cmd).await {
                    log_message(&format!("Failed to run resume command '{}': {}", resume_cmd, e));
                }
            }
        }

        self.state.actions.resume_queue.clear();
    }

    pub fn next_action_instant(&self) -> Option<Instant> {
        if self.state.inhibitors.paused || self.state.inhibitors.manually_paused {
            return None;
        }

        // Use helper method to get active actions
        let actions = self.state.get_active_actions();

        if actions.is_empty() {
            return None;
        }

        // Get config for notification settings
        let (notify_enabled, notify_seconds) = if let Some(ref cfg) = self.state.cfg {
            (cfg.notify_before_action, cfg.notify_seconds_before)
        } else {
            (false, 0)
        };

        let mut min_time: Option<Instant> = None;

        for (i, action) in actions.iter().enumerate() {
            // Skip lock if already locked
            if matches!(action.kind, IdleAction::LockScreen) && self.state.lock.is_locked {
                continue;
            }

            // Calculate next fire time for this action
            let timeout = Duration::from_secs(action.timeout as u64);
            let base_timeout_instant = if let Some(last_trig) = action.last_triggered {
                // Already triggered: timeout from when it last fired
                last_trig + timeout
            } else if i > 0 {
                // Not first action: fire relative to previous action
                if let Some(prev_trig) = actions[i - 1].last_triggered {
                    prev_trig + timeout
                } else {
                    // Previous hasn't fired yet, shouldn't happen but fallback
                    self.state.timing.last_activity + timeout
                }
            } else {
                // First action: use debounce + timeout
                let base = self.state.debounce.main_debounce.unwrap_or(self.state.timing.last_activity);
                base + timeout
            };

            // Determine the next wake time
            let next_wake_time = if notify_enabled && action.notification.is_some() {
                if !self.state.notifications.notification_sent {
                    // Wake up at notification time (base_timeout_instant)
                    base_timeout_instant
                } else {
                    // Notification already sent, wake up at actual action time
                    let notify_duration = Duration::from_secs(notify_seconds);
                    base_timeout_instant + notify_duration
                }
            } else {
                // No notification, wake up at base_timeout_instant
                base_timeout_instant
            };

            //log_debug_message(&format!(
            //    "next_action_instant: action={}, base_timeout={:?}s, notification_sent={}, next_wake={:?}s",
            //    action.name,
            //    base_timeout_instant.duration_since(self.state.start_time).as_secs(),
            //    self.state.notification_sent,
            //    next_wake_time.duration_since(self.state.start_time).as_secs()
            //));

            min_time = Some(match min_time {
                None => next_wake_time,
                Some(current_min) => current_min.min(next_wake_time),
            });
        }

        min_time
    }

    pub async fn advance_past_lock(&mut self) {
        log_debug_message("Advancing state past lock stage...");
        self.state.lock.post_advanced = true;
        self.state.lock.last_advanced = Some(Instant::now());
    }

    pub async fn pause(&mut self, manual: bool) {
        if manual {
            self.state.inhibitors.manually_paused = true;
            log_debug_message("Idle timers manually paused");
        } else if !self.state.inhibitors.manually_paused {
            self.state.inhibitors.paused = true;
            log_message("Idle timers automatically paused");
        }
    }

    pub async fn resume(&mut self, manually: bool) {
        if manually {
            if self.state.inhibitors.manually_paused {
                self.state.inhibitors.manually_paused = false;
                
                if self.state.inhibitors.active_inhibitor_count == 0 {
                    self.state.inhibitors.paused = false;
                    log_message("Idle timers manually resumed");
                } else {
                    log_message(&format!(
                        "Manual pause cleared, but {} inhibitor(s) still active - timers remain paused",
                        self.state.inhibitors.active_inhibitor_count
                    ));
                }
            }
        } else if !self.state.inhibitors.manually_paused && self.state.inhibitors.paused {
            // This is called by decr_active_inhibitor when count reaches 0
            self.state.inhibitors.paused = false;
            log_message("Idle timers automatically resumed");
        }
    }

    pub async fn toggle_state(&mut self, inhibit: bool) {
        if inhibit {
            self.pause(true).await;
        } else {
            self.resume(true).await;
        }
    }

    pub async fn recheck_media(&mut self) {
        // read ignore_remote_media + media blacklist from cfg
        let (ignore_remote, media_blacklist) = match &self.state.cfg {
            Some(cfg) => (cfg.ignore_remote_media, cfg.media_blacklist.clone()),
            None => (false, Vec::new()),
        };

        // sync check (pactl + mpris).
        let playing = crate::core::services::media::check_media_playing(ignore_remote, &media_blacklist, false, );

        // Only change state via the helpers so behaviour stays consistent:
        if playing && !self.state.media.media_playing {
            // call the same helper the monitor uses
            incr_active_inhibitor(self).await;
            self.state.media.media_playing = true;
        } else if !playing && self.state.media.media_playing {
            decr_active_inhibitor(self).await;
            self.state.media.media_playing = false;
        }
    }

    pub async fn restart_media_monitoring(manager_arc: Arc<tokio::sync::Mutex<Manager>>) {
        let should_monitor = {
            let mgr = manager_arc.lock().await;
            mgr.state.cfg
                .as_ref()
                .map(|c| c.monitor_media)
                .unwrap_or(true)
        };

        if should_monitor {
            log_message("Restarting media monitoring...");
            if let Err(e) = crate::core::services::media::spawn_media_monitor_dbus(
                Arc::clone(&manager_arc)
            ).await {
                log_error_message(&format!("Failed to restart media monitor: {}", e));
            }
        }
    }
 
    pub async fn cleanup_media_monitoring(&mut self) {
        log_message("Cleaning up media monitoring state");
        
        // Clear standard media inhibitor
        if self.state.media.media_playing {
            decr_active_inhibitor(self).await;
        }
        
        // Clear browser tab inhibitors
        let tab_count = self.state.media.browser_playing_tab_count;
        for _ in 0..tab_count {
            decr_active_inhibitor(self).await;
        }
        
        // Reset media state
        self.state.media = MediaState::default();
    }

    pub async fn shutdown(&mut self) {
        self.state.shutdown_flag.notify_waiters();
        sleep(Duration::from_millis(200)).await;
        self.tasks.abort_all();
    }
}
