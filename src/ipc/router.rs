use std::sync::Arc;
use crate::{
    core::manager::Manager,
    config::info,
    daemon::ShutdownSender,
};
use super::handlers::{
    config, control, info as infoHandler, pause_resume, profile, trigger,
};
use eventline::{
    event_warn_scoped, event_error_scoped, event_scope_async,
};

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

    // All other commands go through eventline
    event_scope_async!("Router", {
        let result: Result<String, String> = match cmd_owned.as_str() {
            // Config
            "reload" => {
                event_scope_async!("Config", {
                    Ok(config::handle_reload(manager.clone()).await)
                })
                .await
            }

            // Pause/Resume
            cmd if cmd.starts_with("pause") => {
                let args = cmd.strip_prefix("pause").unwrap_or("").trim().to_string();
                event_scope_async!("PauseResume", {
                    Ok(pause_resume::handle_pause(manager.clone(), &args).await)
                })
                .await
            }
            "resume" => {
                event_scope_async!("PauseResume", {
                    Ok(pause_resume::handle_resume(manager.clone()).await)
                })
                .await
            }

            // List
            cmd if cmd.starts_with("list") => {
                let args = cmd.strip_prefix("list").unwrap_or("").trim().to_string();
                event_scope_async!("List", {
                    super::handlers::list::handle_list_command(manager.clone(), &args)
                        .await
                        .map_err(|e| {
                            let e_for_log = e.clone();
                                event_error_scoped!(
                                    "List",
                                    "List command failed: {}",
                                    e_for_log
                                );
                            e
                        })
                })
                .await
            }

            // Trigger
            cmd if cmd.starts_with("trigger ") => {
                let action = cmd.strip_prefix("trigger ").unwrap_or("").trim().to_string();
                event_scope_async!("Trigger", {
                    Ok(trigger::handle_trigger(manager.clone(), &action).await)
                })
                .await
            }

            // Profile
            cmd if cmd.starts_with("profile ") => {
                let profile = cmd.strip_prefix("profile ").unwrap_or("").trim().to_string();
                event_scope_async!("Profile", {
                    Ok(profile::handle_profile(manager.clone(), &profile).await)
                })
                .await
            }

            // Control
            "stop" => {
                event_scope_async!("Control", {
                    Ok(control::handle_stop(manager.clone(), shutdown_tx.clone()).await)
                })
                .await
            }
            "toggle_inhibit" => {
                event_scope_async!("Control", {
                    Ok(control::handle_toggle_inhibit(manager.clone()).await)
                })
                .await
            }

            // Info (non-JSON only)
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

                event_scope_async!("Info", {
                    Ok(infoHandler::handle_info(manager.clone(), false, sections).await)
                })
                .await
            }

            // Unknown
            _ => {
                let cmd_for_log = cmd_owned.clone();
                event_warn_scoped!(
                    "Router",
                    "Unknown IPC command: {}",
                    cmd_for_log
                );
                Err(format!("ERROR: Unknown command '{}'", cmd_owned))
            }
        };

        result.unwrap_or_else(|e| e)
    })
    .await
}
