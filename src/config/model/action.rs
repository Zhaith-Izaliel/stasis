use std::{fmt::{Display, Formatter, Result}, time::Instant};

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum IdleAction {
    Brightness,
    Dpms,
    LockScreen,
    Suspend,
    Custom,
}

impl Display for IdleAction {
    fn fmt(&self, f: &mut Formatter<'_>) -> Result {
        match self {
            IdleAction::Brightness => write!(f, "brightness"),
            IdleAction::Dpms => write!(f, "dpms"),
            IdleAction::LockScreen => write!(f, "lock_screen"),
            IdleAction::Suspend => write!(f, "suspend"),
            IdleAction::Custom => write!(f, "custom"),
        }
    }
}

#[derive(Debug, Clone)]
pub struct IdleActionBlock {
    pub name: String,
    pub timeout: u64,
    pub command: String,
    pub kind: IdleAction,
    pub resume_command: Option<String>,
    pub lock_command: Option<String>,
    pub last_triggered: Option<Instant>,
    pub notification: Option<String>,
    pub notify_seconds_before: Option<u64>,
}

impl IdleActionBlock {
    pub fn is_instant(&self) -> bool {
        self.timeout == 0
    }
    
    pub fn has_resume_command(&self) -> bool {
        self.resume_command.is_some()
    }

    pub fn get_lock_command(&self) -> &str {
        if self.command == "loginctl lock-session" {
            self.lock_command.as_deref().unwrap_or(&self.command)
        } else {
            &self.command
        }
    }
}
