use crate::core::manager::{inhibitors::{decr_active_inhibitor, incr_active_inhibitor, InhibitorSource}, Manager};
use eventline::event_debug_scoped;

/// Manages media playback state from multiple sources
#[derive(Debug, Default, Clone)]
pub struct MediaState {
    pub browser_media_playing: bool,
    pub browser_playing_tab_count: usize,
    pub media_playing: bool,
    pub media_bridge_active: bool,
    pub media_blocking: bool,
    pub mpris_media_playing: bool,
    pub override_media_inhibitor: bool,
}

impl MediaState {
    pub fn is_any_playing(&self) -> bool {
        if self.media_bridge_active {
            self.browser_media_playing || self.media_playing
        } else {
            self.media_playing
        }
    }
    
    pub fn get_inhibitor_count(&self) -> usize {
        if self.media_bridge_active {
            self.browser_playing_tab_count
        } else if self.media_playing {
            1
        } else {
            0
        }
    }
    
    pub async fn force_stop_media(&mut self, mgr: &mut Manager) {
        if self.is_any_playing() {
            decr_active_inhibitor(mgr, InhibitorSource::Media).await;
            self.media_playing = false;
            self.media_blocking = false;
            self.mpris_media_playing = false;
            self.browser_media_playing = false;
            self.browser_playing_tab_count = 0;
        }
    }
    
    pub async fn restore_media(&mut self, mgr: &mut Manager, previous_state: &MediaState) {
        if previous_state.is_any_playing() {
            incr_active_inhibitor(mgr, InhibitorSource::Media).await;
            self.media_playing = previous_state.media_playing;
            self.media_blocking = previous_state.media_blocking;
            self.mpris_media_playing = previous_state.mpris_media_playing;
            self.browser_media_playing = previous_state.browser_media_playing;
            self.browser_playing_tab_count = previous_state.browser_playing_tab_count;
        }
    }
    
    /// Reset media state for profile changes
    /// 
    /// CRITICAL: Browser tabs use per-tab inhibitor counting!
    /// Must decrement once for MPRIS + once PER BROWSER TAB
    pub async fn reset_for_profile_change(&mut self, mgr: &mut Manager) {
        event_debug_scoped!("Stasis", "Resetting media state for profile change");
        
        // MPRIS: decrement once if active
        if self.mpris_media_playing {
            event_debug_scoped!("Stasis", "Clearing MPRIS media inhibitor");
            decr_active_inhibitor(mgr, InhibitorSource::Media).await;
        }
        
        // Browser: decrement once PER TAB (each tab increments separately!)
        let tab_count = self.browser_playing_tab_count;
        if tab_count > 0 {
            event_debug_scoped!("Stasis", "Clearing {} browser tab inhibitors", tab_count);
            for _ in 0..tab_count {
                decr_active_inhibitor(mgr, InhibitorSource::Media).await;
            }
        }
        
        // Clear all state
        self.media_playing = false;
        self.media_blocking = false;
        self.mpris_media_playing = false;
        self.browser_media_playing = false;
        self.browser_playing_tab_count = 0;
        // Don't reset media_bridge_active - it's independent of profile
        
        event_debug_scoped!("Stasis", "Media state reset complete");
    }
    
    pub fn log_state(&self) { 
        // Copy or clone all fields we need
        let bridge_active = self.media_bridge_active;
        let browser_playing = self.browser_media_playing;
        let tabs = self.browser_playing_tab_count;
        let mpris_playing = self.media_playing;

        event_debug_scoped!(
            "Media",
            "Media State: bridge_active={}, browser_playing={} (tabs={}), mpris_playing={}",
            bridge_active,
            browser_playing,
            tabs,
            mpris_playing,
        );
    }
}
