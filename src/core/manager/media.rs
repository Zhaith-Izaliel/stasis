use std::sync::Arc;
use tokio::sync::Mutex;

use crate::{
    core::manager::{
        Manager, inhibitors::{InhibitorSource, decr_active_inhibitor, incr_active_inhibitor}, state::media::MediaState
    }, sdebug, serror, sinfo
};

impl Manager {
    /// Synchronously check if media is playing and update inhibitor state accordingly
    pub async fn recheck_media(&mut self) {
        // Read ignore_remote_media + media blacklist from config
        let (ignore_remote, media_blacklist) = match &self.state.cfg {
            Some(cfg) => (cfg.ignore_remote_media, cfg.media_blacklist.clone()),
            None => (false, Vec::new()),
        };

        // Synchronous check (pactl + mpris)
        let playing = crate::core::services::media::check_media_playing(
            ignore_remote,
            &media_blacklist,
            false,
        );

        // Only change state via the helpers so behaviour stays consistent
        if playing && !self.state.media.media_playing {
            // Call the same helper the monitor uses
            incr_active_inhibitor(self, InhibitorSource::Media).await;
            self.state.media.media_playing = true;
        } else if !playing && self.state.media.media_playing {
            decr_active_inhibitor(self, InhibitorSource::Media).await;
            self.state.media.media_playing = false;
        }
    }

    /// Restart media monitoring tasks based on current config
    pub async fn restart_media_monitoring(manager_arc: Arc<Mutex<Manager>>) {
        let should_monitor = {
            let mgr = manager_arc.lock().await;
            mgr.state.cfg
                .as_ref()
                .map(|c| c.monitor_media)
                .unwrap_or(true)
        };

        if should_monitor {
            sdebug!("MPRIS", "Restarting media monitor...");
            if let Err(e) = crate::core::services::media::spawn_media_monitor_dbus(
                Arc::clone(&manager_arc),
            )
            .await
            {
                serror!("Stasis", "Failed to restart media monitor: {}", e);
            }
        }
    }

    /// Clean up all media monitoring state and inhibitors
    pub async fn cleanup_media_monitoring(&mut self) {
        sinfo!("Stasis", "Cleaning up media monitoring state");

        // Clear standard media inhibitor
        if self.state.media.media_playing {
            decr_active_inhibitor(self, InhibitorSource::Media).await;
        }

        // Clear browser tab inhibitors
        let tab_count = self.state.media.browser_playing_tab_count;
        for _ in 0..tab_count {
            decr_active_inhibitor(self, InhibitorSource::Media).await;
        }

        // Reset media state
        self.state.media = MediaState::default();
    }
}
