use std::sync::Arc;
use tokio::sync::Mutex;
use crate::core::services::app_inhibit::AppInhibitor;
use crate::core::manager::inhibitors::{InhibitorSource, decr_active_inhibitor};

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
    
    /// Reset inhibitor state when switching profiles
    /// 
    /// This properly decrements active inhibitors and clears all state
    pub async fn reset_inhibitor(&mut self) {
        if let Some(inhibitor) = &self.app_inhibitor {
            let mut guard = inhibitor.lock().await;
            
            // If inhibitor was active, decrement it
            if guard.inhibitor_active {
                let mut mgr = guard.manager.lock().await;
                decr_active_inhibitor(&mut *mgr, InhibitorSource::App).await;
                drop(mgr); // Release lock before clearing
            }
            
            // Clear all state
            guard.clear_active_apps();
        }
    }
}
