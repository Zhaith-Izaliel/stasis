use std::sync::Arc;
use tokio::time::Duration;
use crate::core::manager::Manager;
use crate::config::info::InfoSections;
use super::state_info::{collect_manager_state, format_text_response, format_json_response};

/// Handles the "info" command - displays current state
pub async fn handle_info(
    manager: Arc<tokio::sync::Mutex<Manager>>,
    as_json: bool,
    sections: InfoSections,
) -> String {
    let mut retry_count = 0;
    let max_retries = 5;
    
    loop {
        match manager.try_lock() {
            Ok(mut mgr) => {
                let state = collect_manager_state(&mut mgr);
                drop(mgr);
                
                return if as_json {
                    format_json_response(&state)
                } else {
                    format_text_response(&state, sections)
                };
            }
            Err(_) => {
                retry_count += 1;
                if retry_count >= max_retries {
                    return if as_json {
                        serde_json::json!({
                            "text": "",
                            "alt": "not_running",
                            "tooltip": "Busy, try again",
                            "profile": null
                        }).to_string()
                    } else {
                        "Manager is busy, try again".to_string()
                    };
                }
                tokio::time::sleep(Duration::from_millis(20)).await;
            }
        }
    }
}

