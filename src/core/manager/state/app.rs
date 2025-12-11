use std::sync::Arc;
use tokio::sync::Mutex;
use crate::core::services::app_inhibit::AppInhibitor;
use crate::core::manager::{inhibitors::{InhibitorSource, decr_active_inhibitor}};

#[derive(Debug)]
pub struct AppState {
    pub app_inhibitor: Option<Arc<Mutex<AppInhibitor>>>,
}

impl Default for AppState {
    fn default() -> Self {
        Self {
            app_inhibitor: None,
        }
    }
}

impl AppState {
    pub fn attach_inhibitor(&mut self, inhibitor: Arc<Mutex<AppInhibitor>>) {
        self.app_inhibitor = Some(inhibitor);
    }

    pub async fn reset_inhibitor(&mut self) {
        if let Some(inhibitor) = &self.app_inhibitor {
            let mut guard = inhibitor.lock().await;

            if !guard.active_apps.is_empty() {
                let mut mgr = guard.manager.lock().await;
                decr_active_inhibitor(&mut *mgr, InhibitorSource::App).await;
            }

            guard.clear_active_apps();
        }
    }
}
