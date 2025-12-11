use crate::config::model::AppInhibitPattern;

/// Manages pause state and inhibitors
#[derive(Debug)]
pub struct InhibitorState {
    pub active_inhibitor_count: u32,
    pub active_app_inhibitors: u32,
    pub active_media_inhibitors: u32,
    pub dbus_inhibit_active: bool,
    pub manually_paused: bool,
    pub paused: bool,
    pub compositor_managed: bool,
    pub inhibit_apps: Vec<AppInhibitPattern>,
}

impl Default for InhibitorState {
    fn default() -> Self {
        Self {
            active_inhibitor_count: 0,
            active_app_inhibitors: 0,
            active_media_inhibitors: 0,
            dbus_inhibit_active: false,
            manually_paused: false,
            paused: false,
            compositor_managed: false,
            inhibit_apps: Vec::new(),
        }
    }
}

impl InhibitorState {
    pub fn is_inhibited(&self) -> bool {
        self.paused || self.manually_paused || self.dbus_inhibit_active
    }
    
    pub fn total_count(&self) -> u32 {
        self.active_inhibitor_count
    }

    pub fn refresh_from_profile(&mut self, patterns: Vec<AppInhibitPattern>) {
        // Update patterns
        self.inhibit_apps = patterns;

        // Reset state related to the old profile
        self.active_inhibitor_count = 0;
        self.paused = false;
        self.manually_paused = false;
    }
}
