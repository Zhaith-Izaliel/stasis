use std::fmt::{Display, Formatter, Result};

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum LidCloseAction {
    Ignore,
    LockScreen,
    Suspend,
    Custom(String),
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum LidOpenAction {
    Ignore,
    Wake,
    Custom(String),
}

impl Display for LidCloseAction {
    fn fmt(&self, f: &mut Formatter<'_>) -> Result {
        match self {
            LidCloseAction::Ignore => write!(f, "ignore"),
            LidCloseAction::LockScreen => write!(f, "lock_screen"),
            LidCloseAction::Suspend => write!(f, "suspend"),
            LidCloseAction::Custom(cmd) => write!(f, "custom: {}", cmd),
        }
    }
}

impl Display for LidOpenAction {
    fn fmt(&self, f: &mut Formatter<'_>) -> Result {
        match self {
            LidOpenAction::Wake => write!(f, "wake"),
            LidOpenAction::Ignore => write!(f, "ignore"),
            LidOpenAction::Custom(cmd) => write!(f, "custom: {}", cmd),
        }
    }
}
