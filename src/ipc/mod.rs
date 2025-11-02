pub mod commands;

use std::sync::Arc;
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::UnixListener,
    time::{Duration, timeout},
};

use crate::{
    config, core::{
        manager::{helpers::{get_manual_inhibit, set_manual_inhibit, trigger_all_idle_actions}, Manager}, 
        services::app_inhibit::AppInhibitor,
        utils::format_duration,
    }, 
    ipc::commands::trigger_action_by_name, 
    log::{log_error_message, log_message}, 
    SOCKET_PATH
};

/// Spawn the IPC control socket task using a pre-bound listener.
pub async fn spawn_ipc_socket_with_listener(
    manager: Arc<tokio::sync::Mutex<Manager>>,
    app_inhibitor: Arc<tokio::sync::Mutex<AppInhibitor>>,
    cfg_path: String,
    listener: UnixListener,
) {
    tokio::spawn(async move {
        loop {
            match listener.accept().await {
                Ok((mut stream, _addr)) => {
                    // Clone for each connection
                    let manager = Arc::clone(&manager);
                    let app_inhibitor = Arc::clone(&app_inhibitor);
                    let cfg_path = cfg_path.clone();
                    
                    tokio::spawn(async move {
                        let result = timeout(Duration::from_secs(10), async {
                            let mut buf = vec![0u8; 256];
                            match stream.read(&mut buf).await {
                                Ok(n) if n > 0 => {
                                    let cmd = String::from_utf8_lossy(&buf[..n]).trim().to_string();
                                    if !cmd.contains("--json") {
                                        log_message(&format!("Received IPC command: {}", cmd));
                                    }

                                    let response = match cmd.as_str() {
                                        // === CONFIG ===
                                        "reload" => {
                                            match config::parser::load_config(&cfg_path) {
                                                Ok(new_cfg) => {
                                                    let mut mgr = manager.lock().await;
                                                    mgr.state.update_from_config(&new_cfg).await;
                                                    mgr.trigger_instant_actions().await;

                                                    log_message("Config reloaded successfully");
                                                    "Config reloaded successfully".to_string()
                                                }
                                                Err(e) => {
                                                    log_error_message(&format!("Failed to reload config: {}", e));
                                                    format!("ERROR: Failed to reload config: {e}")
                                                }
                                            }
                                        }

                                        "pause" => {
                                            let mut mgr = manager.lock().await;
                                            mgr.pause(true).await;
                                            "Idle manager paused".to_string()
                                        }

                                        "resume" => {
                                            let mut mgr = manager.lock().await;
                                            mgr.resume(true).await;
                                            "Idle manager resumed".to_string()
                                        }

                                        cmd if cmd.starts_with("trigger ") => {
                                            let step = cmd.strip_prefix("trigger ").unwrap_or("").trim();

                                            if step.is_empty() {
                                                log_error_message("Trigger command missing action name");
                                                "ERROR: No action name provided".to_string()
                                            } else if step == "all" {
                                                let mut mgr = manager.lock().await;
                                                trigger_all_idle_actions(&mut mgr).await;
                                                log_message("Triggered all idle actions");
                                                "All idle actions triggered".to_string()
                                            } else {
                                                match trigger_action_by_name(manager.clone(), step).await {
                                                    Ok(action) => format!("Action '{}' triggered successfully", action),
                                                    Err(e) => format!("ERROR: {e}"),
                                                }
                                            }
                                        }

                                        "stop" => {
                                            log_message("Received stop command — shutting down gracefully");
                                            let manager_clone = Arc::clone(&manager);
                                            tokio::spawn(async move {
                                                let mut mgr = manager_clone.lock().await;
                                                mgr.shutdown().await;
                                                log_message("Manager shutdown complete, exiting process");
                                                let _ = std::fs::remove_file(SOCKET_PATH);
                                                std::process::exit(0);
                                            });
                                            "Stopping Stasis...".to_string()
                                        }

                                        "toggle_inhibit" => {
                                            let mut mgr = manager.lock().await;
                                            let currently_inhibited = get_manual_inhibit(&mut mgr.state);

                                            if currently_inhibited {
                                                set_manual_inhibit(&mut mgr, false).await;
                                                log_message("Manual inhibit disabled (toggle)");
                                            } else {
                                                set_manual_inhibit(&mut mgr, true).await;
                                                log_message("Manual inhibit enabled (toggle)");
                                            }

                                            let response = if currently_inhibited {
                                                serde_json::json!({
                                                    "text": "Idle Active",
                                                    "alt": "idle_active",
                                                    "tooltip": "Idle inhibition cleared"
                                                })
                                            } else {
                                                serde_json::json!({
                                                    "text": "Inhibited",
                                                    "alt": "manually_inhibited",
                                                    "tooltip": "Idle inhibition active"
                                                })
                                            };
                                            
                                            response.to_string()
                                        }

                                        "info" | "info --json" => {
                                            let as_json = cmd.contains("--json");

                                            // Use try_lock with retry for info command to avoid blocking
                                            let mut retry_count = 0;
                                            let max_retries = 5;
                                            
                                            loop {
                                                match manager.try_lock() {
                                                    Ok(mgr) => {
                                                        let idle_time = mgr.state.last_activity.elapsed();
                                                        let uptime = mgr.state.start_time.elapsed();
                                                        let manually_inhibited = mgr.state.manually_paused;
                                                        let paused = mgr.state.paused;
                                                        let cfg_clone = mgr.state.cfg.clone();
                                                        
                                                        // Release manager lock before acquiring app_inhibitor lock
                                                        drop(mgr);
                                                        
                                                        // Try to get app blocking status with timeout
                                                        let app_blocking = match timeout(
                                                            Duration::from_millis(100),
                                                            async {
                                                                let mut inhibitor = app_inhibitor.lock().await;
                                                                inhibitor.is_any_app_running().await
                                                            }
                                                        ).await {
                                                            Ok(result) => result,
                                                            Err(_) => false, // Timeout, assume no blocking
                                                        };
                                                        
                                                        let idle_inhibited = paused || app_blocking || manually_inhibited;

                                                        break if as_json {
                                                            let icon = if manually_inhibited {
                                                                "manually_inhibited"
                                                            } else if idle_inhibited {
                                                                "idle_inhibited"
                                                            } else {
                                                                "idle_active"
                                                            };

                                                            serde_json::json!({
                                                                "text": "",
                                                                "alt": icon,
                                                                "tooltip": format!(
                                                                    "{}\nIdle time: {}\nUptime: {}\nPaused: {}\nManually paused: {}\nApp blocking: {}",
                                                                    if idle_inhibited { "Idle inhibited" } else { "Idle active" },
                                                                    format_duration(idle_time),
                                                                    format_duration(uptime),
                                                                    paused,
                                                                    manually_inhibited,
                                                                    app_blocking
                                                                )
                                                            })
                                                            .to_string()
                                                        } else if let Some(cfg) = &cfg_clone {
                                                            cfg.pretty_print(Some(idle_time), Some(uptime), Some(idle_inhibited), Some(manually_inhibited))
                                                        } else {
                                                            "No configuration loaded".to_string()
                                                        };
                                                    }
                                                    Err(_) => {
                                                        // Lock is held, retry with small delay
                                                        retry_count += 1;
                                                        if retry_count >= max_retries {
                                                            // Give up and return a timeout response
                                                            break if as_json {
                                                                serde_json::json!({
                                                                    "text": "",
                                                                    "alt": "not_running",
                                                                    "tooltip": "Busy, try again"
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

                                        "list_actions" => {
                                            match crate::ipc::commands::list_available_actions(manager.clone()).await.as_slice() {
                                                [] => "No actions available".to_string(),
                                                actions => actions.join(", "),
                                            }
                                        }

                                        _ => {
                                            log_error_message(&format!("Unknown IPC command: {}", cmd));
                                            format!("ERROR: Unknown command '{}'", cmd)
                                        }
                                    };

                                    // Write response and flush
                                    if let Err(e) = stream.write_all(response.as_bytes()).await {
                                        log_error_message(&format!("Failed to write IPC response: {e}"));
                                    } else {
                                        // Flush to ensure data is sent before closing
                                        let _ = stream.flush().await;
                                    }
                                }
                                Ok(_) => {
                                    // Empty read - client disconnected
                                }
                                Err(e) => {
                                    log_error_message(&format!("Failed to read IPC command: {e}"));
                                }
                            }
                        }).await;
                        
                        if result.is_err() {
                            log_error_message("IPC connection timed out after 10 seconds");
                        }
                        
                        // Ensure stream is properly shut down
                        let _ = stream.shutdown().await;
                    });
                }

                Err(e) => log_error_message(&format!("Failed to accept IPC connection: {}", e)),
            }
        }
    });
}
