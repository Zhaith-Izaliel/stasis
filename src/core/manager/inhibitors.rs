use crate::core::manager::Manager;
use eventline::event_debug_scoped;

/// Tracks active inhibitors from different sources (apps, media, etc.)
/// Still maintains `active_inhibitor_count` for backward compatibility/logging
impl Manager {
    /// Recalculate total active inhibitors from individual sources
    fn update_total_inhibitors(&mut self) {
        let total = self.state.inhibitors.active_app_inhibitors
            + self.state.inhibitors.active_media_inhibitors;
        self.state.inhibitors.active_inhibitor_count = total;
    }
}

/// Increment an app or media inhibitor
pub async fn incr_active_inhibitor(mgr: &mut Manager, source: InhibitorSource) {
    match source {
        InhibitorSource::App => mgr.state.inhibitors.active_app_inhibitors += 1,
        InhibitorSource::Media => mgr.state.inhibitors.active_media_inhibitors += 1,
    }

    // Update global count for logging/compatibility
    mgr.update_total_inhibitors();
    let now = mgr.state.inhibitors.active_inhibitor_count;
    let prev = now.saturating_sub(1);

    if prev == 0 {
        if !mgr.state.inhibitors.manually_paused {
            mgr.state.inhibitors.paused = true;
            event_debug_scoped!(
                "Inhibitors",
                "Inhibitor registered (count: {} → {}): first inhibitor active → idle timers paused",
                prev,
                now
            ).await;
        } else {
            event_debug_scoped!(
                "Inhibitors",
                "Inhibitor registered (count: {} → {}): manual pause already active",
                prev,
                now
            ).await;
        }
    } else {
        event_debug_scoped!(
            "Inhibitors",
            "Inhibitor registered (count: {} → {})",
            prev,
            now
        ).await;
    }

    // Wake idle task to recalc next timeout
    mgr.state.notify.notify_one();
}

/// Decrement an app or media inhibitor
pub async fn decr_active_inhibitor(mgr: &mut Manager, source: InhibitorSource) {
    let source_count = match source {
        InhibitorSource::App => &mut mgr.state.inhibitors.active_app_inhibitors,
        InhibitorSource::Media => &mut mgr.state.inhibitors.active_media_inhibitors,
    };

    if *source_count == 0 {
        event_debug_scoped!(
            "Inhibitors",
            "decr_active_inhibitor called for {:?} but count already 0 (possible mismatch)",
            source
        ).await;
        return;
    }

    *source_count = source_count.saturating_sub(1);
    mgr.update_total_inhibitors();

    let now = mgr.state.inhibitors.active_inhibitor_count;
    let prev = now + 1;

    if now == 0 {
        if !mgr.state.inhibitors.manually_paused {
            mgr.state.inhibitors.paused = false;
            mgr.reset().await;

            event_debug_scoped!(
                "Inhibitors",
                "Inhibitor removed (count: {} → {}): no more inhibitors → idle timers resumed",
                prev,
                now
            ).await;
        } else {
            event_debug_scoped!(
                "Inhibitors",
                "Inhibitor removed (count: {} → {}): manual pause still active, timers remain paused",
                prev,
                now
            ).await;
        }

        mgr.state.notify.notify_one();
    } else {
        event_debug_scoped!(
            "Inhibitors",
            "Inhibitor removed (count: {} → {})",
            prev,
            now
        ).await;
    }
}

/// Source of inhibitor (app or media)
#[derive(Debug, Copy, Clone)]
pub enum InhibitorSource {
    App,
    Media,
}
