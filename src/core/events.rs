// Author: Dustin Pilgrim
// License: MIT

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ActivityKind {
    Any
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MediaState {
    Idle,
    PlayingLocal,
    PlayingRemote,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PowerState {
    OnAC,
    OnBattery,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Event {
    Tick {
        now_ms: u64,
    },

    UserActivity {
        kind: ActivityKind,
        now_ms: u64,
    },

    MediaStateChanged {
        state: MediaState,
        now_ms: u64,
    },

    PowerChanged {
        state: PowerState,
        now_ms: u64,
    },

    LidClosed {
        now_ms: u64,
    },
    LidOpened {
        now_ms: u64,
    },

    SessionLocked {
        now_ms: u64,
    },
    SessionUnlocked {
        now_ms: u64,
    },

    ManualPause {
        now_ms: u64,
    },
    ManualResume {
        now_ms: u64,
    },

    /// Manually run a configured plan step by name/kind.
    /// (e.g. "startup", "dpms", "lock_screen", "suspend", "early-dpms")
    ManualTrigger {
        now_ms: u64,
        name: String,
    },

    /// A timer-driven pause ("pause for"/"pause until") ended.
    /// This is *not* the same as the user explicitly running `stasis resume`.
    PauseExpired {
        now_ms: u64,
        message: String,
    },

    ProfileChanged {
        name: String,
        now_ms: u64,
    },

    PrepareForSleep {
        now_ms: u64,
    },
    ResumedFromSleep {
        now_ms: u64,
    },

    AppInhibitorCount {
        count: u64,
        now_ms: u64,
    },
    MediaInhibitorCount {
        count: u64,
        now_ms: u64,
    },
}

impl Event {
    pub fn now_ms(&self) -> u64 {
        match self {
            Event::Tick { now_ms }
            | Event::UserActivity { now_ms, .. }
            | Event::MediaStateChanged { now_ms, .. }
            | Event::PowerChanged { now_ms, .. }
            | Event::LidClosed { now_ms }
            | Event::LidOpened { now_ms }
            | Event::SessionLocked { now_ms }
            | Event::SessionUnlocked { now_ms }
            | Event::ManualPause { now_ms }
            | Event::ManualResume { now_ms }
            | Event::ManualTrigger { now_ms, .. }
            | Event::PauseExpired { now_ms, .. }
            | Event::ProfileChanged { now_ms, .. }
            | Event::PrepareForSleep { now_ms }
            | Event::ResumedFromSleep { now_ms }
            | Event::AppInhibitorCount { now_ms, .. }
            | Event::MediaInhibitorCount { now_ms, .. } => *now_ms,
        }
    }
}
