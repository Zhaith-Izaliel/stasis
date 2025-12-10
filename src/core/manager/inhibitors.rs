use crate::core::manager::Manager;
use crate::sdebug;

pub async fn incr_active_inhibitor(mgr: &mut Manager) {
    let prev = mgr.state.inhibitors.active_inhibitor_count;
    mgr.state.inhibitors.active_inhibitor_count = prev.saturating_add(1);
    let now = mgr.state.inhibitors.active_inhibitor_count;

    if prev == 0 {
        if !mgr.state.inhibitors.manually_paused {
            mgr.state.inhibitors.paused = true;
            sdebug!(
                "Inhibitors",
                "Inhibitor registered (count: {} → {}): first inhibitor active → idle timers paused",
                prev,
                now
            );
        } else {
            sdebug!(
                "Inhibitors",
                "Inhibitor registered (count: {} → {}): manual pause already active",
                prev,
                now
            );
        }
    } else {
        sdebug!(
            "Inhibitors",
            "Inhibitor registered (count: {} → {})",
            prev,
            now
        );
    }

    // wake idle task so it can recalc next timeout (if needed)
    mgr.state.notify.notify_one();
}

pub async fn decr_active_inhibitor(mgr: &mut Manager) {
    let prev = mgr.state.inhibitors.active_inhibitor_count;

    if prev == 0 {
        sdebug!(
            "Inhibitors",
            "decr_active_inhibitor called but count already 0 (possible mismatch)"
        );
        return;
    }

    mgr.state.inhibitors.active_inhibitor_count = prev.saturating_sub(1);
    let now = mgr.state.inhibitors.active_inhibitor_count;

    if now == 0 {
        if !mgr.state.inhibitors.manually_paused {
            mgr.state.inhibitors.paused = false;
            mgr.reset().await;

            sdebug!(
                "Inhibitors",
                "Inhibitor removed (count: {} → {}): no more inhibitors → idle timers resumed",
                prev,
                now
            );
        } else {
            sdebug!(
                "Inhibitors",
                "Inhibitor removed (count: {} → {}): manual pause still active, timers remain paused",
                prev,
                now
            );
        }

        // wake idle task so timeouts will be recalculated right away
        mgr.state.notify.notify_one();
    } else {
        sdebug!(
            "Inhibitors",
            "Inhibitor removed (count: {} → {})",
            prev,
            now
        );
    }
}
