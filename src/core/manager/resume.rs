use crate::{
    core::manager::Manager,
    config::model::IdleAction,
    core::manager::processes::run_command_detached,
};
use eventline::event_debug_scoped;

impl Manager {
    // fire only pre-lock resume commands (on unlock)
    pub async fn fire_pre_lock_resume_queue(&mut self) {
        let queue_len = self.state.actions.pre_lock_resume_queue.len();
        if queue_len == 0 {
            return;
        }

        event_debug_scoped!(
            "Stasis",
            "Firing {} pre-lock resume command(s) on unlock...",
            queue_len
        );

        for action in self.state.actions.pre_lock_resume_queue.drain(..) {
            if let Some(resume_cmd) = &action.resume_command {
                let action_name_for_log = action.name.clone();
                let cmd_clone = resume_cmd.clone();

                event_debug_scoped!(
                    "Stasis",
                    "Running pre-lock resume command for action: {}",
                    action_name_for_log
                );

                if let Err(e) = run_command_detached(&cmd_clone).await {
                    let _action_name_for_err = action.name.clone();
                    event_debug_scoped!(
                        "Stasis",
                        "Failed to run resume command '{}': {}",
                        cmd_clone,
                        e
                    );
                }
            }
        }

        self.state.actions.pre_lock_resume_queue.clear();
    }

    // fire post-lock resume commands (while locked)
    pub async fn fire_post_lock_resume_queue(&mut self) {
        let queue_len = self.state.actions.post_lock_resume_queue.len();
        if queue_len == 0 {
            return;
        }

        event_debug_scoped!(
            "Stasis",
            "Firing {} post-lock resume command(s) while locked...",
            queue_len
        );

        let actions_to_fire: Vec<_> = self.state.actions.post_lock_resume_queue.drain(..).collect();
        for action in actions_to_fire.into_iter().rev() {
            if let Some(resume_cmd) = &action.resume_command {
                let action_name_for_log = action.name.clone();
                let action_name_for_retain = action_name_for_log.clone();
                let cmd_clone = resume_cmd.clone();

                event_debug_scoped!(
                    "Stasis",
                    "Running resume command for action: {}",
                    action_name_for_log
                );

                if let Err(e) = run_command_detached(&cmd_clone).await {
                    event_debug_scoped!(
                        "Stasis",
                        "Failed to run resume command '{}': {}",
                        cmd_clone,
                        e
                    );
                }

                // If this was a DPMS action, remove it from pre-lock queue
                if matches!(action.kind, IdleAction::Dpms) {
                    self.state.actions.pre_lock_resume_queue.retain(|a| {
                        !matches!(a.kind, IdleAction::Dpms) || a.name != action_name_for_retain
                    });

                    let action_name_for_log2 = action.name.clone();
                    event_debug_scoped!(
                        "Stasis",
                        "DPMS resume fired post-lock, removed from pre-lock queue: {}",
                        action_name_for_log2
                    );
                }
            }
        }

        self.state.actions.post_lock_resume_queue.clear();
    }

    // fire all resume commands (on unlock or if no lock)
    pub async fn fire_all_resume_queues(&mut self) {
        // Calculate actual number of commands
        let mut actual_count = self.state.actions.post_lock_resume_queue.len();
        let dpms_names_in_post: Vec<String> = self
            .state
            .actions
            .post_lock_resume_queue
            .iter()
            .filter(|a| matches!(a.kind, IdleAction::Dpms))
            .map(|a| a.name.clone())
            .collect();

        for action in &self.state.actions.pre_lock_resume_queue {
            if matches!(action.kind, IdleAction::Dpms)
                && dpms_names_in_post.contains(&action.name)
            {
                continue;
            }
            actual_count += 1;
        }

        if actual_count == 0 {
            return;
        }

        event_debug_scoped!(
            "Stasis",
            "Firing {} total resume command(s)...",
            actual_count
        );

        // Fire post-lock commands first
        let mut fired_dpms_names: Vec<String> = Vec::new();

        for action in self.state.actions.post_lock_resume_queue.drain(..) {
            if let Some(resume_cmd) = &action.resume_command {
                let action_name_for_log = action.name.clone();
                let action_name_for_vec = action_name_for_log.clone();
                let cmd_clone = resume_cmd.clone();

                event_debug_scoped!(
                    "Stasis",
                    "Running post-lock resume command for action: {}",
                    action_name_for_log
                );

                if let Err(e) = run_command_detached(&cmd_clone).await {
                    event_debug_scoped!(
                        "Stasis",
                        "Failed to run resume command '{}': {}",
                        cmd_clone,
                        e
                    );
                }

                if matches!(action.kind, IdleAction::Dpms) {
                    fired_dpms_names.push(action_name_for_vec);
                }
            }
        }

        // Fire pre-lock commands
        for action in self.state.actions.pre_lock_resume_queue.drain(..) {
            let action_name_for_log = action.name.clone();
            if matches!(action.kind, IdleAction::Dpms)
                && fired_dpms_names.contains(&action_name_for_log)
            {
                event_debug_scoped!(
                    "Stasis",
                    "Skipping duplicate DPMS resume for: {}",
                    action_name_for_log
                );
                continue;
            }

            if let Some(resume_cmd) = &action.resume_command {
                let cmd_clone = resume_cmd.clone();
                let action_name_for_log2 = action.name.clone();

                event_debug_scoped!(
                    "Stasis",
                    "Running pre-lock resume command for action: {}",
                    action_name_for_log2
                );

                if let Err(e) = run_command_detached(&cmd_clone).await {
                    event_debug_scoped!(
                        "Stasis",
                        "Failed to run resume command '{}': {}",
                        cmd_clone,
                        e
                    );
                }
            }
        }

        self.state.actions.pre_lock_resume_queue.clear();
        self.state.actions.post_lock_resume_queue.clear();
    }
}
