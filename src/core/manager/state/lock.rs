use std::time::Instant;
use crate::{
    config::model::{IdleAction, StasisConfig},
    core::manager::processes::ProcessInfo,
};

#[derive(Debug, Clone)]
pub struct LockState {
    pub is_locked: bool,
    pub process_info: Option<ProcessInfo>,
    pub command: Option<String>,
    pub last_advanced: Option<Instant>,
    pub post_advanced: bool,
}

impl Default for LockState {
    fn default() -> Self {
        Self {
            is_locked: false,
            process_info: None,
            command: None,
            last_advanced: None,
            post_advanced: false,
        }
    }
}

impl LockState {
    pub fn from_config(cfg: &StasisConfig) -> Self {
        let lock_action = cfg.actions.iter().find(|a| a.kind == IdleAction::LockScreen);

        let command = lock_action.map(|a| {
            if let Some(ref lock_cmd) = a.lock_command {
                lock_cmd.clone()
            } else {
                a.command.clone()
            }
        });

        Self {
            is_locked: false,
            process_info: None,
            command,
            last_advanced: None,
            post_advanced: false,
        }
    }
}
