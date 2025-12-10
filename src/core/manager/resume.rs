use crate::{
    core::manager::Manager,
    config::model::IdleAction,
    core::manager::processes::run_command_detached,
    log::{log_message, log_debug_message},
};

impl Manager {
    // fire only pre-lock resume commands (on unlock)
    pub async fn fire_pre_lock_resume_queue(&mut self) {
        if self.state.actions.pre_lock_resume_queue.is_empty() {
            return;
        }

        log_message(&format!(
            "Firing {} pre-lock resume command(s) on unlock...", 
            self.state.actions.pre_lock_resume_queue.len()
        ));

        for action in self.state.actions.pre_lock_resume_queue.drain(..) {
            if let Some(resume_cmd) = &action.resume_command {
                log_message(&format!("Running pre-lock resume command for action: {}", action.name));
                if let Err(e) = run_command_detached(resume_cmd).await {
                    log_message(&format!("Failed to run resume command '{}': {}", resume_cmd, e));
                }
            }
        }

        self.state.actions.pre_lock_resume_queue.clear();
    }

    // fire post-lock resume commands (while locked)
    pub async fn fire_post_lock_resume_queue(&mut self) {
        if self.state.actions.post_lock_resume_queue.is_empty() {
            return;
        }

        log_message(&format!(
            "Firing {} post-lock resume command(s) while locked...", 
            self.state.actions.post_lock_resume_queue.len()
        ));

        let actions_to_fire: Vec<_> = self.state.actions.post_lock_resume_queue.drain(..).collect();
        for action in actions_to_fire.into_iter().rev() {
            if let Some(resume_cmd) = &action.resume_command {
                log_message(&format!("Running resume command for action: {}", action.name));
                if let Err(e) = run_command_detached(resume_cmd).await {
                    log_message(&format!("Failed to run resume command '{}': {}", resume_cmd, e));
                }
                
                // If this was a DPMS action, remove it from pre-lock queue since we just fired it
                if matches!(action.kind, IdleAction::Dpms) {
                    self.state.actions.pre_lock_resume_queue.retain(|a| {
                        !matches!(a.kind, IdleAction::Dpms) || a.name != action.name
                    });
                    log_debug_message(&format!("DPMS resume fired post-lock, removed from pre-lock queue: {}", action.name));
                }
            }
        }

        self.state.actions.post_lock_resume_queue.clear();
    }

    // fire all resume commands (on unlock or if no lock)
    pub async fn fire_all_resume_queues(&mut self) {
        // Calculate the actual number of commands that will fire
        // (accounting for DPMS actions that appear in both queues)
        let mut actual_count = self.state.actions.post_lock_resume_queue.len();
        let dpms_names_in_post: Vec<String> = self.state.actions.post_lock_resume_queue
            .iter()
            .filter(|a| matches!(a.kind, IdleAction::Dpms))
            .map(|a| a.name.clone())
            .collect();
        
        // Count pre-lock actions, but skip DPMS actions that are already in post-lock
        for action in &self.state.actions.pre_lock_resume_queue {
            if matches!(action.kind, IdleAction::Dpms) && dpms_names_in_post.contains(&action.name) {
                continue; // Don't count this one, it's a duplicate
            }
            actual_count += 1;
        }
            
        if actual_count == 0 {
            return;
        }

        log_message(&format!("Firing {} total resume command(s)...", actual_count));

        // Fire post-lock commands first and track which DPMS actions we fire
        let mut fired_dpms_names: Vec<String> = Vec::new();
        
        for action in self.state.actions.post_lock_resume_queue.drain(..) {
            if let Some(resume_cmd) = &action.resume_command {
                log_message(&format!("Running post-lock resume command for action: {}", action.name));
                if let Err(e) = run_command_detached(resume_cmd).await {
                    log_message(&format!("Failed to run resume command '{}': {}", resume_cmd, e));
                }
                
                // Track DPMS actions we've fired
                if matches!(action.kind, IdleAction::Dpms) {
                    fired_dpms_names.push(action.name.clone());
                }
            }
        }

        // Fire pre-lock commands, but skip any DPMS actions we already fired
        for action in self.state.actions.pre_lock_resume_queue.drain(..) {
            // Skip DPMS actions that were already fired from post-lock queue
            if matches!(action.kind, IdleAction::Dpms) && fired_dpms_names.contains(&action.name) {
                log_debug_message(&format!("Skipping duplicate DPMS resume for: {}", action.name));
                continue;
            }
            
            if let Some(resume_cmd) = &action.resume_command {
                log_message(&format!("Running pre-lock resume command for action: {}", action.name));
                if let Err(e) = run_command_detached(resume_cmd).await {
                    log_message(&format!("Failed to run resume command '{}': {}", resume_cmd, e));
                }
            }
        }

        self.state.actions.pre_lock_resume_queue.clear();
        self.state.actions.post_lock_resume_queue.clear();
    }
} 
