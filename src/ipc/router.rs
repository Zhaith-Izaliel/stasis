use std::sync::Arc;
use crate::{
    core::manager::Manager,
    config::info,
    daemon::ShutdownSender,
};
use super::handlers::{
    config, control, info as infoHandler, pause_resume, profile, trigger,
};
use eventline::{event_warn, event_error, scoped_eventline};

/// Routes incoming commands to appropriate handlers
pub async fn route_command(
    cmd: &str,
    manager: Arc<tokio::sync::Mutex<Manager>>,
    shutdown_tx: ShutdownSender,
) -> String {
    let cmd_owned = cmd.to_string();

    // Special-case: info --json must NOT emit eventline output
    if cmd_owned.starts_with("info") && cmd_owned.contains("--json") {
        let args = cmd_owned
            .strip_prefix("info")
            .unwrap_or("")
            .trim()
            .to_string();

        let section_arg = args
            .split_whitespace()
            .find(|s| !s.starts_with("--"))
            .unwrap_or("")
            .to_string();

        let mut sections = info::InfoSections::default();
        if !section_arg.is_empty() {
            sections = info::InfoSections {
                status: false,
                config: false,
                actions: false,
            };
            for part in section_arg.split(',') {
                match part.trim() {
                    "status" | "s" => sections.status = true,
                    "config" | "c" => sections.config = true,
                    "actions" | "a" => sections.actions = true,
                    _ => {}
                }
            }
        }

        return infoHandler::handle_info(manager, true, sections).await;
    }

    // All other commands go through Eventline
    scoped_eventline!("Router", {
        let result: Result<String, String> = match cmd_owned.as_str() {
            // Config
            "reload" => {
                scoped_eventline!("Config", {
                    Ok(config::handle_reload(manager.clone()).await)
                })
            }

            // Pause/Resume
            cmd if cmd.starts_with("pause") => {
                let args = cmd.strip_prefix("pause").unwrap_or("").trim().to_string();
                scoped_eventline!("PauseResume", {
                    Ok(pause_resume::handle_pause(manager.clone(), &args).await)
                })
            }
            "resume" => {
                scoped_eventline!("PauseResume", {
                    Ok(pause_resume::handle_resume(manager.clone()).await)
                })
            }

            // List
            cmd if cmd.starts_with("list") => {
                let args = cmd.strip_prefix("list").unwrap_or("").trim().to_string();
                scoped_eventline!("List", {
                    super::handlers::list::handle_list_command(manager.clone(), &args)
                        .await
                        .map_err(|e| {
                            let e_for_log = e.clone();
                            event_error!("List command failed: {}", e_for_log);
                            e
                        })
                })
            }

            // Trigger
            cmd if cmd.starts_with("trigger ") => {
                let action = cmd.strip_prefix("trigger ").unwrap_or("").trim().to_string();
                scoped_eventline!("Trigger", {
                    Ok(trigger::handle_trigger(manager.clone(), &action).await)
                })
            }

            // Profile
            cmd if cmd.starts_with("profile ") => {
                let profile_name = cmd.strip_prefix("profile ").unwrap_or("").trim().to_string();
                scoped_eventline!("Profile", {
                    Ok(profile::handle_profile(manager.clone(), &profile_name).await)
                })
            }

            // Control
            "stop" => {
                scoped_eventline!("Control", {
                    Ok(control::handle_stop(manager.clone(), shutdown_tx.clone()).await)
                })
            }
            "toggle_inhibit" => {
                scoped_eventline!("Control", {
                    Ok(control::handle_toggle_inhibit(manager.clone()).await)
                })
            }

            // Info (non-JSON)
            cmd if cmd.starts_with("info") => {
                let args = cmd.strip_prefix("info").unwrap_or("").trim().to_string();
                let section_arg = args
                    .split_whitespace()
                    .find(|s| !s.starts_with("--"))
                    .unwrap_or("")
                    .to_string();

                let mut sections = info::InfoSections::default();
                if !section_arg.is_empty() {
                    sections = info::InfoSections {
                        status: false,
                        config: false,
                        actions: false,
                    };
                    for part in section_arg.split(',') {
                        match part.trim() {
                            "status" | "s" => sections.status = true,
                            "config" | "c" => sections.config = true,
                            "actions" | "a" => sections.actions = true,
                            _ => {}
                        }
                    }
                }

                scoped_eventline!("Info", {
                    Ok(infoHandler::handle_info(manager.clone(), false, sections).await)
                })
            }

            // Unknown
            _ => {
                let cmd_for_log = cmd_owned.clone();
                event_warn!("Unknown IPC command: {}", cmd_for_log);
                Err(format!("ERROR: Unknown command '{}'", cmd_owned))
            }
        };

        result.unwrap_or_else(|e| e)
    })
}
