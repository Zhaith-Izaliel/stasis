use std::sync::Arc;
use crate::{
    core::manager::Manager,
    config::info,
    serror,
};
use super::handlers::{
    config, control, info as infoHandler, pause_resume, profile, trigger,
};

/// Routes incoming commands to appropriate handlers
pub async fn route_command(
    cmd: &str,
    manager: Arc<tokio::sync::Mutex<Manager>>,
) -> String {
    match cmd {
        // Config
        "reload" => config::handle_reload(manager).await,
        
        // Pause/Resume
        cmd if cmd.starts_with("pause") => {
            let args = cmd.strip_prefix("pause").unwrap_or("").trim();
            pause_resume::handle_pause(manager, args).await
        }
        "resume" => pause_resume::handle_resume(manager).await,
        
        // List
        cmd if cmd.starts_with("list") => {
            let args = cmd.strip_prefix("list").unwrap_or("").trim();
            super::handlers::list::handle_list_command(manager, args).await
                .unwrap_or_else(|e| format!("ERROR: {}", e))
        }
        
        // Trigger
        cmd if cmd.starts_with("trigger ") => {
            let action = cmd.strip_prefix("trigger ").unwrap_or("").trim();
            trigger::handle_trigger(manager, action).await
        }
        
        // Profile
        cmd if cmd.starts_with("profile ") => {
            let profile = cmd.strip_prefix("profile ").unwrap_or("").trim();
            profile::handle_profile(manager, profile).await
        }
        
        // Control
        "stop" => control::handle_stop(manager).await,
        "toggle_inhibit" => control::handle_toggle_inhibit(manager).await,
              
        // Info - NEW: Handle section arguments
        cmd if cmd.starts_with("info") => {
            let args = cmd.strip_prefix("info").unwrap_or("").trim();
            let as_json = args.contains("--json");
            
            // Parse section argument
            let section_arg = args
                .split_whitespace()
                .find(|s| !s.starts_with("--"))
                .unwrap_or("");
            
            let mut sections = info::InfoSections::default();
            
            if !section_arg.is_empty() {
                // User specified sections, start with all false
                sections = info::InfoSections {
                    status: false,
                    config: false,
                    actions: false,
                };
                
                // Parse comma-separated or individual section names
                for part in section_arg.split(',') {
                    match part.trim() {
                        "status" | "s" => sections.status = true,
                        "config" | "c" => sections.config = true,
                        "actions" | "a" => sections.actions = true,
                        _ => {} // ignore unknown sections
                    }
                }
            }
            
            infoHandler::handle_info(manager, as_json, sections).await
        }
       
        // Unknown
        _ => {
            serror!("Stasis", "Unknown IPC command: {}", cmd);
            format!("ERROR: Unknown command '{}'", cmd)
        }
    }
}
