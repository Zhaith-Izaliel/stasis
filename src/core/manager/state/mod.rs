pub mod media;
pub mod power;

use std::{sync::Arc, time::{Duration, Instant}};
use tokio::sync::Notify;

use crate::{
    config::model::{IdleActionBlock, StasisConfig},
    core::manager::{
        processes::ProcessInfo, state::media::MediaState
    },
    log::log_debug_message,
};
use crate::core::manager::state::power::PowerState;

#[derive(Debug)]
pub struct ManagerState {
    pub action_index: usize,
    pub active_flags: ActiveFlags,
    pub active_inhibitor_count: u32,
    pub app_inhibit_debounce: Option<Instant>,
    pub brightness_device: Option<String>,
    pub cfg: Option<Arc<StasisConfig>>,
    pub compositor_managed: bool,
    pub dbus_inhibit_active: bool,
    pub debounce: Option<Instant>,
    pub instants_triggered: bool,
    pub last_activity: Instant,
    pub lock_state: LockState,
    pub lock_notify: Arc<Notify>,
    pub manually_paused: bool,
    pub max_brightness: Option<u32>,
    pub media: MediaState,
    pub notify: Arc<Notify>,
    pub notification_sent: bool,
    pub paused: bool,
    pub power: PowerState,
    pub previous_brightness: Option<u32>,
    pub pre_suspend_command: Option<String>,
    pub resume_queue: Vec<IdleActionBlock>,
    pub resume_commands_fired: bool,
    pub shutdown_flag: Arc<Notify>,
    pub start_time: Instant,
    pub suspend_occured: bool,
}

impl Default for ManagerState {
    fn default() -> Self {
        let now = Instant::now();

        Self {
            action_index: 0,
            active_flags: ActiveFlags::default(),
            active_inhibitor_count: 0,
            app_inhibit_debounce: None,
            brightness_device: None,
            cfg: None,
            compositor_managed: false,
            dbus_inhibit_active: false,
            debounce: None,
            instants_triggered: false,
            last_activity: now,
            lock_state: LockState::default(),
            manually_paused: false,
            max_brightness: None,
            media: MediaState::default(),
            notify: Arc::new(Notify::new()),
            lock_notify: Arc::new(Notify::new()),
            notification_sent: false,
            paused: false,
            power: PowerState::new_from_config(&[]),
            previous_brightness: None,
            pre_suspend_command: None,
            resume_queue: Vec::new(),
            resume_commands_fired: false,
            shutdown_flag: Arc::new(Notify::new()),
            start_time: now,
            suspend_occured: false,
        }
    }
}

impl ManagerState {
    pub fn new(cfg: Arc<StasisConfig>) -> Self {
        let power = PowerState::new_from_config(&cfg.actions);

        let now = Instant::now();
        let debounce = Some(now + Duration::from_secs(cfg.debounce_seconds as u64));

        Self {
            action_index: 0,
            active_flags: ActiveFlags::default(),
            active_inhibitor_count: 0,
            app_inhibit_debounce: None,
            brightness_device: None,
            cfg: Some(cfg.clone()),
            compositor_managed: false,
            dbus_inhibit_active: false,
            debounce,
            instants_triggered: false,
            last_activity: now,
            lock_state: LockState::from_config(&cfg),
            manually_paused: false,
            max_brightness: None,
            media: MediaState::default(),
            notify: Arc::new(Notify::new()),
            notification_sent: false,
            lock_notify: Arc::new(Notify::new()),
            paused: false,
            power,
            previous_brightness: None,
            pre_suspend_command: cfg.pre_suspend_command.clone(),
            resume_queue: Vec::new(),
            resume_commands_fired: false,
            shutdown_flag: Arc::new(Notify::new()),
            start_time: now,
            suspend_occured: false,
        }
    }

    // -------------------------
    // POWER WRAPPERS
    // -------------------------

    pub fn is_laptop(&self) -> bool {
        self.power.is_laptop()
    }

    pub fn on_battery(&self) -> Option<bool> {
        self.power.on_battery()
    }

    pub fn set_on_battery(&mut self, value: bool) {
        if self.power.set_on_battery(value) {
            self.reset_action_state();
        }
    }

    fn reset_action_state(&mut self) {
        self.action_index = 0;
        self.instants_triggered = false;
        self.notify.notify_one();
    }

    // -------------------------
    // ACTION ACCESSORS
    // -------------------------

    pub fn get_active_actions(&self) -> &[IdleActionBlock] {
        self.power.active_actions()     // CHANGED
    }

    pub fn get_active_actions_mut(&mut self) -> &mut Vec<IdleActionBlock> {
        self.power.active_actions_mut() // CHANGED
    }

    pub fn get_active_instant_actions(&self) -> Vec<IdleActionBlock> {
        self.power.active_instant_actions() // CHANGED
    }

    // -------------------------
    // CONFIG RELOAD
    // -------------------------

    pub async fn update_from_config(&mut self, cfg: &StasisConfig) {
        self.active_flags = ActiveFlags::default();
        self.previous_brightness = None;
        self.pre_suspend_command = cfg.pre_suspend_command.clone();

        // CHANGED: power logic fully delegated
        self.power.reload_actions(&cfg.actions);

        // Reset last_triggered for active block
        for a in self.get_active_actions_mut().iter_mut() {
            a.last_triggered = None;
        }

        self.reset_action_state();

        // Debounce reset
        self.debounce = Some(Instant::now() + Duration::from_secs(cfg.debounce_seconds as u64));

        self.cfg = Some(Arc::new(cfg.clone()));
        self.lock_state = LockState::from_config(cfg);
        self.last_activity = Instant::now();

        log_debug_message(&format!(
            "Idle timers reloaded from config (active block: {})",
            self.power.current_block
        ));
    }

    // -------------------------
    // MEDIA STATE
    // -------------------------

    pub fn wake_idle_tasks(&self) {
        self.notify.notify_waiters();
    }

    pub fn set_locked(&mut self, locked: bool) {
        self.lock_state.is_locked = locked;
    }

    pub fn compositor_managed(&self) -> bool {
        self.compositor_managed
    }

    pub fn set_compositor_managed(&mut self, value: bool) {
        self.compositor_managed = value;
    }

    pub fn is_manually_paused(&self) -> bool {
        self.manually_paused
    }
}

//
// LockState, ActiveFlags unchanged
//

#[derive(Debug, Clone)]
pub struct LockState {
    pub is_locked: bool,
    pub process_info: Option<ProcessInfo>,
    pub command: Option<String>,
    pub last_advanced: Option<std::time::Instant>,
    pub post_advanced: bool,
}

impl Default for LockState {
    fn default() -> Self {
        Self {
            is_locked: false,
            process_info: None,
            command: None,
            last_advanced: None,
            post_advanced: false,
        }
    }
}

impl LockState {
    pub fn from_config(cfg: &StasisConfig) -> Self {
        use crate::config::model::IdleAction;

        let lock_action = cfg.actions.iter().find(|a| a.kind == IdleAction::LockScreen);

        let command = lock_action.map(|a| {
            if let Some(ref lock_cmd) = a.lock_command {
                lock_cmd.clone()
            } else {
                a.command.clone()
            }
        });

        Self {
            is_locked: false,
            process_info: None,
            command,
            last_advanced: None,
            post_advanced: false,
        }
    }
}

#[derive(Debug)]
pub struct ActiveFlags {
    pub pre_suspend_triggered: bool,
    pub brightness_captured: bool,
}

impl Default for ActiveFlags {
    fn default() -> Self {
        Self {
            pre_suspend_triggered: false,
            brightness_captured: false,
        }
    }
}
