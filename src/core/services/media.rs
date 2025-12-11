use std::{process::Command, sync::Arc};
use eyre::Result;
use futures_util::stream::StreamExt;
use mpris::{PlayerFinder, PlaybackStatus};
use tokio::task;
use zbus::{Connection, MatchRule, MessageStream};

use crate::{core::manager::{
    Manager, inhibitors::{decr_active_inhibitor, incr_active_inhibitor}
}, sdebug, serror, sinfo};

// Players that are always considered local (browsers, local video players)
// Note: Firefox is intentionally included here but handled specially
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
    // Check if media monitoring is enabled
    let monitor_media = {
        let mgr = manager.lock().await;
        mgr.state.cfg
            .as_ref()
            .map(|c| c.monitor_media)
            .unwrap_or(true)
    };
    
    if !monitor_media {
        sinfo!("MPRIS", "Media monitor disabled in config");
        return Ok(());
    }

    sinfo!("MPRIS", "Starting media monitor");

    task::spawn(async move {
        let conn = match Connection::session().await {
            Ok(c) => c,
            Err(e) => {
                serror!("MPRIS", "Failed to connect to D-Bus: {}", e);
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
            let (ignore_remote_media, media_blacklist, bridge_active) = {
                let mgr = manager.lock().await;
                let ignore = mgr.state.cfg
                    .as_ref()
                    .map(|c| c.ignore_remote_media)
                    .unwrap_or(false);
                let blacklist = mgr.state.cfg
                    .as_ref()
                    .map(|c| c.media_blacklist.clone())
                    .unwrap_or_default();
                (ignore, blacklist, mgr.state.media.media_bridge_active)
            };

            // Always check non-Firefox players, regardless of bridge state
            let playing = check_media_playing(
                ignore_remote_media,
                &media_blacklist,
                bridge_active // skip Firefox if bridge is active
            );
            
            if playing {
                let mut mgr = manager.lock().await;
                if !mgr.state.media.mpris_media_playing {
                    sinfo!("MPRIS", "Initial check: media playing");
                    incr_active_inhibitor(&mut mgr).await;
                    mgr.state.media.mpris_media_playing = true;
                    
                    // Update overall flags
                    mgr.state.media.media_playing = true;
                    mgr.state.media.media_blocking = true;
                }
            }
        }

        // Monitor MPRIS changes
        loop {
            if let Some(_msg) = stream.next().await {
                let (ignore_remote_media, media_blacklist, bridge_active) = {
                    let mgr = manager.lock().await;
                    let ignore = mgr.state.cfg
                        .as_ref()
                        .map(|c| c.ignore_remote_media)
                        .unwrap_or(false);
                    let blacklist = mgr.state.cfg
                        .as_ref()
                        .map(|c| c.media_blacklist.clone())
                        .unwrap_or_default();
                    (ignore, blacklist, mgr.state.media.media_bridge_active)
                };

                // Check non-Firefox players (or all players if bridge inactive)
                let any_playing = check_media_playing(
                    ignore_remote_media,
                    &media_blacklist,
                    bridge_active // skip Firefox if bridge is active
                );
                
                let mut mgr = manager.lock().await;
                
                // Update MPRIS-specific inhibitor
                if any_playing && !mgr.state.media.mpris_media_playing {
                    sdebug!("MPRIS", "Media started");
                    incr_active_inhibitor(&mut mgr).await;
                    mgr.state.media.mpris_media_playing = true;
                } else if !any_playing && mgr.state.media.mpris_media_playing {
                    // Do final verification with playerctl + pactl
                    if !bridge_active && has_playerctl_players() && has_any_media_playing() {
                        continue;
                    }
                   
                    sdebug!("MPRIS", "Media stopped");
                    decr_active_inhibitor(&mut mgr).await;
                    mgr.state.media.mpris_media_playing = false;
                }
                
                // Update overall media state (combines MPRIS + browser)
                update_combined_state(&mut mgr);
            }
        }
    });
    
    Ok(())
}

/// Update the combined media_playing and media_blocking flags based on all sources
fn update_combined_state(mgr: &mut Manager) {
    let combined = mgr.state.media.mpris_media_playing || mgr.state.media.browser_media_playing;
    mgr.state.media.media_playing = combined;
    mgr.state.media.media_blocking = combined;
}

pub fn check_media_playing(
    ignore_remote_media: bool,
    media_blacklist: &[String],
    skip_firefox: bool
) -> bool {
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

    // Fallback for multi-tab Firefox (only if we're NOT skipping Firefox)
    if !skip_firefox && has_playerctl_players() && has_any_media_playing() {
        return true;
    }

    // Check each player
    for player in playing_players {
        let identity = player.identity().to_lowercase();
        
        // Skip Firefox if the bridge is handling it
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
        
        if ignore_remote_media {
            if is_player_local_by_pactl(&identity) {
                return true;
            } else {
                continue;
            }
        } else {
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

fn is_player_local_by_pactl(player_name: &str) -> bool {
    let output = match Command::new("pactl")
        .args(["list", "sink-inputs"])
        .output()
    {
        Ok(o) => o,
        Err(_) => return false,
    };

    let stdout = String::from_utf8_lossy(&output.stdout).to_lowercase();
    let needle = player_name.to_lowercase();

    stdout.contains(&needle)
}
