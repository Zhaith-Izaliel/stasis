use std::{collections::HashSet, sync::Arc};
use tokio::sync::Mutex;
use tokio::process::Command;
use tokio::task::JoinHandle;
use serde_json::Value;
use procfs::process::all_processes;

use crate::core::manager::inhibitors::{InhibitorSource, decr_active_inhibitor, incr_active_inhibitor};
use crate::core::manager::Manager;
use crate::{sdebug, sinfo};

/// Tracks currently running apps to inhibit idle
#[derive(Debug)]
pub struct AppInhibitor {
    pub active_apps: HashSet<String>,
    pub desktop: String,
    pub manager: Arc<Mutex<Manager>>,
    pub inhibitor_active: bool, // moved here for persistent state
}

impl AppInhibitor {
    pub fn new(manager: Arc<Mutex<Manager>>) -> Self {
        let desktop = std::env::var("XDG_CURRENT_DESKTOP")
            .unwrap_or_default()
            .to_lowercase();

        sdebug!("Stasis", "XDG_CURRENT_DESKTOP detected: {}", desktop);

        Self {
            active_apps: HashSet::new(),
            desktop,
            manager,
            inhibitor_active: false,
        }
    }

    pub fn clear_active_apps(&mut self) {
        self.active_apps.clear();
        self.inhibitor_active = false; // reset on profile change
    }

    pub async fn reset_inhibitors(&mut self) {
        // clear active apps immediately
        self.active_apps.clear();
        if self.inhibitor_active {
            // spawn a detached task to decrement without holding locks
            let manager = Arc::clone(&self.manager);
            tokio::spawn(async move {
                let mut mgr = manager.lock().await;
                decr_active_inhibitor(&mut mgr, InhibitorSource::App).await;
            });
            self.inhibitor_active = false;
        }
    }

    pub async fn is_any_app_running(&mut self) -> bool {
        // If no apps configured, reset immediately
        {
            let mgr = self.manager.lock().await;
            if mgr.state.inhibitors.inhibit_apps.is_empty() {
                self.active_apps.clear();
                return false;
            }
        }

        let mut new_active_apps = HashSet::new();

        let running = match self.check_compositor_windows().await {
            Ok(result_apps) => {
                new_active_apps = result_apps;
                !new_active_apps.is_empty()
            },
            Err(_) => self.check_processes_with_tracking(&mut new_active_apps).await,
        };

        for app in &new_active_apps {
            if !self.active_apps.contains(app) {
                sinfo!("Stasis", "App inhibit active: {}", app);
            }
        }

        self.active_apps = new_active_apps;
        running
    }

    /// Check apps without holding manager lock (for immediate profile change check)
    /// Returns true if any configured apps are running
    pub async fn check_apps_immediate(&self) -> bool {
        let inhibit_apps = {
            let mgr = self.manager.lock().await;
            mgr.state.inhibitors.inhibit_apps.clone()
        };

        if inhibit_apps.is_empty() {
            return false;
        }

        // Try compositor check first
        match self.check_compositor_windows_sync(&inhibit_apps).await {
            Ok(apps) => !apps.is_empty(),
            Err(_) => self.check_processes_sync(&inhibit_apps).await,
        }
    }

    async fn check_processes_with_tracking(&mut self, new_active_apps: &mut HashSet<String>) -> bool {
        let mut any_running = false;

        let processes_iter = match all_processes() {
            Ok(iter) => iter,
            Err(_) => return false,
        };

        let inhibit_apps = {
            let mgr = self.manager.lock().await;
            mgr.state.inhibitors.inhibit_apps.clone()
        };

        for process in processes_iter {
            let process = match process {
                Ok(p) => p,
                Err(_) => continue,
            };

            let proc_name = match std::fs::read_to_string(format!("/proc/{}/comm", process.pid)) {
                Ok(name) => name.trim().to_string(),
                Err(_) => continue,
            };

            for pattern in &inhibit_apps {
                let matched = match pattern {
                    crate::config::model::AppInhibitPattern::Literal(s) => {
                        proc_name.eq_ignore_ascii_case(s)
                    }
                    crate::config::model::AppInhibitPattern::Regex(r) => r.is_match(&proc_name),
                };

                if matched {
                    new_active_apps.insert(proc_name.clone());
                    any_running = true;
                    break;
                }
            }
        }

        any_running
    }

    /// Synchronous process check (doesn't acquire manager lock)
    async fn check_processes_sync(&self, inhibit_apps: &[crate::config::model::AppInhibitPattern]) -> bool {
        let processes_iter = match all_processes() {
            Ok(iter) => iter,
            Err(_) => return false,
        };

        for process in processes_iter {
            let process = match process {
                Ok(p) => p,
                Err(_) => continue,
            };

            let proc_name = match std::fs::read_to_string(format!("/proc/{}/comm", process.pid)) {
                Ok(name) => name.trim().to_string(),
                Err(_) => continue,
            };

            for pattern in inhibit_apps {
                let matched = match pattern {
                    crate::config::model::AppInhibitPattern::Literal(s) => {
                        proc_name.eq_ignore_ascii_case(s)
                    }
                    crate::config::model::AppInhibitPattern::Regex(r) => r.is_match(&proc_name),
                };

                if matched {
                    return true;
                }
            }
        }

        false
    }

    async fn check_compositor_windows(&self) -> Result<HashSet<String>, Box<dyn std::error::Error + Send + Sync>> {
        match self.desktop.as_str() {
            "niri" => {
                let app_ids = self.try_niri_ipc().await?;
                Ok(app_ids.into_iter()
                    .filter(|app| self.should_inhibit_for_app_sync(app))
                    .collect())
            }
            "hyprland" => {
                let windows = self.try_hyprland_ipc().await?;
                Ok(windows.into_iter()
                    .filter_map(|win| win.get("app_id").and_then(|v| v.as_str()).map(|s| s.to_string()))
                    .filter(|app| self.should_inhibit_for_app_sync(app))
                    .collect())
            }
            _ => Err("No IPC available, fallback to process scan".into())
        }
    }

    /// Check compositor windows without manager lock
    async fn check_compositor_windows_sync(
        &self,
        inhibit_apps: &[crate::config::model::AppInhibitPattern],
    ) -> Result<HashSet<String>, Box<dyn std::error::Error + Send + Sync>> {
        match self.desktop.as_str() {
            "niri" => {
                let app_ids = self.try_niri_ipc().await?;
                Ok(app_ids.into_iter()
                    .filter(|app| Self::should_inhibit_static(app, inhibit_apps))
                    .collect())
            }
            "hyprland" => {
                let windows = self.try_hyprland_ipc().await?;
                Ok(windows.into_iter()
                    .filter_map(|win| win.get("app_id").and_then(|v| v.as_str()).map(|s| s.to_string()))
                    .filter(|app| Self::should_inhibit_static(app, inhibit_apps))
                    .collect())
            }
            _ => Err("No IPC available, fallback to process scan".into())
        }
    }

    async fn try_niri_ipc(&self) -> Result<Vec<String>, Box<dyn std::error::Error + Send + Sync>> {
        let output = Command::new("niri").args(&["msg", "windows"]).output().await?;
        if !output.status.success() {
            return Err(format!("niri command failed: {}", String::from_utf8_lossy(&output.stderr)).into());
        }
        let text = String::from_utf8(output.stdout)?;
        Ok(text.lines()
            .filter_map(|line| line.strip_prefix("  App ID: "))
            .map(|s| s.trim_matches('"').to_string())
            .collect())
    }

    async fn try_hyprland_ipc(&self) -> Result<Vec<Value>, Box<dyn std::error::Error + Send + Sync>> {
        let output = Command::new("hyprctl").args(&["clients", "-j"]).output().await?;
        if !output.status.success() {
            return Err(format!("hyprctl command failed: {}", String::from_utf8_lossy(&output.stderr)).into());
        }

        let clients: Vec<Value> = serde_json::from_slice(&output.stdout)?;
        let windows = clients.into_iter().map(|mut client| {
            if let Some(class) = client.get("class").cloned() {
                client.as_object_mut().unwrap().insert("app_id".to_string(), class);
            }
            client
        }).collect();

        Ok(windows)
    }

    fn should_inhibit_for_app_sync(&self, app_id: &str) -> bool {
        let inhibit_apps = {
            let mgr = futures::executor::block_on(self.manager.lock());
            mgr.state.inhibitors.inhibit_apps.clone()
        };

        for pattern in &inhibit_apps {
            let matched = match pattern {
                crate::config::model::AppInhibitPattern::Literal(s) => self.app_id_matches(s, app_id),
                crate::config::model::AppInhibitPattern::Regex(r) => r.is_match(app_id),
            };
            if matched { return true; }
        }
        false
    }

    /// Static version that doesn't need self
    fn should_inhibit_static(app_id: &str, inhibit_apps: &[crate::config::model::AppInhibitPattern]) -> bool {
        for pattern in inhibit_apps {
            let matched = match pattern {
                crate::config::model::AppInhibitPattern::Literal(s) => Self::app_id_matches_static(s, app_id),
                crate::config::model::AppInhibitPattern::Regex(r) => r.is_match(app_id),
            };
            if matched { return true; }
        }
        false
    }

    fn app_id_matches(&self, pattern: &str, app_id: &str) -> bool {
        Self::app_id_matches_static(pattern, app_id)
    }

    fn app_id_matches_static(pattern: &str, app_id: &str) -> bool {
        if pattern.eq_ignore_ascii_case(app_id) { return true; }
        if app_id.ends_with(".exe") {
            let name = app_id.strip_suffix(".exe").unwrap_or(app_id);
            if pattern.eq_ignore_ascii_case(name) { return true; }
        }
        if let Some(last) = pattern.split('.').last() {
            if last.eq_ignore_ascii_case(app_id) { return true; }
        }
        false
    }
}

/// Spawns the app inhibitor background task and returns its JoinHandle
pub async fn spawn_app_inhibit_task(
    manager: Arc<Mutex<Manager>>,
) -> (Arc<Mutex<AppInhibitor>>, JoinHandle<()>) {
    let has_apps = {
        let mgr = manager.lock().await;
        !mgr.state.inhibitors.inhibit_apps.is_empty()
    };

    let inhibitor = Arc::new(Mutex::new(AppInhibitor::new(Arc::clone(&manager))));

    if !has_apps {
        sinfo!("Stasis", "No inhibit_apps configured, sleeping app inhibitor.");
        let inhibitor_clone = Arc::clone(&inhibitor);
        let handle = tokio::spawn(async move {
            let inhibitor_guard = inhibitor_clone.lock().await;
            let manager_guard = inhibitor_guard.manager.lock().await;
            let shutdown = manager_guard.state.shutdown_flag.clone();

            shutdown.notified().await;
            sinfo!("Stasis", "App inhibitor shutting down...");
        });
        return (inhibitor, handle);
    }

    let inhibitor_clone = Arc::clone(&inhibitor);

    let handle = tokio::spawn(async move {
        loop {
            let shutdown = {
                let guard = inhibitor_clone.lock().await;
                guard.manager.lock().await.state.shutdown_flag.clone()
            };

            tokio::select! {
                _ = shutdown.notified() => {
                    sinfo!("Stasis", "App inhibitor shutting down...");
                    break;
                }
                
                _ = tokio::time::sleep(std::time::Duration::from_secs(4)) => {
                    let running = {
                        let mut guard = inhibitor_clone.lock().await;
                        guard.is_any_app_running().await
                    };

                    let mut guard = inhibitor_clone.lock().await;
                    {
                        let mut mgr = guard.manager.lock().await;

                        if running && !guard.inhibitor_active {
                            incr_active_inhibitor(&mut mgr, InhibitorSource::App).await;
                        } else if (!running && guard.inhibitor_active) || (mgr.state.inhibitors.inhibit_apps.is_empty() && guard.inhibitor_active) {
                            decr_active_inhibitor(&mut mgr, InhibitorSource::App).await;
                        }
                    } // <- mgr lock is dropped here

                    // Now it's safe to mutate guard.inhibitor_active
                    if running {
                        guard.inhibitor_active = true;
                    } else {
                        guard.inhibitor_active = false;
                    }
                }

            }
        }
    });

    (inhibitor, handle)
}
