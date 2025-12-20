use crate::core::manager::{inhibitors::{decr_active_inhibitor, incr_active_inhibitor, InhibitorSource}, Manager};
use crate::sdebug;

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

    pub fn log_state(&self) { 
        sdebug!(
            "Media",
            "Media State: bridge_active={}, browser_playing={} (tabs={}), mpris_playing={}",
            self.media_bridge_active,
            self.browser_media_playing,
            self.browser_playing_tab_count,
            self.media_playing,
        );
    }
}
