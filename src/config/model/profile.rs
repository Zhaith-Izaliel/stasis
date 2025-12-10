use super::{IdleActionBlock, AppInhibitPattern, LidCloseAction, LidOpenAction};
use super::LockDetectionType;

#[derive(Debug, Clone)]
pub struct Profile {
    pub name: String,
    pub actions: Vec<IdleActionBlock>,
    pub debounce_seconds: u8,
    pub inhibit_apps: Vec<AppInhibitPattern>,
    pub monitor_media: bool,
    pub ignore_remote_media: bool,
    pub media_blacklist: Vec<String>,
    pub pre_suspend_command: Option<String>,
    pub respect_wayland_inhibitors: bool,
    pub lid_close_action: LidCloseAction,
    pub lid_open_action: LidOpenAction,
    pub notify_on_unpause: bool,
    pub notify_before_action: bool,
    pub notify_seconds_before: u64,
    pub lock_detection_type: LockDetectionType,
}
