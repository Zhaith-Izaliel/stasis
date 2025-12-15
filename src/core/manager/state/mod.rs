pub mod app;
pub mod actions;
pub mod debounce;
pub mod inhibitors;
pub mod lock;
pub mod media;
pub mod notifications;
pub mod power;
pub mod profile;
pub mod timing;

use std::{sync::Arc, time::Instant};
use tokio::sync::Notify;

use crate::{
    config::model::{CombinedConfig, IdleActionBlock, StasisConfig},
    core::manager::state::{
        app::AppState,
        actions::ActionState, 
        debounce::DebounceState, 
        inhibitors::InhibitorState, 
        lock::LockState, 
        media::MediaState, 
        notifications::NotificationState, 
        power::PowerState, 
        profile::ProfileState,
        timing::TimingState
    }, sdebug,
};

#[derive(Debug)]
pub struct ManagerState {
    pub app: AppState,
    pub actions: ActionState,
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
    pub profile: ProfileState,
    pub shutdown_flag: Arc<Notify>,
    pub suspend_occured: bool,
    pub timing: TimingState,
}

impl Default for ManagerState {
    fn default() -> Self {
        Self {
            app: AppState::default(),
            actions: ActionState::default(),
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
            profile: ProfileState::default(),
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

        let mut inhibitors = InhibitorState::default();
        inhibitors.inhibit_apps = cfg.inhibit_apps.clone();
        
        Self {
            app: AppState::default(),
            actions: ActionState::default(),
            cfg: Some(cfg.clone()),
            debounce,
            inhibitors,
            lock: LockState::from_config(&cfg),
            lock_notify: Arc::new(Notify::new()),
            media: MediaState::default(),
            notify: Arc::new(Notify::new()),
            notifications: NotificationState::default(),
            power,
            pre_suspend_command: cfg.pre_suspend_command.clone(),
            profile: ProfileState::default(),
            shutdown_flag: Arc::new(Notify::new()),
            suspend_occured: false,
            timing: TimingState::default(),
        }
    }

    /// NEW: Initialize with combined config (base + profiles)
    pub fn new_with_profiles(combined: &CombinedConfig) -> Self {
        let cfg = Arc::new(combined.base.clone());
        let mut state = Self::new(cfg);
        
        // Load available profiles
        state.profile.update_profiles(combined.profiles.clone());
        
        // Set active profile if specified
        if let Some(active) = &combined.active_profile {
            state.profile.set_active(Some(active.clone()));
        }
        
        state
    }

    /// Power wrappers
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

    /// Action wrappers
    pub fn get_active_actions(&self) -> &[IdleActionBlock] {
        self.power.active_actions()
    }

    pub fn get_active_actions_mut(&mut self) -> &mut Vec<IdleActionBlock> {
        self.power.active_actions_mut()
    }

    pub fn get_active_instant_actions(&self) -> Vec<IdleActionBlock> {
        self.power.active_instant_actions()
    }

    /// Config reload
    pub async fn update_from_config(&mut self, cfg: &StasisConfig) {
        self.pre_suspend_command = cfg.pre_suspend_command.clone();

        self.power.reload_actions(&cfg.actions);

        // Reset last_triggered for active block
        for a in self.get_active_actions_mut().iter_mut() {
            a.last_triggered = None;
        }

        self.actions.reset();
        self.debounce.reset_main(cfg.debounce_seconds.into());

        self.inhibitors.inhibit_apps = cfg.inhibit_apps.clone();

        self.cfg = Some(Arc::new(cfg.clone()));
        self.lock = LockState::from_config(cfg);
        self.timing.last_activity = Instant::now();

        sdebug!("Stasis", "Idle timers reloaded from config (active block: {})", self.power.current_block);
    }

    /// NEW: Reload profiles from combined config
    pub async fn reload_profiles(&mut self, combined: &CombinedConfig) {
        self.profile.update_profiles(combined.profiles.clone());
        sdebug!("Stasis", "Reloaded {} profile(s)", combined.profiles.len());
    }

    pub fn wake_idle_tasks(&self) {
        self.notify.notify_waiters();
    }

    pub fn is_manually_paused(&self) -> bool {
        self.inhibitors.manually_paused
    }
}
