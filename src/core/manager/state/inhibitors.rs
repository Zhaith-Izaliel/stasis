/// Manages pause state and inhibitors
#[derive(Debug, Default)]
pub struct InhibitorState {
    pub active_inhibitor_count: u32,
    pub dbus_inhibit_active: bool,
    pub manually_paused: bool,
    pub paused: bool,
    pub compositor_managed: bool,
}

impl InhibitorState {
    pub fn is_inhibited(&self) -> bool {
        self.paused || self.manually_paused || self.dbus_inhibit_active
    }

    pub fn total_count(&self) -> u32 {
        self.active_inhibitor_count
    }
}
