use std::time::{Duration, Instant};

/// Manages timing and activity tracking
#[derive(Debug)]
pub struct TimingState {
    pub start_time: Instant,
    pub last_activity: Instant,
}

impl TimingState {
    pub fn new() -> Self {
        let now = Instant::now();
        Self {
            start_time: now,
            last_activity: now,
        }
    }

    pub fn update_activity(&mut self) {
        self.last_activity = Instant::now();
    }

    pub fn idle_duration(&self) -> Duration {
        self.last_activity.elapsed()
    }
}

impl Default for TimingState {
    fn default() -> Self {
        Self::new()
    }
}
