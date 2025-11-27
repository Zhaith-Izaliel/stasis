use std::{process::Command, sync::Arc};
use eyre::Result;
use futures_util::stream::StreamExt;
use mpris::{PlayerFinder, PlaybackStatus};
use tokio::task;
use zbus::{Connection, MatchRule, MessageStream};

use crate::core::manager::{helpers::{decr_active_inhibitor, incr_active_inhibitor}, Manager};

// Players that are always considered local (browsers, local video players)
// For these, we trust MPRIS even without audio output (handles muted tabs)
const ALWAYS_LOCAL_PLAYERS: &[&str] = &[
    "firefox",
    "chrome",
    "chromium",
    "brave",
    "opera",
    "vivaldi",
    "edge",
    "safari",
    "mpv",
    "vlc",
    "totem",
    "celluloid",
];

pub async fn spawn_media_monitor_dbus(manager: Arc<tokio::sync::Mutex<Manager>>) -> Result<()> {
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

            let playing = check_media_playing(ignore_remote_media, &media_blacklist);
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
                let (ignore_remote_media, media_blacklist) = {
                    let mgr = manager.lock().await;
                    let ignore = mgr.state.cfg.as_ref().map(|c| c.ignore_remote_media).unwrap_or(false);
                    let blacklist = mgr.state.cfg.as_ref().map(|c| c.media_blacklist.clone()).unwrap_or_default();
                    (ignore, blacklist)
                };

                let any_playing = check_media_playing(ignore_remote_media, &media_blacklist);

                let mut mgr = manager.lock().await;
                if any_playing && !mgr.state.media_playing {
                    incr_active_inhibitor(&mut mgr).await;
                    mgr.state.media_playing = true;
                    mgr.state.media_blocking = true;
                } else if !any_playing && mgr.state.media_playing {
                    decr_active_inhibitor(&mut mgr).await;
                    mgr.state.media_playing = false;
                    mgr.state.media_blocking = false;
                }
            }
        }
    });
    Ok(())
}

pub fn check_media_playing(ignore_remote_media: bool, media_blacklist: &[String]) -> bool {
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

    // Check each player
    for player in playing_players {
        let identity = player.identity().to_lowercase();
        let bus_name = player.bus_name().to_string().to_lowercase();
        let combined = format!("{} {}", identity, bus_name);
        
        // Check user's custom blacklist (always applies)
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
            // Browsers/video players are always considered local
            // This handles muted YouTube tabs - they're still playing locally
            return true;
        }
        
        // For other players (Spotify, music players, etc.):
        // - If NOT filtering remote: accept immediately
        // - If filtering remote: verify with audio check
        if ignore_remote_media {
            // Check if there's actual audio output
            // This distinguishes:
            //   - Local playback (Spotify on desktop) = HAS sink-inputs
            //   - Remote playback (Spotify Connect, KDE Connect) = NO sink-inputs
            if has_any_audio_output() {
                return true;
            }
            // No audio output = remote playback, continue checking other players
        } else {
            // Not filtering remote media, accept this player
            return true;
        }
    }
    
    false
}

fn has_any_audio_output() -> bool {
    // Small delay to allow audio state to settle after MPRIS state change
    std::thread::sleep(std::time::Duration::from_millis(300));
    
    // Check if there are ANY active sink-inputs
    // This is the key to detecting remote vs local:
    //   - Spotify playing locally: Creates sink-input
    //   - Spotify Connect (playing on phone): NO sink-input
    //   - KDE Connect (forwarding phone playback): NO sink-input
    let output = match Command::new("pactl")
        .args(["list", "sink-inputs", "short"])
        .output() {
        Ok(o) => o,
        Err(_) => return false,
    };

    let stdout = String::from_utf8_lossy(&output.stdout);
    
    // If there's any output, something is playing audio locally
    !stdout.trim().is_empty()
}
