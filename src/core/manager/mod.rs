pub mod actions;
pub mod brightness;
pub mod helpers;
pub mod idle_loops;
pub mod inhibitors;
pub mod media;
pub mod processes;
pub mod resume;
pub mod state;
pub mod tasks;

use std::{sync::Arc, time::{Duration, Instant}};
use tokio::{
    time::sleep
};

pub use self::state::ManagerState;
use crate::{
    config::model::{IdleAction, StasisConfig, CombinedConfig}, 
    core::manager::{
        actions::run_action,
        brightness::restore_brightness,
        helpers::{profile_to_stasis_config, has_lock_action, get_lock_index},
        inhibitors::{incr_active_inhibitor, InhibitorSource},
        processes::{is_process_running, run_command_detached},
        tasks::TaskManager,
    },
    sinfo,
    sdebug,
    serror,
};

#[derive(Debug)]
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

    pub fn new_with_profiles(combined: &CombinedConfig) -> Self {
        Self {
            state: ManagerState::new_with_profiles(combined),
            tasks: TaskManager::new(),
        }
    }

    pub async fn trigger_instant_actions(&mut self) {
        if self.state.actions.instants_triggered {
            return;
        }

        let instant_actions = self.state.get_active_instant_actions();

        sinfo!("Stasis", "Triggering instant actions...");
        for action in instant_actions {
            run_action(self, &action).await;
        }

        // Mark instants as triggered
        self.state.actions.instants_triggered = true;

        // Update action_index: set to one past the last instant action
        let actions = self.state.get_active_actions();
        let mut index = 0;
        for a in actions {
            if a.is_instant() {
                index += 1;
            } else {
                break;
            }
        }
        self.state.actions.action_index = index;

        sdebug!("Stasis", "Instant actions complete, starting at index {}", index);
    }

    pub fn reset_instant_actions(&mut self) {
        self.state.actions.instants_triggered = false;
        sdebug!("Stasis", "Instant actions reset");
    }

    // Check if any actions have surpassed their timeout period
    pub async fn check_timeouts(&mut self) {
        if self.state.inhibitors.paused || self.state.inhibitors.manually_paused {
            return;
        }

        let now = Instant::now();
        
        let action_index = self.state.actions.action_index;
        let is_locked = self.state.lock.is_locked;
        let last_activity = self.state.timing.last_activity;
        let debounce = self.state.debounce.main_debounce;
        let notification_sent = self.state.notifications.notification_sent;
        
        let (notify_enabled, notify_seconds) = if let Some(ref cfg) = self.state.cfg {
            (cfg.notify_before_action, cfg.notify_seconds_before)
        } else {
            (false, 0)
        };

        let actions = self.state.get_active_actions_mut();

        if actions.is_empty() {
            return;
        }

        let index = action_index.min(actions.len() - 1);

        if matches!(actions[index].kind, IdleAction::LockScreen) && is_locked {
            return;
        }

        let timeout = Duration::from_secs(actions[index].timeout as u64);
        let base_timeout_instant = if let Some(last_trig) = actions[index].last_triggered {
            last_trig + timeout
        } else if index > 0 {
            if let Some(prev_trig) = actions[index - 1].last_triggered {
                prev_trig + timeout
            } else {
                let base = debounce.unwrap_or(last_activity);
                base + timeout
            }
        } else {
            let base = debounce.unwrap_or(last_activity);
            base + timeout
        };

        let has_notification = actions[index].notification.is_some();
        let notify_duration = Duration::from_secs(notify_seconds);
        let actual_action_fire_instant = if notify_enabled && has_notification {
            base_timeout_instant + notify_duration
        } else {
            base_timeout_instant
        };

        if notify_enabled && has_notification && !notification_sent {
            if now >= base_timeout_instant {
                if let Some(ref notification_msg) = actions[index].notification {
                    let notify_cmd = format!("notify-send -a Stasis '{}'", notification_msg);
                    sinfo!("Stasis", "Sending pre-action notification: {}", notification_msg);
                    
                    if let Err(e) = run_command_detached(&notify_cmd).await {
                        serror!("Stasis", "Failed to send notification: {}", e);
                    }
                    
                    self.state.notifications.mark_sent();
                    self.state.notify.notify_one();
                }
                return;
            } else {
                return;
            }
        }

        if now < actual_action_fire_instant {
            return;
        }

        let (action_clone, actions_len) = {
            let actions = self.state.get_active_actions_mut();
            let action_clone = actions[index].clone();
            actions[index].last_triggered = Some(now);
            (action_clone, actions.len())
        };

        self.state.notifications.reset();

        // Determine which queue to add resume command to
        if action_clone.resume_command.is_some() {
            let has_lock = has_lock_action(self);
            let lock_index = get_lock_index(self);
            
            if !matches!(action_clone.kind, IdleAction::LockScreen) {
                if has_lock {
                    if let Some(lock_idx) = lock_index {
                        if index < lock_idx {
                            // Pre-lock action - save for unlock
                            sdebug!("Stasis", "Queueing pre-lock resume for: {}", action_clone.name);
                            self.state.actions.pre_lock_resume_queue.push(action_clone.clone());
                            
                            // DPMS exception: Also add to post-lock queue so it fires on reset while locked
                            if matches!(action_clone.kind, IdleAction::Dpms) {
                                sdebug!("Stasis", "DPMS action - also queing for post-lock resume: {}", action_clone.name);
                                self.state.actions.post_lock_resume_queue.push(action_clone.clone());
                            }
                        } else {
                            // Post-lock action - fire on next reset while locked
                            sdebug!("Stasis", "Queueing post-lock resume for: {}", action_clone.name);
                            self.state.actions.post_lock_resume_queue.push(action_clone.clone());
                        }
                    }
                } else {
                    // No lock action - add to post-lock queue (will fire on reset)
                    sdebug!("Stasis", "Queueing resume command for: {}", action_clone.name);
                    self.state.actions.post_lock_resume_queue.push(action_clone.clone());
                }
            }
        }

        self.state.actions.action_index += 1;
        if self.state.actions.action_index < actions_len {
            self.state.actions.resume_commands_fired = false;
        } else {
            self.state.actions.action_index = actions_len - 1;
        }

        run_action(self, &action_clone).await;
    }

    // Updated reset method - fire post-lock resume commands if locked
    pub async fn reset(&mut self) {
        let cfg = match &self.state.cfg {
            Some(cfg) => Arc::clone(cfg),
            None => {
                sdebug!("Stasis", "No configuration available, skipping reset");
                return;
            }
        };
        
        if self.state.brightness.previous_brightness.is_some() 
            && self.has_non_instant_action_fired() {
            if let Err(e) = restore_brightness(&mut self.state).await {
                sinfo!("Stasis", "Failed to restore brightness: {}", e);
            }
        }
        
        let now = Instant::now();
        let debounce = Duration::from_secs(cfg.debounce_seconds as u64);
        self.state.debounce.main_debounce = Some(now + debounce);
        self.state.timing.last_activity = now;

        if !self.state.lock.is_locked {
            self.state.notifications.reset();
        }

        let is_locked = self.state.lock.is_locked;
        let cmd_to_check = self.state.lock.command.clone();

        for actions in [&mut self.state.power.default_actions, &mut self.state.power.ac_actions, &mut self.state.power.battery_actions] {
            let mut past_lock = false;
            for a in actions.iter_mut() {
                if a.is_instant() {
                    continue;
                }

                if matches!(a.kind, crate::config::model::IdleAction::LockScreen) {
                    past_lock = true;
                }

                if is_locked && past_lock {
                    continue;
                } 

                a.last_triggered = None;
            }
        }

        let (is_instant, lock_index) = {
            let actions = self.state.get_active_actions_mut();
            let index = actions.iter()
                .position(|a| a.last_triggered.is_none())
                .unwrap_or(actions.len().saturating_sub(1));
            let is_instant = !actions.is_empty() && actions[index].is_instant();
            let lock_index = if is_locked {
                actions.iter().position(|a| matches!(a.kind, crate::config::model::IdleAction::LockScreen))
            } else {
                None
            };
            (is_instant, lock_index)
        };

        if !is_locked {
            let actions = self.state.get_active_actions();
            let mut index = 0;
            for a in actions {
                if a.is_instant() {
                    index += 1;
                } else {
                    break;
                }
            }
            self.state.actions.action_index = index;
        }

        // Fire appropriate resume queue based on lock state
        if is_locked {
            // While locked, fire post-lock resume commands (only once per lock session)
            if !self.state.actions.post_lock_resumes_fired {
                self.fire_post_lock_resume_queue().await;
                self.state.actions.post_lock_resumes_fired = true;
            }
        } else {
            // Not locked, fire all resume commands
            self.fire_all_resume_queues().await;
            // Reset the flag for next lock session
            self.state.actions.post_lock_resumes_fired = false;
        }

        if is_instant {
            return;
        }

        if is_locked {
            if let Some(lock_index) = lock_index {
                let still_active = if let Some(cmd) = cmd_to_check {
                    is_process_running(&cmd).await
                } else {
                    true
                };

                if still_active {
                    self.state.actions.action_index = lock_index.saturating_add(1);
                    
                    let debounce_end = now + debounce;
                    let new_action_index = self.state.actions.action_index;
                    let actions = self.state.get_active_actions_mut();
                    if new_action_index < actions.len() {
                        actions[new_action_index].last_triggered = Some(debounce_end); 
                    } else {
                        if lock_index < actions.len() {
                            actions[lock_index].last_triggered = Some(debounce_end);
                        } 
                    }
                    
                    self.state.lock.post_advanced = true;
                } 
            } 
        }
        
        self.state.notify.notify_one();
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

            // sdebug!(
            //     "Idle",
            //     "next_action_instant: action={}, base_timeout={:?}s, notification_sent={}, next_wake={:?}s",
            //     action.name,
            //     base_timeout_instant.duration_since(self.state.start_time).as_secs(),
            //     self.state.notification_sent,
            //     next_wake_time.duration_since(self.state.start_time).as_secs()
            // );

            min_time = Some(match min_time {
                None => next_wake_time,
                Some(current_min) => current_min.min(next_wake_time),
            });
        }

        min_time
    } 

    pub async fn set_profile(&mut self, profile_name: Option<&str>) -> Result<String, String> {
        let profile_name_opt = profile_name.map(|s| s.to_string());

        // Check if profile exists
        if let Some(name) = &profile_name_opt {
            if !self.state.profile.has_profile(name) {
                return Err(format!("Profile '{}' not found", name));
            }
        }

        // Load profile or base config
        let config_to_apply = if let Some(name) = &profile_name_opt {
            let profile = self.state.profile.get_profile(name)
                .ok_or_else(|| format!("Profile '{}' not found", name))?;
            profile_to_stasis_config(profile)
        } else {
            crate::config::parser::load_combined_config()
                .map(|combined| combined.base)
                .map_err(|e| format!("Failed to load base config: {}", e))?
        };

        // Refresh app inhibitors
        self.state
            .inhibitors
            .refresh_from_profile(config_to_apply.inhibit_apps.clone());

        if let Some(app_inhibitor) = &self.state.app.app_inhibitor {
            app_inhibitor.lock().await.reset_inhibitors().await;
        }

        // Apply the config
        self.state.update_from_config(&config_to_apply).await;

        // Update active profile tracking
        self.state.profile.set_active(profile_name_opt.clone());

        if config_to_apply.monitor_media {
            // Only stop existing media if monitoring is enabled
            self.cleanup_media_monitoring().await;

            // One-shot immediate check
            let (ignore_remote, media_blacklist) = (
                config_to_apply.ignore_remote_media,
                config_to_apply.media_blacklist.clone(),
            );

            let playing = crate::core::services::media::check_media_playing(
                ignore_remote,
                &media_blacklist,
                self.state.media.media_bridge_active,
            );

            if playing {
                self.state.media.media_playing = true;
                self.state.media.media_blocking = true;
                self.state.media.mpris_media_playing = true;
                incr_active_inhibitor(self, InhibitorSource::Media).await;
            }
        } else {
            // Monitoring disabled → force-stop any running media inhibitors
            self.cleanup_media_monitoring().await;
        }

        Ok(if let Some(name) = profile_name_opt {
            format!("Switched to profile: {}", name)
        } else {
            "Switched to base configuration".to_string()
        })
    }

    fn has_non_instant_action_fired(&self) -> bool {
        let actions = self.state.get_active_actions();
        
        for action in actions {
            if !action.is_instant() && action.last_triggered.is_some() {
                return true;
            }
        }
        
        false
    }
 
    pub async fn pause(&mut self, manual: bool) {
        if manual {
            self.state.inhibitors.manually_paused = true;
            sdebug!("Stasis", "Idle timers manually paused");
        } else if !self.state.inhibitors.manually_paused {
            self.state.inhibitors.paused = true;
            sdebug!("Stasis", "Idle timers automatically paused");
        }
    }

    pub async fn resume(&mut self, manually: bool) {
        if manually {
            if self.state.inhibitors.manually_paused {
                self.state.inhibitors.manually_paused = false;
                
                if self.state.inhibitors.active_inhibitor_count == 0 {
                    self.state.inhibitors.paused = false;
                    sinfo!("Stasis", "Idle timers manually resumed");
                } else {
                    sinfo!("Stasis", "Manual paused cleaed, but {} inhibitor(s) still active", self.state.inhibitors.active_inhibitor_count);
                }
            }
        } else if !self.state.inhibitors.manually_paused && self.state.inhibitors.paused {
            // This is called by decr_active_inhibitor when count reaches 0
            self.state.inhibitors.paused = false;
            sinfo!("Stasis", "Idle timers automatically resumed");
        }
    }

    pub async fn shutdown(&mut self) {
        self.state.shutdown_flag.notify_waiters();
        sleep(Duration::from_millis(100)).await;
        self.tasks.shutdown_all().await;
    }
}
