use std::{process::Command, sync::Arc};
use eyre::Result;
use futures_util::stream::StreamExt;
use mpris::{PlayerFinder, PlaybackStatus};
use tokio::task;
use zbus::{Connection, MatchRule, MessageStream};

use crate::core::manager::{helpers::{decr_active_inhibitor, incr_active_inhibitor}, Manager};

// Players that are always considered local (browsers, local video players)
const ALWAYS_LOCAL_PLAYERS: &[&str] = &[
    "firefox",
    "chrome",
    "chromium",
    "brave",
    "opera",
    "vivaldi",
    "edge",
    "mpv",
    "vlc",
    "totem",
    "celluloid",
];

pub async fn spawn_media_monitor_dbus(manager: Arc<tokio::sync::Mutex<Manager>>) -> Result<()> {
    let skip_firefox = firefox_extension_exists();

    // If Firefox extension exists, spawn the browser media monitor
    if skip_firefox {
        crate::log::log_message("Media Bridge plugin detected, spawning browser media monitor");
        crate::core::services::browser_media::spawn_browser_media_monitor(Arc::clone(&manager)).await;
    } else {
        crate::log::log_message("Firefox MPRIS bridge not found, using standard MPRIS detection");
    }

    let manager_clone = Arc::clone(&manager);
    task::spawn(async move {
        let mut interval = tokio::time::interval(tokio::time::Duration::from_secs(30));
        let mut was_detected = skip_firefox;
        
        loop {
            interval.tick().await;
            let is_detected = firefox_extension_exists();
            
            if is_detected && !was_detected {
                crate::log::log_message("Firefox MPRIS bridge now detected, spawning browser media monitor");
                
                // Reset media state - clear any MPRIS-based media tracking
                {
                    let mut mgr = manager_clone.lock().await;
                    if mgr.state.media_playing {
                        decr_active_inhibitor(&mut mgr).await;
                        mgr.state.media_playing = false;
                        mgr.state.media_blocking = false;
                    }
                }
                
                // Now spawn the browser monitor which will do a fresh check
                crate::core::services::browser_media::spawn_browser_media_monitor(Arc::clone(&manager_clone)).await;
                was_detected = true;
            } else if !is_detected && was_detected {
                crate::log::log_message("Firefox MPRIS bridge lost");
                
                // Reset browser media state
                crate::core::services::browser_media::reset_browser_monitor().await;
                {
                    let mut mgr = manager_clone.lock().await;
                    // Clear browser-specific state
                    let prev_tab_count = mgr.state.browser_playing_tab_count;
                    for _ in 0..prev_tab_count {
                        decr_active_inhibitor(&mut mgr).await;
                    }
                    mgr.state.browser_playing_tab_count = 0;
                    mgr.state.browser_media_playing = false;
                    mgr.state.media_playing = false;
                    mgr.state.media_blocking = false;
                    mgr.state.media_bridge_active = false;
                }
                
                // Re-enable D-Bus MPRIS checking
                crate::log::log_message("Re-enabling standard MPRIS detection");
                was_detected = false;
            }
        }
    });

    task::spawn(async move {
        let conn = match Connection::session().await {
            Ok(c) => c,
            Err(e) => {
                crate::log::log_error_message(&format!("Failed to connect to D-Bus: {}", e));
                return;
            }
        };

        let rule = MatchRule::builder()
            .msg_type(zbus::message::Type::Signal)
            .interface("org.freedesktop.DBus.Properties")
            .unwrap()
            .member("PropertiesChanged")
            .unwrap()
            .path_namespace("/org/mpris/MediaPlayer2")
            .unwrap()
            .build();

        let mut stream = MessageStream::for_match_rule(rule, &conn, None).await.unwrap();

        // Initial check
        {
            let (ignore_remote_media, media_blacklist) = {
                let mgr = manager.lock().await;
                let ignore = mgr.state.cfg.as_ref().map(|c| c.ignore_remote_media).unwrap_or(false);
                let blacklist = mgr.state.cfg.as_ref().map(|c| c.media_blacklist.clone()).unwrap_or_default();
                (ignore, blacklist)
            };

            let playing = check_media_playing(ignore_remote_media, &media_blacklist, skip_firefox);
            if playing {
                let mut mgr = manager.lock().await;
                if !mgr.state.media_playing {
                    incr_active_inhibitor(&mut mgr).await;
                    mgr.state.media_playing = true;
                    mgr.state.media_blocking = true;
                }
            }
        }

        loop {
            if let Some(_msg) = stream.next().await {
                let (ignore_remote_media, media_blacklist, browser_playing) = {
                    let mgr = manager.lock().await;
                    let ignore = mgr.state.cfg.as_ref().map(|c| c.ignore_remote_media).unwrap_or(false);
                    let blacklist = mgr.state.cfg.as_ref().map(|c| c.media_blacklist.clone()).unwrap_or_default();
                    (ignore, blacklist, mgr.state.browser_media_playing)
                };

                // If browser extension says it's playing, trust it completely
                if browser_playing {
                    let mut mgr = manager.lock().await;
                    if !mgr.state.media_playing {
                        incr_active_inhibitor(&mut mgr).await;
                        mgr.state.media_playing = true;
                        mgr.state.media_blocking = true;
                    }
                    continue;
                }

                // Check if extension exists - if it does, skip MPRIS for Firefox
                let skip_ff = firefox_extension_exists();

                // Otherwise check MPRIS for non-browser media
                let any_playing = check_media_playing(ignore_remote_media, &media_blacklist, skip_ff);
                let mut mgr = manager.lock().await;
                if any_playing && !mgr.state.media_playing {
                    incr_active_inhibitor(&mut mgr).await;
                    mgr.state.media_playing = true;
                    mgr.state.media_blocking = true;
                } else if !any_playing && mgr.state.media_playing {
                    // MPRIS says nothing playing, but do final check with playerctl + pactl
                    // This helps with multi-tab Firefox where one tab might still be playing
                    if !skip_ff && has_playerctl_players() && has_any_media_playing() {
                        // Still actually playing, don't stop blocking
                        continue;
                    }
                    decr_active_inhibitor(&mut mgr).await;
                    mgr.state.media_playing = false;
                    mgr.state.media_blocking = false;
                }
            }
        }
    });
    Ok(())
}

// Detect if Firefox extension/script exists
fn firefox_extension_exists() -> bool {
    // Just check for the socket (most reliable)
    let socket_path = std::path::Path::new("/tmp/media_bridge.sock");
    socket_path.exists()
}

pub fn check_media_playing(ignore_remote_media: bool, media_blacklist: &[String], skip_firefox: bool) -> bool {
    // Get all playing MPRIS players
    let playing_players = match PlayerFinder::new() {
        Ok(finder) => match finder.find_all() {
            Ok(players) => {
                players.into_iter().filter(|player| {
                    player.get_playback_status()
                        .map(|s| s == PlaybackStatus::Playing)
                        .unwrap_or(false)
                }).collect::<Vec<_>>()
            },
            Err(_) => Vec::new(),
        },
        Err(_) => Vec::new(),
    };

    if playing_players.is_empty() {
        return false;
    }

    // For fallback MPRIS (no extension): if playerctl shows players AND audio is playing, consider it active
    // This helps with multi-tab Firefox where MPRIS might show "playing" but actual tab could be muted
    if !skip_firefox && has_playerctl_players() && has_any_media_playing() {
        return true;
    }

    // Check each player
    for player in playing_players {
        let identity = player.identity().to_lowercase();
        if skip_firefox && identity.contains("firefox") {
            continue;
        }

        let bus_name = player.bus_name().to_string().to_lowercase();
        let combined = format!("{} {}", identity, bus_name);
        
        // Check user's custom blacklist
        let is_blacklisted = media_blacklist.iter().any(|b| {
            let b_lower = b.to_lowercase();
            combined.contains(&b_lower)
        });
        
        if is_blacklisted {
            continue;
        }
        
        // Check if this is a browser or local video player
        let is_always_local = ALWAYS_LOCAL_PLAYERS.iter().any(|local| {
            combined.contains(local)
        });
        
        if is_always_local {
            return true;
        }
        
        // For non-local players: two-pronged approach
        // First check if any media is actually playing
        if !has_any_media_playing() {
            continue; // No audio detected, skip this player
        }
        
        // Media is playing - now check user preference
        if ignore_remote_media {
            // User wants to ignore remote media
            // Verify audio is actually going to a running sink
            if has_running_sink() {
                return true; // Local audio output confirmed
            }
            // No running sink, so this is likely remote - skip it
            continue;
        } else {
            // User doesn't want to ignore remote media
            // Any playing media counts
            return true;
        }
    }
    
    false
}

fn has_any_media_playing() -> bool {
    std::thread::sleep(std::time::Duration::from_millis(300));
    
    let output = match Command::new("pactl")
        .args(["list", "sink-inputs", "short"])
        .output() {
        Ok(o) => o,
        Err(_) => return false,
    };

    let stdout = String::from_utf8_lossy(&output.stdout);
    !stdout.trim().is_empty()
}

fn has_running_sink() -> bool {
    let output = match Command::new("sh")
        .args(["-c", "pactl list sinks short | grep -i running"])
        .output() {
        Ok(o) => o,
        Err(_) => return false,
    };

    let stdout = String::from_utf8_lossy(&output.stdout);
    !stdout.trim().is_empty()
}

fn has_playerctl_players() -> bool {
    let output = match Command::new("playerctl")
        .args(["-l"])
        .output() {
        Ok(o) => o,
        Err(_) => return false,
    };

    let stdout = String::from_utf8_lossy(&output.stdout);
    !stdout.trim().is_empty()
}
