use std::{collections::HashSet, sync::Arc};
use tokio::sync::Mutex;
use tokio::process::Command;
use tokio::task::JoinHandle;
use serde_json::Value;
use procfs::process::all_processes;

use crate::core::manager::inhibitors::{InhibitorSource, decr_active_inhibitor, incr_active_inhibitor};
use crate::core::manager::Manager;
use eventline::{event_info_scoped, event_debug_scoped};

/// Tracks currently running apps to inhibit idle
#[derive(Debug)]
pub struct AppInhibitor {
    pub active_apps: HashSet<String>,
    pub desktop: String,
    pub manager: Arc<Mutex<Manager>>,
    pub inhibitor_active: bool,
}

impl AppInhibitor {
    pub fn new(manager: Arc<Mutex<Manager>>) -> Self {      
        let desktop = std::env::var("XDG_CURRENT_DESKTOP")
            .unwrap_or_default()
            .to_lowercase();

        // log without moving the variable
        let desktop_for_log = desktop.clone();
        event_debug_scoped!("Stasis", "XDG_CURRENT_DESKTOP detected: {}", desktop_for_log);

        Self {
            active_apps: HashSet::new(),
            desktop, // move original here safely
            manager,
            inhibitor_active: false,
        }
    }

    pub fn clear_active_apps(&mut self) {
        self.active_apps.clear();
        self.inhibitor_active = false;
    }

    /// Reset inhibitors properly - decrements if active, then clears state
    pub async fn reset_inhibitors(&mut self) {
        if self.inhibitor_active {
            let manager = Arc::clone(&self.manager);
            tokio::spawn(async move {
                let mut mgr = manager.lock().await;
                decr_active_inhibitor(&mut mgr, InhibitorSource::App).await;
            });
            self.inhibitor_active = false;
        }
        self.active_apps.clear();
    }

    /// Check if any configured apps are running and update active_apps
    pub async fn is_any_app_running(&mut self) -> bool {
        // Get inhibit_apps without holding manager lock for long
        let inhibit_apps = {
            let mgr = self.manager.lock().await;
            if mgr.state.inhibitors.inhibit_apps.is_empty() {
                self.active_apps.clear();
                return false;
            }
            mgr.state.inhibitors.inhibit_apps.clone()
        };

        let mut new_active_apps = HashSet::new();

        // Try compositor check first (faster)
        let running = match self.check_compositor_windows_sync(&inhibit_apps).await {
            Ok(result_apps) => {
                new_active_apps = result_apps;
                !new_active_apps.is_empty()
            },
            Err(_) => {
                // Fallback to process scanning
                self.check_processes_sync(&inhibit_apps, &mut new_active_apps).await
            },
        };

        // Log newly detected apps (iterate over owned clone to avoid borrowing issues)
        for app in new_active_apps.clone() {
            if !self.active_apps.contains(&app) {
                event_info_scoped!("Stasis", "App inhibit active: {}", app);
            }
        }

        self.active_apps = new_active_apps;
        running
    }

    /// Quick check without modifying state (for immediate profile change check)
    pub async fn check_apps_immediate(&self) -> bool {
        let inhibit_apps = {
            let mgr = self.manager.lock().await;
            mgr.state.inhibitors.inhibit_apps.clone()
        };

        if inhibit_apps.is_empty() {
            return false;
        }

        // Try compositor first
        match self.check_compositor_windows_sync(&inhibit_apps).await {
            Ok(apps) => !apps.is_empty(),
            Err(_) => {
                let mut dummy = HashSet::new();
                self.check_processes_sync(&inhibit_apps, &mut dummy).await
            }
        }
    }

    /// Check processes and optionally track them
    async fn check_processes_sync(
        &self,
        inhibit_apps: &[crate::config::model::AppInhibitPattern],
        new_active_apps: &mut HashSet<String>
    ) -> bool {
        let processes_iter = match all_processes() {
            Ok(iter) => iter,
            Err(_) => return false,
        };

        let mut any_running = false;

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
                    new_active_apps.insert(proc_name.clone());
                    any_running = true;
                    break;
                }
            }
        }

        any_running
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

/// Spawns the app inhibitor background task
pub async fn spawn_app_inhibit_task(
    manager: Arc<Mutex<Manager>>,
) -> (Arc<Mutex<AppInhibitor>>, JoinHandle<()>) {
    let has_apps = {
        let mgr = manager.lock().await;
        !mgr.state.inhibitors.inhibit_apps.is_empty()
    };

    let inhibitor = Arc::new(Mutex::new(AppInhibitor::new(Arc::clone(&manager))));

    if !has_apps {
        event_info_scoped!("Stasis", "No inhibit_apps configured, sleeping app inhibitor.");
        let inhibitor_clone = Arc::clone(&inhibitor);
        let handle = tokio::spawn(async move {
            let inhibitor_guard = inhibitor_clone.lock().await;
            let manager_guard = inhibitor_guard.manager.lock().await;
            let shutdown = manager_guard.state.shutdown_flag.clone();

            shutdown.notified().await;
            event_info_scoped!("Stasis", "App inhibitor shutting down...");
        });
        return (inhibitor, handle);
    }

    let inhibitor_clone = Arc::clone(&inhibitor);

    let handle = tokio::spawn(async move {
        // Use faster polling interval for quicker response to profile changes
        let check_interval = std::time::Duration::from_secs(2);
        
        loop {
            let shutdown = {
                let guard = inhibitor_clone.lock().await;
                guard.manager.lock().await.state.shutdown_flag.clone()
            };

            tokio::select! {
                _ = shutdown.notified() => {
                    event_info_scoped!("Stasis", "App inhibitor shutting down...");
                    break;
                }
                
                _ = tokio::time::sleep(check_interval) => {
                    let running = {
                        let mut guard = inhibitor_clone.lock().await;
                        guard.is_any_app_running().await
                    };

                    let mut guard = inhibitor_clone.lock().await;
                    let should_update = {
                        let mgr = guard.manager.lock().await;
                        
                        // Check if we need to update inhibitor state
                        (running && !guard.inhibitor_active) || 
                        (!running && guard.inhibitor_active) || 
                        (mgr.state.inhibitors.inhibit_apps.is_empty() && guard.inhibitor_active)
                    };
                    
                    if should_update {
                        let mut mgr = guard.manager.lock().await;

                        if running && !guard.inhibitor_active {
                            incr_active_inhibitor(&mut mgr, InhibitorSource::App).await;
                            drop(mgr);
                            guard.inhibitor_active = true;
                        } else if (!running && guard.inhibitor_active) || 
                                  (mgr.state.inhibitors.inhibit_apps.is_empty() && guard.inhibitor_active) {
                            decr_active_inhibitor(&mut mgr, InhibitorSource::App).await;
                            drop(mgr);
                            guard.inhibitor_active = false;
                        }
                    }
                }
            }
        }
    });

    (inhibitor, handle)
}
