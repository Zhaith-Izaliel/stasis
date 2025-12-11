use crate::config::model::Profile;

#[derive(Debug, Clone)]
pub struct ProfileState {
    /// Currently active profile name (None = using base config)
    pub active_profile: Option<String>,
    /// Available profiles from config
    pub available_profiles: Vec<Profile>,
}

impl Default for ProfileState {
    fn default() -> Self {
        Self {
            active_profile: None,
            available_profiles: Vec::new(),
        }
    }
}

impl ProfileState {
    pub fn new() -> Self {
        Self::default()
    }

    /// Check if a profile exists by name
    pub fn has_profile(&self, name: &str) -> bool {
        self.available_profiles.iter().any(|p| p.name == name)
    }

    /// Get a profile by name
    pub fn get_profile(&self, name: &str) -> Option<&Profile> {
        self.available_profiles.iter().find(|p| p.name == name)
    }

    /// Set the active profile
    pub fn set_active(&mut self, name: Option<String>) {
        self.active_profile = name;
    }

    /// Check if using base config (no profile active)
    pub fn is_using_base(&self) -> bool {
        self.active_profile.is_none()
    }

    /// Get list of profile names
    pub fn profile_names(&self) -> Vec<String> {
        self.available_profiles.iter().map(|p| p.name.clone()).collect()
    }

    /// Update available profiles
    pub fn update_profiles(&mut self, profiles: Vec<Profile>) {
        self.available_profiles = profiles;
        
        // Clear active profile if it no longer exists
        if let Some(active) = &self.active_profile {
            if !self.has_profile(active) {
                self.active_profile = None;
            }
        }
    }
}
