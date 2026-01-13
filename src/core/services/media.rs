use std::{process::Command, sync::Arc, time::Duration, fmt};
use futures_util::stream::StreamExt;
use mpris::{PlayerFinder, PlaybackStatus};
use tokio::{task, time::sleep};
use zbus::{Connection, MatchRule, MessageStream};

use crate::core::manager::{
    Manager, inhibitors::{InhibitorSource, decr_active_inhibitor, incr_active_inhibitor}
};

use eventline::{event_debug_scoped, event_error_scoped, event_info_scoped};

#[derive(Debug)]
pub enum MediaError {
    DbusConnection(String),
    MatchRuleBuild(String),
    StreamCreation(String),
}

impl fmt::Display for MediaError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            MediaError::DbusConnection(msg) => write!(f, "Failed to connect to D-Bus: {}", msg),
            MediaError::MatchRuleBuild(msg) => write!(f, "Failed to build match rule: {}", msg),
            MediaError::StreamCreation(msg) => write!(f, "Failed to create message stream: {}", msg),
        }
    }
}

impl std::error::Error for MediaError {}

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

pub async fn spawn_media_monitor_dbus(manager: Arc<tokio::sync::Mutex<Manager>>) -> Result<(), MediaError> {
    event_info_scoped!("MPRIS", "Starting media monitor").await;

    task::spawn(async move {
        let conn = match Connection::session().await {
            Ok(c) => c,
            Err(e) => {
                event_error_scoped!("MPRIS", "Failed to connect to D-Bus: {}", e).await;
                return;
            }
        };

        let rule = match MatchRule::builder()
            .msg_type(zbus::message::Type::Signal)
            .interface("org.freedesktop.DBus.Properties")
            .and_then(|b| b.member("PropertiesChanged"))
            .and_then(|b| b.path_namespace("/org/mpris/MediaPlayer2"))
            .map(|b| b.build())
        {
            Ok(r) => r,
            Err(e) => {
                event_error_scoped!("MPRIS", "Failed to build match rule: {}", e).await;
                return;
            }
        };

        let mut stream = match MessageStream::for_match_rule(rule, &conn, None).await {
            Ok(s) => s,
            Err(e) => {
                event_error_scoped!("MPRIS", "Failed to create message stream: {}", e).await;
                return;
            }
        };
        
        // Conditional polling - only poll when something is playing
        let mut poll_interval = tokio::time::interval(tokio::time::Duration::from_secs(2));
        let mut should_poll = false; // Start with polling disabled
        
        // Initial check
        {
            let (monitor_enabled, bridge_active) = {
                let mgr = manager.lock().await;
                let enabled = mgr.state.cfg.as_ref().map(|c| c.monitor_media).unwrap_or(true);
                let bridge = mgr.state.media.media_bridge_active;
                (enabled, bridge)
            };
            
            if !monitor_enabled {
                event_debug_scoped!("MPRIS", "Media monitoring disabled by config, skipping initial check").await;
            } else if bridge_active {
                event_debug_scoped!("MPRIS", "Bridge active at startup, MPRIS monitoring paused").await;
            } else {
                let any_playing = check_and_update_media_state(Arc::clone(&manager)).await;
                should_poll = any_playing;
                if any_playing {
                    event_debug_scoped!("MPRIS", "Initial check: media playing, polling enabled").await;
                } else {
                    event_debug_scoped!("MPRIS", "Initial check: no media, polling disabled").await;
                }
            }
        }

        let shutdown = {
            let mgr = manager.lock().await;
            mgr.state.shutdown_flag.clone()
        };

        // Monitor MPRIS changes AND poll pactl when needed
        loop {
            tokio::select! {
                _ = shutdown.notified() => {
                    event_info_scoped!("MPRIS", "Media monitor shutting down...").await;
                    break;
                }
                
                _ = poll_interval.tick(), if should_poll => {
                    let (monitor_enabled, bridge_active) = {
                        let mgr = manager.lock().await;
                        let enabled = mgr.state.cfg.as_ref().map(|c| c.monitor_media).unwrap_or(true);
                        let bridge = mgr.state.media.media_bridge_active;
                        (enabled, bridge)
                    };
                    
                    // Stop polling if bridge is active or monitoring disabled
                    if !monitor_enabled || bridge_active {
                        should_poll = false;
                        if bridge_active {
                            event_debug_scoped!("MPRIS", "Bridge activated, pausing MPRIS monitoring").await;
                        }
                        continue;
                    }
                    
                    // Periodic check - catches cases where MPRIS doesn't fire events
                    event_debug_scoped!("MPRIS", "Periodic pactl check (polling enabled)").await;
                    let any_playing = check_and_update_media_state(Arc::clone(&manager)).await;
                    
                    // Disable polling if nothing is playing anymore
                    if !any_playing {
                        should_poll = false;
                        event_debug_scoped!("MPRIS", "All media stopped/paused, disabling polling").await;
                    }
                }
                
                msg = stream.next() => {
                    if msg.is_none() {
                        break;
                    }
                    
                    let (monitor_enabled, bridge_active) = {
                        let mgr = manager.lock().await;
                        let enabled = mgr.state.cfg.as_ref().map(|c| c.monitor_media).unwrap_or(true);
                        let bridge = mgr.state.media.media_bridge_active;
                        (enabled, bridge)
                    };
                    
                    // Skip events if bridge is active or monitoring disabled
                    if !monitor_enabled {
                        event_debug_scoped!("MPRIS", "Media monitoring disabled, skipping event").await;
                        continue;
                    }
                    
                    if bridge_active {
                        event_debug_scoped!("MPRIS", "Bridge active, skipping MPRIS event").await;
                        continue;
                    }
                    
                    // MPRIS event - check immediately
                    event_debug_scoped!("MPRIS", "MPRIS event detected").await;
                    let any_playing = check_and_update_media_state(Arc::clone(&manager)).await;
                    
                    // Enable polling if something started playing
                    if any_playing && !should_poll {
                        should_poll = true;
                        event_debug_scoped!("MPRIS", "Media started playing, enabling polling").await;
                    } else if !any_playing && should_poll {
                        should_poll = false;
                        event_debug_scoped!("MPRIS", "All media stopped, disabling polling").await;
                    }
                }
            }
        }
    });
    
    Ok(())
}

/// Check media state and update manager accordingly
/// Returns true if any media is playing, false otherwise
async fn check_and_update_media_state(manager: Arc<tokio::sync::Mutex<Manager>>) -> bool {
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

    // If bridge is active, MPRIS should not be checking
    if bridge_active {
        return false;
    }

    // note: now awaiting the async check_media_playing
    let any_playing = check_media_playing(
        ignore_remote_media,
        &media_blacklist,
        bridge_active
    ).await;

    let mut mgr = manager.lock().await;
    
    if any_playing && !mgr.state.media.mpris_media_playing {
        event_debug_scoped!("MPRIS", "Media started").await;
        incr_active_inhibitor(&mut mgr, InhibitorSource::Media).await;
        mgr.state.media.mpris_media_playing = true;
    } else if !any_playing && mgr.state.media.mpris_media_playing {
        event_debug_scoped!("MPRIS", "Media stopped").await;
        decr_active_inhibitor(&mut mgr, InhibitorSource::Media).await;
        mgr.state.media.mpris_media_playing = false;
    }
    
    update_combined_state(&mut mgr);
    
    any_playing
}

/// Update the combined media_playing and media_blocking flags based on all sources
fn update_combined_state(mgr: &mut Manager) {
    let combined = mgr.state.media.mpris_media_playing || mgr.state.media.browser_media_playing;
    mgr.state.media.media_playing = combined;
    mgr.state.media.media_blocking = combined;
}

/// Make this async now
pub async fn check_media_playing(
    ignore_remote_media: bool,
    media_blacklist: &[String],
    skip_firefox: bool
) -> bool {
    // PRIMARY CHECK: pactl for Firefox/browsers (unless bridge is handling it)
    if !skip_firefox {
        if has_playerctl_players() {
            let has_uncorked = has_any_uncorked_audio();
            event_debug_scoped!("MPRIS", "Firefox pactl check: playerctl_players=true, uncorked_audio={}", has_uncorked).await;

            if has_uncorked {
                return true;
            }

            // The transient case — consult MPRIS for browser players as a *fast* fallback
            // Because MPRIS may already report Playing before pactl uncorks the stream.
            event_debug_scoped!("MPRIS", "playerctl players exist but audio corked; checking MPRIS playback status for browser players").await;
            if let Ok(finder) = PlayerFinder::new() {
                if let Ok(players) = finder.find_all() {
                    for player in players.into_iter() {
                        let id = player.identity().to_lowercase();
                        // only check browser/firefox identity here
                        if id.contains("firefox") || id.contains("chrome") || id.contains("chromium") {
                            if player.get_playback_status().map(|s| s == PlaybackStatus::Playing).unwrap_or(false) {
                                tokio::spawn(event_debug_scoped!("MPRIS", "Browser MPRIS reports Playing (transient) for: {}", id));
                                return true;
                            }
                        }
                    }
                }
            }

            // Also retry pactl a few short times before concluding paused (covers very short resume windows)
            for i in 0..3 {
                let backoff_ms = 50 * (i + 1); // 50, 100, 150 ms
                sleep(Duration::from_millis(backoff_ms)).await;
                if has_any_uncorked_audio() {
                    event_debug_scoped!("MPRIS", "pactl detected uncorked audio after {}ms retry", backoff_ms).await;
                    return true;
                }
            }

            // After consulting MPRIS and retrying pactl briefly, treat as paused and continue to check others.
            event_debug_scoped!("MPRIS", "Firefox/browser players appear corked/paused after retries").await;
        }
    }

    // SECONDARY CHECK: MPRIS for non-Firefox players (or if Firefox check was skipped)
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
        tokio::spawn(event_debug_scoped!("MPRIS", "No MPRIS players reporting as playing"));
        return false;
    }

    // Check each MPRIS player (skipping Firefox if we already checked pactl in the "primary" stage)
    for player in playing_players {
        let identity = player.identity().to_lowercase();
        
        // If we did the pactl check and it indicated firefox players exist, we already handled transient case above.
        if !skip_firefox && identity.contains("firefox") {
            tokio::spawn(event_debug_scoped!("MPRIS", "Skipping Firefox MPRIS check (already checked via pactl fallback)"));
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
            tokio::spawn(event_debug_scoped!("MPRIS", "Player blacklisted: {}", identity));
            continue;
        }
        
        // Check if this is a browser or local video player
        let is_always_local = ALWAYS_LOCAL_PLAYERS.iter().any(|local| {
            combined.contains(local)
        });
        
        if is_always_local {
            tokio::spawn(event_debug_scoped!("MPRIS", "Local player detected via MPRIS: {}", identity));
            return true;
        }
        
        if ignore_remote_media {
            if is_player_local_by_pactl(&identity) {
                tokio::spawn(event_debug_scoped!("MPRIS", "Remote media check passed for: {}", identity));
                return true;
            } else {
                tokio::spawn(event_debug_scoped!("MPRIS", "Ignoring remote player: {}", identity));
                continue;
            }
        } else {
            tokio::spawn(event_debug_scoped!("MPRIS", "Player detected via MPRIS: {}", identity));
            return true;
        }
    }
    
    tokio::spawn(event_debug_scoped!("MPRIS", "No valid playing media detected"));
    false
}


/// Check if there's any audio that's actually playing (not corked/paused)
fn has_any_uncorked_audio() -> bool {
    let output = match Command::new("pactl")
        .args(["list", "sink-inputs"])
        .output() {
        Ok(o) => o,
        Err(_) => return false,
    };

    let stdout = String::from_utf8_lossy(&output.stdout);
    
    // Parse pactl output to find any sink input that is NOT corked
    let mut current_sink_has_audio = false;
    
    for line in stdout.lines() {
        let line_trimmed = line.trim();
        
        // New sink input section
        if line_trimmed.starts_with("Sink Input #") {
            current_sink_has_audio = true;
        }
        // Check corked status for current sink
        else if current_sink_has_audio && line_trimmed.starts_with("Corked:") {
            // "Corked: no" means audio is playing
            if line_trimmed.contains("no") {
                return true;
            }
            current_sink_has_audio = false;
        }
    }
    
    false
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
