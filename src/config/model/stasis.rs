use super::{IdleActionBlock, AppInhibitPattern, LidCloseAction, LidOpenAction};
use super::LockDetectionType;

#[derive(Debug, Clone)]
pub struct StasisConfig {
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

#[derive(Debug, Clone)]
pub struct CombinedConfig {
    pub base: StasisConfig,
    pub profiles: Vec<super::Profile>,
    pub active_profile: Option<String>,
}

impl Default for StasisConfig {
    fn default() -> Self {
        Self {
            actions: Vec::new(),
            debounce_seconds: 0,
            inhibit_apps: Vec::new(),

            monitor_media: false,
            ignore_remote_media: false,
            media_blacklist: Vec::new(),

            pre_suspend_command: None,
            respect_wayland_inhibitors: false,

            lid_close_action: LidCloseAction::Ignore,
            lid_open_action: LidOpenAction::Ignore,

            notify_on_unpause: false,
            notify_before_action: false,
            notify_seconds_before: 0,

            lock_detection_type: LockDetectionType::Process,
        }
    }
}
