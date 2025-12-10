pub mod commands;
pub mod list;
pub mod pause;

use std::sync::Arc;
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::UnixListener,
    time::{Duration, timeout},
};

use crate::{
    SOCKET_PATH, config, core::{
        manager::{Manager, helpers::{current_profile, set_manually_paused, trigger_all_idle_actions}}, 
        services::app_inhibit::AppInhibitor,
        utils::format_duration,
    }, ipc::commands::trigger_action_by_name, sdebug, serror, sinfo
};

pub async fn spawn_ipc_socket_with_listener(
    manager: Arc<tokio::sync::Mutex<Manager>>,
    app_inhibitor: Arc<tokio::sync::Mutex<AppInhibitor>>,
    listener: UnixListener,
) {
    tokio::spawn(async move {
        loop {
            match listener.accept().await {
                Ok((mut stream, _addr)) => {
                    let manager = Arc::clone(&manager);
                    let app_inhibitor = Arc::clone(&app_inhibitor);
                    
                    tokio::spawn(async move {
                        let result = timeout(Duration::from_secs(10), async {
                            let mut buf = vec![0u8; 256];
                            match stream.read(&mut buf).await {
                                Ok(n) if n > 0 => {
                                    let cmd = String::from_utf8_lossy(&buf[..n]).trim().to_string();
                                    if !cmd.contains("--json") {
                                        sdebug!("Stasis", "Received IPC command: {}", cmd);
                                    }

                                    let response = match cmd.as_str() {
                                        // === CONFIG ===
                                        "reload" => {
                                            match config::parser::load_combined_config() {
                                                Ok(combined) => {
                                                    let should_cleanup = {
                                                        let mgr = manager.lock().await;
                                                        let old_monitor = mgr.state.cfg
                                                            .as_ref()
                                                            .map(|c| c.monitor_media)
                                                            .unwrap_or(true);
                                                        let new_monitor = combined.base.monitor_media;
                                                        old_monitor && !new_monitor
                                                    };
                                                    
                                                    if should_cleanup {
                                                        let mut mgr = manager.lock().await;
                                                        mgr.cleanup_media_monitoring().await;
                                                        
                                                        if mgr.state.media.media_bridge_active {
                                                            drop(mgr);
                                                            crate::core::services::browser_media::stop_browser_monitor(
                                                                Arc::clone(&manager)
                                                            ).await;
                                                        }
                                                    }
                                                    
                                                    {
                                                        let mut mgr = manager.lock().await;
                                                        mgr.state.update_from_config(&combined.base).await;
                                                        mgr.state.reload_profiles(&combined).await;
                                                    }
                                                    
                                                    let new_monitor_media = combined.base.monitor_media;
                                                    if new_monitor_media {
                                                        sinfo!("Stasis", "Restarting media monitoring after config reload...");
                                                        if let Err(e) = crate::core::services::media::spawn_media_monitor_dbus(
                                                            Arc::clone(&manager)
                                                        ).await {
                                                            serror!("Stasis", "Failed to restart media monitor: {}", e);
                                                        }
                                                        
                                                        tokio::time::sleep(Duration::from_millis(100)).await;
                                                        
                                                        let mut mgr = manager.lock().await;
                                                        mgr.recheck_media().await;
                                                        mgr.trigger_instant_actions().await;
                                                    } else {
                                                        let mut mgr = manager.lock().await;
                                                        mgr.trigger_instant_actions().await;
                                                    }
                                                    
                                                    let (idle_time, uptime, manually_inhibited, paused, media_blocking, 
                                                         media_bridge_active, cfg_clone) = {
                                                        let mgr = manager.lock().await;
                                                        (
                                                            mgr.state.timing.last_activity.elapsed(),
                                                            mgr.state.timing.start_time.elapsed(),
                                                            mgr.state.inhibitors.manually_paused,
                                                            mgr.state.inhibitors.paused,
                                                            mgr.state.media.media_blocking,
                                                            mgr.state.media.media_bridge_active,
                                                            mgr.state.cfg.clone()
                                                        )
                                                    };

                                                    {
                                                        let mut inhibitor = app_inhibitor.lock().await;
                                                        inhibitor.update_from_config(&combined.base).await;
                                                    }
                                                    
                                                    let app_blocking = match timeout(
                                                        Duration::from_millis(100),
                                                        async {
                                                            let mut inhibitor = app_inhibitor.lock().await;
                                                            inhibitor.is_any_app_running().await
                                                        }
                                                    ).await {
                                                        Ok(result) => result,
                                                        Err(_) => false,
                                                    };

                                                    sdebug!("Stasis", "Config reloaded successfully");
                                                    
                                                    if let Some(cfg) = &cfg_clone {
                                                        format!(
                                                            "Config reloaded successfully\n\n{}",
                                                            cfg.pretty_print(
                                                                Some(idle_time),
                                                                Some(uptime),
                                                                Some(paused),
                                                                Some(manually_inhibited),
                                                                Some(app_blocking),
                                                                Some(media_blocking),
                                                                Some(media_bridge_active)
                                                            )
                                                        )
                                                    } else {
                                                        "Config reloaded successfully".to_string()
                                                    }
                                                }
                                                Err(e) => {
                                                    serror!("Stasis", "Failed to reload config: {}", e);
                                                    format!("ERROR: Failed to reload config: {e}")
                                                }
                                            }
                                        }

                                        // === PAUSE/RESUME ===
                                        cmd if cmd.starts_with("pause") => {
                                            let args = cmd.strip_prefix("pause").unwrap_or("").trim();
                                            
                                            if args.eq_ignore_ascii_case("help") 
                                                || args == "-h" 
                                                || args == "--help" {
                                                pause::PAUSE_HELP_MESSAGE.to_string()
                                            } else {
                                                match pause::handle_pause_command(manager.clone(), args).await {
                                                    Ok(msg) => msg,
                                                    Err(e) => format!("ERROR: {}", e),
                                                }
                                            }
                                        }

                                        "resume" => {
                                            let mut mgr = manager.lock().await;
                                            mgr.resume(true).await;
                                            "Idle manager resumed".to_string()
                                        }

                                        // === LIST ===
                                        cmd if cmd.starts_with("list") => {
                                            let args = cmd.strip_prefix("list").unwrap_or("").trim();
                                            match list::handle_list_command(manager.clone(), args).await {
                                                Ok(msg) => msg,
                                                Err(e) => format!("ERROR: {}", e),
                                            }
                                        }

                                        // === TRIGGER ACTIONS ===
                                        cmd if cmd.starts_with("trigger ") => {
                                            let step = cmd.strip_prefix("trigger ").unwrap_or("").trim();

                                            if step.is_empty() {
                                                serror!("Stasis", "Trigger command missing action name");
                                                "ERROR: No action name provided".to_string()
                                            } else if step == "all" {
                                                let mut mgr = manager.lock().await;
                                                trigger_all_idle_actions(&mut mgr).await;
                                                sdebug!("Stasis", "Triggered all idle actions");
                                                "All idle actions triggered".to_string()
                                            } else {
                                                match trigger_action_by_name(manager.clone(), step).await {
                                                    Ok(action) => format!("Action '{}' triggered successfully", action),
                                                    Err(e) => format!("ERROR: {e}"),
                                                }
                                            }
                                        }

                                        // === PROFILES ===
                                        cmd if cmd.starts_with("profile ") => {
                                            let profile_arg = cmd.strip_prefix("profile ").unwrap_or("").trim();
                                            
                                            if profile_arg.is_empty() {
                                                serror!("Stasis", "Profile command missing profile name");
                                                "ERROR: No profile name provided".to_string()
                                            } else {
                                                let profile_name = if profile_arg.eq_ignore_ascii_case("none") {
                                                    None
                                                } else {
                                                    Some(profile_arg)
                                                };
                                                
                                                let mut mgr = manager.lock().await;
                                                match mgr.set_profile(profile_name).await {
                                                    Ok(msg) => {
                                                        sinfo!("Stasis", "Profile switched: {}", profile_name.unwrap_or("base config"));
                                                        mgr.trigger_instant_actions().await;
                                                        msg
                                                    }
                                                    Err(e) => {
                                                        serror!("Stasis", "Failed to set profile: {}", e);
                                                        format!("ERROR: {}", e)
                                                    }
                                                }
                                            }
                                        }

                                        // === CONTROL ===
                                        "stop" => {
                                            sinfo!("Stasis", "Received stop command - Shutting down gracefully");
                                            let manager_clone = Arc::clone(&manager);
                                            tokio::spawn(async move {
                                                let mut mgr = manager_clone.lock().await;
                                                mgr.shutdown().await;
                                                sinfo!("Stasis", "Manager shutdown complete, exiting process");
                                                let _ = std::fs::remove_file(SOCKET_PATH);
                                                std::process::exit(0);
                                            });
                                            "Stopping Stasis...".to_string()
                                        }

                                        "toggle_inhibit" => {
                                            let mut mgr = manager.lock().await;
                                            let currently_inhibited = mgr.state.is_manually_paused();

                                            if currently_inhibited {
                                                set_manually_paused(&mut mgr, false).await;
                                                sdebug!("Stasis", "Manual inhibit disabled (toggle)");
                                            } else {
                                                set_manually_paused(&mut mgr, true).await;
                                                sdebug!("Stasis", "Manual inhibit enabled (toggle)");
                                            }

                                            let response = if currently_inhibited {
                                                serde_json::json!({
                                                    "text": "Active",
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

                                        // === INFO ===
                                        "info" | "info --json" => {
                                            let as_json = cmd.contains("--json");
                                            let mut retry_count = 0;
                                            let max_retries = 5;
                                            
                                            loop {
                                                match manager.try_lock() {
                                                    Ok(mut mgr) => {
                                                        let idle_time = mgr.state.timing.last_activity.elapsed();
                                                        let uptime = mgr.state.timing.start_time.elapsed();
                                                        let manually_inhibited = mgr.state.inhibitors.manually_paused;
                                                        let paused = mgr.state.inhibitors.paused;
                                                        let media_blocking = mgr.state.media.media_blocking;
                                                        let media_bridge_active = mgr.state.media.media_bridge_active;
                                                        let cfg_clone = mgr.state.cfg.clone();
                                                        let current_profile = current_profile(&mut mgr);
                                                        
                                                        drop(mgr);
                                                        
                                                        let app_blocking = match timeout(
                                                            Duration::from_millis(100),
                                                            async {
                                                                let mut inhibitor = app_inhibitor.lock().await;
                                                                inhibitor.is_any_app_running().await
                                                            }
                                                        ).await {
                                                            Ok(result) => result,
                                                            Err(_) => false,
                                                        };
                                                        
                                                        let idle_inhibited = paused || app_blocking || manually_inhibited;

                                                        break if as_json {
                                                            let (text, icon) = if manually_inhibited {
                                                                ("Inhibited", "manually_inhibited")
                                                            } else if idle_inhibited {
                                                                ("Blocked", "idle_inhibited")
                                                            } else {
                                                                ("Active", "idle_active")
                                                            };

                                                            let profile_str = if let Some(p) = current_profile {
                                                                format!("\nProfile: {}", p)
                                                            } else {
                                                                "\nProfile: base config".to_string()
                                                            };

                                                            serde_json::json!({
                                                                "text": text,
                                                                "alt": icon,
                                                                "tooltip": format!(
                                                                    "{}\nIdle time: {}\nUptime: {}\nPaused: {}\nManually paused: {}\nApp blocking: {}\nMedia blocking: {}{}",
                                                                    if idle_inhibited { "Idle inhibited" } else { "Idle active" },
                                                                    format_duration(idle_time),
                                                                    format_duration(uptime),
                                                                    paused,
                                                                    manually_inhibited,
                                                                    app_blocking,
                                                                    media_blocking,
                                                                    profile_str
                                                                )
                                                            })
                                                            .to_string()
                                                        } else if let Some(cfg) = &cfg_clone {
                                                            let mut info = cfg.pretty_print(
                                                                Some(idle_time), 
                                                                Some(uptime), 
                                                                Some(idle_inhibited), 
                                                                Some(manually_inhibited), 
                                                                Some(app_blocking), 
                                                                Some(media_blocking),
                                                                Some(media_bridge_active)
                                                            );
                                                            
                                                            if let Some(p) = current_profile {
                                                                info.push_str(&format!("\n\nActive profile: {}", p));
                                                            } else {
                                                                info.push_str("\n\nActive profile: base config");
                                                            }
                                                            
                                                            info
                                                        } else {
                                                            "No configuration loaded".to_string()
                                                        };
                                                    }
                                                    Err(_) => {
                                                        retry_count += 1;
                                                        if retry_count >= max_retries {
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

                                        _ => {
                                            serror!("Stasis", "Unknown IPC command: {}", cmd);
                                            format!("ERROR: Unknown command '{}'", cmd)
                                        }
                                    };

                                    if let Err(e) = stream.write_all(response.as_bytes()).await {
                                        serror!("Stasis", "Failed to write IPC response: {}", e);
                                    } else {
                                        let _ = stream.flush().await;
                                    }
                                }
                                Ok(_) => {}
                                Err(e) => {
                                    serror!("Stasis", "Failed to read IPC command: {}", e);
                                }
                            }
                        }).await;
                        
                        if result.is_err() {
                            serror!("Stasis", "IPC connection timed out after 10 seconds");
                        }
                        
                        let _ = stream.shutdown().await;
                    });
                }

                Err(e) => serror!("Stasis", "Failed to accept IPC connection: {}", e)
            }
        }
    });
}
