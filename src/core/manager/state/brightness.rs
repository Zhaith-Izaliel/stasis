/// Manages brightness control state
#[derive(Debug, Default)]
pub struct BrightnessState {
    pub device: Option<String>,
    pub max_brightness: Option<u32>,
    pub previous_brightness: Option<u32>,
    pub brightness_captured: bool,
}

impl BrightnessState {
    /// Store current brightness value (called by capture functions)
    pub fn store(&mut self, value: u32, max: u32, device: String) {
        if !self.brightness_captured {
            self.previous_brightness = Some(value);
            self.max_brightness = Some(max);
            self.device = Some(device);
            self.brightness_captured = true;
        }
    }

    /// Store brightness without device info (fallback)
    pub fn store_simple(&mut self, value: u32) {
        if !self.brightness_captured {
            self.previous_brightness = Some(value);
            self.brightness_captured = true;
        }
    }

    /// Get stored brightness and device info for restore
    pub fn get_restore_info(&self) -> (Option<u32>, Option<String>, Option<u32>) {
        (
            self.previous_brightness,
            self.device.clone(),
            self.max_brightness,
        )
    }

    /// Clear all stored brightness data
    pub fn clear(&mut self) {
        self.previous_brightness = None;
        self.max_brightness = None;
        self.device = None;
        self.brightness_captured = false;
    }

    /// Reset without clearing previous brightness (for reuse)
    pub fn reset(&mut self) {
        self.previous_brightness = None;
        self.brightness_captured = false;
    }
}
