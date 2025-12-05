use crate::log::log_message;

/// Manages media playback state from multiple sources
#[derive(Debug, Default)]
pub struct MediaState {
    pub browser_media_playing: bool,
    pub browser_playing_tab_count: usize,
    pub media_playing: bool,
    pub media_bridge_active: bool,
    pub media_blocking: bool,
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

    pub fn log_state(&self) {
        log_message(&format!(
            "Media State: bridge_active={}, browser_playing={} (tabs={}), mpris_playing={}",
            self.media_bridge_active,
            self.browser_media_playing,
            self.browser_playing_tab_count,
            self.media_playing,
        ));
    }
}
