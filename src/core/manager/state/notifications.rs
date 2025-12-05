/// Manages notification state
#[derive(Debug, Default)]
pub struct NotificationState {
    pub notification_sent: bool,
}

impl NotificationState {
    pub fn mark_sent(&mut self) {
        self.notification_sent = true;
    }

    pub fn reset(&mut self) {
        self.notification_sent = false;
    }
}
