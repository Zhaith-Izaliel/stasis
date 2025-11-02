pub mod actions;
pub mod helpers;
pub mod idle_loops;
pub mod state;
pub mod tasks;

use std::{sync::Arc, time::{Duration, Instant}};
use tokio::{
    task::JoinHandle, 
    time::sleep
};

pub use self::state::ManagerState;
use crate::{
    config::model::{IdleAction, StasisConfig}, 
    core::manager::{
        actions::{is_process_running, run_command_detached, run_command_silent},
        helpers::{restore_brightness, run_action},
    }, 
    log::log_message
};

pub struct Manager {
    pub state: ManagerState,
    pub spawned_tasks: Vec<JoinHandle<()>>,
    pub idle_task_handle: Option<JoinHandle<()>>,
    pub lock_task_handle: Option<JoinHandle<()>>,
    pub media_task_handle: Option<JoinHandle<()>>,
    pub input_task_handle: Option<JoinHandle<()>>,
}

impl Manager {
    pub fn new(cfg: Arc<StasisConfig>) -> Self {
        Self {
            state: ManagerState::new(cfg),
            spawned_tasks: Vec::new(),
            idle_task_handle: None,
            lock_task_handle: None,
            media_task_handle: None,
            input_task_handle: None,
        }
    }

    pub async fn trigger_instant_actions(&mut self) {
        if self.state.instants_triggered {
            return;
        }

        let instant_actions = self.state.instant_actions.clone();

        log_message("Triggering instant actions at startup...");
        for action in instant_actions {
            run_action(self, &action).await;
        }

        self.state.instants_triggered = true;
    }

    pub fn reset_instant_actions(&mut self) {
        self.state.instants_triggered = false;
        log_message("Instant actions reset; they can trigger again");
    }

    // Called when libinput service resets (on user activity)
    pub async fn reset(&mut self) {
        let cfg = match &self.state.cfg {
            Some(cfg) => Arc::clone(cfg),
            None => {
                log_message("No configuration available, skipping reset");
                return;
            }
        };
        
        // Restore brightness if needed
        if self.state.previous_brightness.is_some() {
            if let Err(e) = restore_brightness(&mut self.state).await {
                log_message(&format!("Failed to restore brightness: {}", e));
            }
        }
        
        let now = Instant::now();
        self.state.last_activity_display = now;
        let debounce = Duration::from_secs(cfg.debounce_seconds as u64);
        self.state.debounce = Some(now + debounce);
        self.state.last_activity = now + debounce;

        // Clear only actions that are before or equal to the current stage
        for actions in [&mut self.state.default_actions, &mut self.state.ac_actions, &mut self.state.battery_actions] {
            let mut past_lock = false;
            for a in actions.iter_mut() {
                if matches!(a.kind, crate::config::model::IdleAction::LockScreen) {
                    past_lock = true;
                }
                // if locked, preserve stages past lock (so dpms/suspend remain offset correctly)
                if self.state.lock_state.is_locked && past_lock {
                    continue;
                }
                a.last_triggered = None;
            }
        }        
        let block_name = if !self.state.ac_actions.is_empty() || !self.state.battery_actions.is_empty() {
            match self.state.on_battery() {
                Some(true) => "battery",
                Some(false) => "ac",
                None => "default",
            }
        } else {
            "default"
        };

        // Only update current_block if it changed
        if self.state.current_block.as_deref() != Some(block_name) {
            self.state.current_block = Some(block_name.to_string());
        }

        // Recompute action_index for the current block
        let actions = match block_name {
            "ac" => &mut self.state.ac_actions,
            "battery" => &mut self.state.battery_actions,
            "default" => &mut self.state.default_actions,
            _ => unreachable!(),
        };

        // Skip instant actions here. handled elsewhere
        let index = actions.iter()
            .position(|a| a.last_triggered.is_none())
            .unwrap_or(actions.len().saturating_sub(1));

        // Reset action_index
        if !self.state.lock_state.is_locked {
            self.state.action_index = 0;
        }

        if !actions.is_empty() && actions[index].is_instant() {
            return;
        }

        if self.state.lock_state.is_locked {
            if let Some(lock_index) = actions.iter().position(|a| matches!(a.kind, crate::config::model::IdleAction::LockScreen)) {
                // Check if lock process is still running
                let cmd_to_check = self.state.lock_state.command.clone();
                let still_active = if let Some(cmd) = cmd_to_check {
                    is_process_running(&cmd).await
                } else {
                    true // Assume lock is active if no command is specified
                };

                if still_active && self.state.lock_state.is_locked {
                    // Always advance to one past lock when locked
                    self.state.action_index = lock_index.saturating_add(1);
                    
                    let debounce_end = now + debounce;
                    if self.state.action_index < actions.len() {
                        actions[self.state.action_index].last_triggered = Some(debounce_end); 
                    } else {
                        // If at the end, reset last_triggered for the last action
                        if lock_index < actions.len() {
                            actions[lock_index].last_triggered = Some(debounce_end);
                        } 
                    }
                    
                    self.state.lock_state.post_advanced = true;
                } 
            } 
        }
        
        self.fire_resume_queue().await;
        self.state.notify.notify_one();
    }

    // Check whether we have been idle enough to elapse one of the timeouts
    pub async fn check_timeouts(&mut self) {
        if self.state.paused || self.state.manually_paused {
            return;
        }
        
        let now = Instant::now();
        if let Some(until) = self.state.debounce {
            if now <= until {
                return;
            } else {
                self.state.debounce = None;
            }
        }
        
        // Determine which block to use
        let block_name = if !self.state.ac_actions.is_empty() || !self.state.battery_actions.is_empty() {
            match self.state.on_battery() {
                Some(true) => "battery",
                Some(false) => "ac",
                None => "default",
            }
        } else {
            "default"
        };
        
        // Only update if changed
        if self.state.current_block.as_deref() != Some(block_name) {
            self.state.current_block = Some(block_name.to_string());
        }
            
        // Get reference to the right actions Vec
        let actions = match block_name {
            "ac" => &mut self.state.ac_actions,
            "battery" => &mut self.state.battery_actions,
            "default" => &mut self.state.default_actions,
            _ => unreachable!(),
        };
        
        if actions.is_empty() {
            return;
        }
        
        let index = self.state.action_index.min(actions.len() - 1);
        
        // Skip lock if already locked
        if matches!(actions[index].kind, crate::config::model::IdleAction::LockScreen) 
            && self.state.lock_state.is_locked {
            return;
        }
        
        // Calculate elapsed - read the data we need before calling run_action
        let last_ref = actions[index].last_triggered.unwrap_or(self.state.last_activity);
        let elapsed = now.duration_since(last_ref);
        let timeout = actions[index].timeout;
        
        if elapsed >= Duration::from_secs(timeout as u64) {
            // Clone the action to pass to run_action (avoids borrow conflict)
            let action_clone = actions[index].clone();
            
            // Update timing BEFORE running action
            if index < actions.len() {
                actions[index].last_triggered = Some(now);
            }
            
            // Advance index
            self.state.action_index += 1;
            if self.state.action_index < actions.len() {
                actions[self.state.action_index].last_triggered = Some(now);
                self.state.resume_commands_fired = false;
            } else {
                self.state.action_index = actions.len() - 1;
            }

            // Add to resume_queue, except if already queued
            if matches!(action_clone.kind, IdleAction::LockScreen) {
                // Do NOT push lock actions to resume_queue
            } else if action_clone.resume_command.is_some() {
                self.state.resume_queue.push(action_clone.clone());
            }
            
            // Now we can call run_action with full mutable self access
            run_action(self, &action_clone).await;
        }
    }

    pub async fn fire_resume_queue(&mut self) {
        if self.state.resume_queue.is_empty() {
            return;
        }

        log_message(&format!("Firing {} queued resume command(s)...", self.state.resume_queue.len()));

        for action in self.state.resume_queue.drain(..) {
            if let Some(resume_cmd) = &action.resume_command {
                log_message(&format!("Running resume command for action: {}", action.name));
                if let Err(e) = run_command_detached(resume_cmd).await {
                    log_message(&format!("Failed to run resume command '{}': {}", resume_cmd, e));
                }
            }
        }

        self.state.resume_queue.clear();
    }

    pub fn next_action_instant(&self) -> Option<Instant> {
        if self.state.paused || self.state.manually_paused {
            return None;
        }

        // Determine actions based on current block
        let actions = match self.state.current_block.as_deref() {
            Some("ac") => &self.state.ac_actions,
            Some("battery") => &self.state.battery_actions,
            _ => &self.state.default_actions,
        };

        if actions.is_empty() {
            return None;
        }

        let mut min_time: Option<Instant> = None;

        for action in actions.iter() {
            // Skip lock if already locked
            if matches!(action.kind, IdleAction::LockScreen) && self.state.lock_state.is_locked {
                continue;
            }

            let last = action.last_triggered.unwrap_or(self.state.last_activity);
            let mut next_time = last + Duration::from_secs(action.timeout as u64);

            // Respect debounce
            if let Some(debounce_end) = self.state.debounce {
                next_time = next_time.max(debounce_end);
            }

            min_time = Some(match min_time {
                None => next_time,
                Some(current_min) => current_min.min(next_time),
            });
        }

        min_time
    }

    pub async fn trigger_pre_suspend(&mut self, manual: bool) {
        if !manual {
            self.state.suspend_occured = true;
        }

        if let Some(cmd) = &self.state.pre_suspend_command {
            log_message(&format!("Running pre-suspend command: {}", cmd));

            // Wait for it to finish (synchronous)
            match run_command_silent(cmd).await {
                Ok(_) => log_message("Pre-suspend command finished"),
                Err(e) => log_message(&format!("Pre-suspend command failed: {}", e)),
            }
        }
    }

    pub async fn update_power_source(&mut self) {
        match self.state.on_battery() {
            Some(true) => {
                // on battery, proceed
            }
            Some(false) | None => {
                return;
            }
        }
    }
  
    pub async fn advance_past_lock(&mut self) {
        log_message("Advancing state past lock stage...");
        self.state.lock_state.post_advanced = true;
        self.state.lock_state.last_advanced = Some(Instant::now());
    }

    pub fn set_post_lock_debounce(&mut self) {
        let now = Instant::now();
        if let Some(cfg) = &self.state.cfg {
            let debounce = Duration::from_secs(cfg.debounce_seconds as u64);
            self.state.debounce = Some(now + debounce);

            // Also mark the next action's last_triggered if we're past lock
            let actions = match self.state.current_block.as_deref() {
                Some("ac") => &mut self.state.ac_actions,
                Some("battery") => &mut self.state.battery_actions,
                _ => &mut self.state.default_actions,
            };

            if self.state.action_index < actions.len() {
                actions[self.state.action_index].last_triggered = Some(now + debounce);
            }
        }

        self.state.notify.notify_one();
    }

    pub async fn pause(&mut self, manual: bool) {
        if manual {
            self.state.manually_paused = true;
            log_message("Idle timers manually paused");
        } else if !self.state.manually_paused {
            self.state.paused = true;
            log_message("Idle timers automatically paused");
        }
    }

    pub async fn resume(&mut self, manually: bool) {
        if manually {
            if self.state.manually_paused {
                self.state.manually_paused = false;
                
                if self.state.active_inhibitor_count == 0 {
                    self.state.paused = false;
                    log_message("Idle timers manually resumed");
                } else {
                    log_message(&format!(
                        "Manual pause cleared, but {} inhibitor(s) still active - timers remain paused",
                        self.state.active_inhibitor_count
                    ));
                }
            }
        } else if !self.state.manually_paused && self.state.paused {
            // This is called by decr_active_inhibitor when count reaches 0
            self.state.paused = false;
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

    pub async fn shutdown(&mut self) {
        self.state.shutdown_flag.notify_waiters();

        sleep(Duration::from_millis(200)).await;

        if let Some(handle) = self.idle_task_handle.take() {
            handle.abort();
        }

        if let Some(handle) = self.lock_task_handle.take() {
            handle.abort();
        }

        if let Some(handle) = self.input_task_handle.take() {
            handle.abort();
        }

        for handle in self.spawned_tasks.drain(..) {
            handle.abort();
        }
    }
}

