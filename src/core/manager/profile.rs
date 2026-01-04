use super::Manager;
use super::helpers::profile_to_stasis_config;
use crate::sdebug;

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
        
        // Reset app inhibitor
        if let Some(app_inhibitor) = &self.state.app.app_inhibitor {
            app_inhibitor.lock().await.reset_inhibitors().await;
        }
        
        // Reset media inhibitor - FIXED: properly handle browser tab counting
        {
            use crate::core::manager::inhibitors::{InhibitorSource, decr_active_inhibitor};
            use crate::sdebug;
            
            sdebug!("Stasis", "Resetting media state for profile change");
            
            // MPRIS media: decrement once if active
            if self.state.media.mpris_media_playing {
                sdebug!("Stasis", "Clearing MPRIS media inhibitor");
                decr_active_inhibitor(self, InhibitorSource::Media).await;
            }
            
            // Browser media: decrement once PER TAB (not just once!)
            let tab_count = self.state.media.browser_playing_tab_count;
            if tab_count > 0 {
                sdebug!("Stasis", "Clearing {} browser tab inhibitors", tab_count);
                for _ in 0..tab_count {
                    decr_active_inhibitor(self, InhibitorSource::Media).await;
                }
            }
            
            // Clear all state
            self.state.media.media_playing = false;
            self.state.media.media_blocking = false;
            self.state.media.mpris_media_playing = false;
            self.state.media.browser_media_playing = false;
            self.state.media.browser_playing_tab_count = 0;
            // Don't reset media_bridge_active - it's independent of profile
            
            sdebug!("Stasis", "Media state reset complete");
        }
        
        // Refresh app inhibitors config
        self.state
            .inhibitors
            .refresh_from_profile(config_to_apply.inhibit_apps.clone());
        
        // Apply the config (this updates all state flags)
        self.state.update_from_config(&config_to_apply).await;

        let media_enabled = config_to_apply.monitor_media;

        if !media_enabled {
            use crate::core::manager::inhibitors::{InhibitorSource, decr_active_inhibitor};

            sdebug!("Stasis", "Profile does not enable media; force disabling all media handling");

            // Clear MPRIS inhibitor if active
            if self.state.media.mpris_media_playing {
                decr_active_inhibitor(self, InhibitorSource::Media).await;
            }

            // Clear browser tab inhibitors
            let tab_count = self.state.media.browser_playing_tab_count;
            for _ in 0..tab_count {
                decr_active_inhibitor(self, InhibitorSource::Media).await;
            }

            // Fully disable media state
            self.state.media.media_playing = false;
            self.state.media.media_blocking = false;
            self.state.media.mpris_media_playing = false;
            self.state.media.browser_media_playing = false;
            self.state.media.browser_playing_tab_count = 0;
            self.state.media.media_bridge_active = false;
        }
       
        // Update active profile tracking
        self.state.profile.set_active(profile_name_opt.clone());
        
        // Trigger instant actions immediately
        self.trigger_instant_actions().await;
        
        // Schedule immediate recheck of app inhibitors
        // (Media will be picked up by MPRIS events naturally)
        if let Some(app_inhibitor) = &self.state.app.app_inhibitor {
            let inhibitor = std::sync::Arc::clone(app_inhibitor);
            tokio::spawn(async move {
                // Small delay to let config fully settle
                tokio::time::sleep(tokio::time::Duration::from_millis(50)).await;
                
                let mut guard = inhibitor.lock().await;
                let running = guard.is_any_app_running().await;
                
                if running {
                    let mut mgr = guard.manager.lock().await;
                    if !guard.inhibitor_active {
                        use crate::core::manager::inhibitors::{InhibitorSource, incr_active_inhibitor};
                        incr_active_inhibitor(&mut mgr, InhibitorSource::App).await;
                        drop(mgr);
                        guard.inhibitor_active = true;
                    }
                }
            });
        }
        
        Ok(if let Some(name) = profile_name_opt {
            format!("Switched to profile: {}", name)
        } else {
            "Switched to base configuration".to_string()
        })
    }
}
