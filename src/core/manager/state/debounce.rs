use std::time::{Duration, Instant};

/// Manages debouncing logic for various state changes
#[derive(Debug)]
pub struct DebounceState {
    pub main_debounce: Option<Instant>,
    pub app_inhibit_debounce: Option<Instant>,
}

impl DebounceState {
    pub fn new(debounce_seconds: u32) -> Self {
        Self {
            main_debounce: Some(Instant::now() + Duration::from_secs(debounce_seconds as u64)),
            app_inhibit_debounce: None,
        }
    }

    pub fn reset_main(&mut self, debounce_seconds: u32) {
        self.main_debounce = Some(Instant::now() + Duration::from_secs(debounce_seconds as u64));
    }

    pub fn clear_main(&mut self) {
        self.main_debounce = None;
    }

    pub fn is_main_active(&self) -> bool {
        self.main_debounce.map_or(false, |d| Instant::now() < d)
    }
}

impl Default for DebounceState {
    fn default() -> Self {
        Self {
            main_debounce: None,
            app_inhibit_debounce: None,
        }
    }
}
