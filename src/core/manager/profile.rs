use super::Manager;
use super::helpers::profile_to_stasis_config;

impl Manager {
    pub async fn set_profile(&mut self, profile_name: Option<&str>) -> Result<String, String> {
        let profile_name_opt = profile_name.map(|s| s.to_string());
        
        // Check if profile exists
        if let Some(name) = &profile_name_opt {
            if !self.state.profile.has_profile(name) {
                return Err(format!("Profile '{}' not found", name));
            }
        }
        
        // Load profile or base config
        let config_to_apply = if let Some(name) = &profile_name_opt {
            let profile = self.state.profile.get_profile(name)
                .ok_or_else(|| format!("Profile '{}' not found", name))?;
            profile_to_stasis_config(profile)
        } else {
            crate::config::parser::load_combined_config()
                .map(|combined| combined.base)
                .map_err(|e| format!("Failed to load base config: {}", e))?
        };
        
        // Refresh app inhibitors
        self.state
            .inhibitors
            .refresh_from_profile(config_to_apply.inhibit_apps.clone());
        
        // split manager lock scope
        {
            let _mgr = &self.state; // only read what is needed
        }

        // now call app_inhibitor reset without holding manager lock
        if let Some(app_inhibitor) = &self.state.app.app_inhibitor {
            app_inhibitor.lock().await.reset_inhibitors().await;
        }
        
        // Apply the config (this updates all state flags)
        self.state.update_from_config(&config_to_apply).await;
        
        // Update active profile tracking
        self.state.profile.set_active(profile_name_opt.clone());
        
        Ok(if let Some(name) = profile_name_opt {
            format!("Switched to profile: {}", name)
        } else {
            "Switched to base configuration".to_string()
        })
    }
}
