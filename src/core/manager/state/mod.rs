pub mod actions;
pub mod brightness;
pub mod debounce;
pub mod inhibitors;
pub mod lock;
pub mod media;
pub mod notifications;
pub mod power;
pub mod timing;

use std::{sync::Arc, time::Instant};
use tokio::sync::Notify;

use crate::{
    config::model::{IdleActionBlock, StasisConfig},
    core::manager::state::{
        actions::ActionState, 
        brightness::BrightnessState, 
        debounce::DebounceState, 
        inhibitors::InhibitorState, 
        lock::LockState, 
        media::MediaState, 
        notifications::NotificationState, 
        power::PowerState, 
        timing::TimingState
    },
    log::log_debug_message,
};

#[derive(Debug)]
pub struct ManagerState {
    pub actions: ActionState,
    pub brightness: BrightnessState,
    pub cfg: Option<Arc<StasisConfig>>,
    pub debounce: DebounceState,
    pub inhibitors: InhibitorState,
    pub lock: LockState,
    pub lock_notify: Arc<Notify>,
    pub media: MediaState,
    pub notify: Arc<Notify>,
    pub notifications: NotificationState,
    pub power: PowerState,
    pub pre_suspend_command: Option<String>,
    pub shutdown_flag: Arc<Notify>,
    pub suspend_occured: bool,
    pub timing: TimingState,
}

impl Default for ManagerState {
    fn default() -> Self {

        Self {
            actions: ActionState::default(),
            brightness: BrightnessState::default(),
            cfg: None,
            debounce: DebounceState::default(),
            inhibitors: InhibitorState::default(),
            lock: LockState::default(),
            lock_notify: Arc::new(Notify::new()),
            media: MediaState::default(),
            notify: Arc::new(Notify::new()),
            notifications: NotificationState::default(),
            power: PowerState::new_from_config(&[]),
            pre_suspend_command: None,
            shutdown_flag: Arc::new(Notify::new()),
            suspend_occured: false,
            timing: TimingState::default(),
        }
    }
}

impl ManagerState {
    pub fn new(cfg: Arc<StasisConfig>) -> Self {
        let power = PowerState::new_from_config(&cfg.actions);
        let debounce = DebounceState::new(cfg.debounce_seconds.into());

        Self {
            actions: ActionState::default(),
            brightness: BrightnessState::default(),
            cfg: Some(cfg.clone()),
            debounce,
            inhibitors: InhibitorState::default(),
            lock: LockState::from_config(&cfg),
            lock_notify: Arc::new(Notify::new()),
            media: MediaState::default(),
            notify: Arc::new(Notify::new()),
            notifications: NotificationState::default(),
            power,
            pre_suspend_command: cfg.pre_suspend_command.clone(),
            shutdown_flag: Arc::new(Notify::new()),
            suspend_occured: false,
            timing: TimingState::default(),
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
            self.reset_actions();
        }
    }

    fn reset_actions(&mut self) {
        self.actions.reset();
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
        self.brightness.reset();
        self.pre_suspend_command = cfg.pre_suspend_command.clone();

        self.power.reload_actions(&cfg.actions);

        // Reset last_triggered for active block
        for a in self.get_active_actions_mut().iter_mut() {
            a.last_triggered = None;
        }

        self.actions.reset();
        self.debounce.reset_main(cfg.debounce_seconds.into());

        self.cfg = Some(Arc::new(cfg.clone()));
        self.lock = LockState::from_config(cfg);
        self.timing.last_activity = Instant::now();

        log_debug_message(&format!(
            "Idle timers reloaded from config (active block: {})",
            self.power.current_block
        ));
    }


    pub fn wake_idle_tasks(&self) {
        self.notify.notify_waiters();
    }

    pub fn is_manually_paused(&self) -> bool {
        self.inhibitors.manually_paused
    }
}

