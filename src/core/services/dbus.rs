use std::{collections::HashMap, sync::Arc};
use futures::StreamExt;
use tokio::sync::Mutex;
use zbus::{Connection, fdo::Result as ZbusResult, Proxy, MatchRule};
use zvariant::Value;
use crate::core::events::handlers::{handle_event, Event};
use crate::core::manager::Manager;
use crate::log::{log_dbus_message, log_error_message, log_warning_message};

pub async fn listen_for_suspend_events(idle_manager: Arc<Mutex<Manager>>) -> ZbusResult<()> {
    let connection = Connection::system().await?;
    let proxy = Proxy::new(
        &connection,
        "org.freedesktop.login1",
        "/org/freedesktop/login1",
        "org.freedesktop.login1.Manager"
    ).await?;
    
    let mut stream = proxy.receive_signal("PrepareForSleep").await?;
    log_dbus_message("Listening for D-Bus suspend events...");
    
    while let Some(signal) = stream.next().await {
        let going_to_sleep: bool = match signal.body().deserialize() {
            Ok(val) => val,
            Err(e) => {
                log_warning_message(&format!("Failed to parse D-Bus suspend signal: {e:?}"));
                continue;
            }
        };
        
        let manager_arc = Arc::clone(&idle_manager);
        if going_to_sleep {
            handle_event(&manager_arc, Event::Suspend).await; 
        } else {
            handle_event(&manager_arc, Event::Wake).await;
        }
    }
    
    Ok(())
}

pub async fn listen_for_lid_events(idle_manager: Arc<Mutex<Manager>>) -> ZbusResult<()> {
    let connection = Connection::system().await?;
    log_dbus_message("Listening for D-Bus lid events via UPower...");
    
    // Create a match rule for PropertiesChanged signals from UPower
    let rule = MatchRule::builder()
        .msg_type(zbus::message::Type::Signal)
        .interface("org.freedesktop.DBus.Properties")?
        .member("PropertiesChanged")?
        .path("/org/freedesktop/UPower")?
        .build();
    
    let mut stream = zbus::MessageStream::for_match_rule(
        rule,
        &connection,
        None,
    ).await?;
    
    while let Some(msg) = stream.next().await {
        let msg = match msg {
            Ok(m) => m,
            Err(e) => {
                log_error_message(&format!("Error receiving message: {e:?}"));
                continue;
            }
        };
        
        // Bind the body to a variable to extend its lifetime
        let body = msg.body();
        let (iface, changed, _): (String, HashMap<String, Value>, Vec<String>) =
            match body.deserialize() {
                Ok(val) => val,
                Err(e) => {
                    log_warning_message(&format!("Failed to parse lid signal: {e:?}"));
                    continue;
                }
            };
        
        if iface == "org.freedesktop.UPower" {
            if let Some(val) = changed.get("LidIsClosed") {
                // Use Result-based pattern matching instead of Option
                match val.downcast_ref::<bool>() {
                    Ok(lid_closed) => {
                        let manager_arc = Arc::clone(&idle_manager);
                        if lid_closed {
                            handle_event(&manager_arc, Event::LidClosed).await;
                        } else {
                            handle_event(&manager_arc, Event::LidOpened).await;
                        }
                    }
                    Err(e) => {
                        log_error_message(&format!("Failed to downcast LidIsClosed value: {e:?}"));
                    }
                }
            }
        }
    }
    
    Ok(())
}

pub async fn listen_for_lock_events(idle_manager: Arc<Mutex<Manager>>) -> ZbusResult<()> {
    let connection = Connection::system().await?;
    log_dbus_message("Listening for D-Bus lock/unlock events...");
    
    // Get the session path for the current session
    let session_path = get_current_session_path(&connection).await?;
    
    log_dbus_message(&format!("Monitoring session: {}", session_path.as_str()));
    
    let proxy = Proxy::new(
        &connection,
        "org.freedesktop.login1",
        session_path.clone(),
        "org.freedesktop.login1.Session"
    ).await?;
    
    let mut lock_stream = proxy.receive_signal("Lock").await?;
    let manager_for_lock = Arc::clone(&idle_manager);
    
    let mut unlock_stream = proxy.receive_signal("Unlock").await?;
    let manager_for_unlock = Arc::clone(&idle_manager);
    
    let lock_task = tokio::spawn(async move {
        while let Some(_signal) = lock_stream.next().await {
            log_dbus_message("Received Lock signal from loginctl");
            handle_event(&manager_for_lock, Event::LoginctlLock).await;
        }
    });
    
    let unlock_task = tokio::spawn(async move {
        while let Some(_signal) = unlock_stream.next().await {
            log_dbus_message("Received Unlock signal from loginctl");
            handle_event(&manager_for_unlock, Event::LoginctlUnlock).await;
        }
    });
    
    let _ = tokio::try_join!(lock_task, unlock_task);
    Ok(())
}

async fn get_current_session_path(connection: &Connection) -> ZbusResult<zvariant::OwnedObjectPath> {
    let proxy = Proxy::new(
        connection,
        "org.freedesktop.login1",
        "/org/freedesktop/login1",
        "org.freedesktop.login1.Manager"
    ).await?;
    
    // Method 1: Try XDG_SESSION_ID environment variable (most reliable for graphical sessions)
    if let Ok(session_id) = std::env::var("XDG_SESSION_ID") {
        log_dbus_message(&format!("Attempting to use XDG_SESSION_ID: {}", session_id));
        let result: Result<zvariant::OwnedObjectPath, zbus::Error> = proxy.call("GetSession", &(session_id.as_str(),)).await;
        match result {
            Ok(path) => {
                log_dbus_message(&format!("Using session from XDG_SESSION_ID: {}", path.as_str()));
                return Ok(path);
            }
            Err(e) => {
                log_warning_message(&format!("XDG_SESSION_ID lookup failed: {}, trying other methods", e));
            }
        }
    }
    
    // Method 2: Find the active graphical session for current UID
    let uid = unsafe { libc::getuid() };
    log_dbus_message(&format!("Looking for sessions with UID: {}", uid));
    
    let sessions: Vec<(String, u32, String, String, zvariant::OwnedObjectPath)> = 
        proxy.call("ListSessions", &()).await?;
    
    // First pass: try to find an active graphical session on seat0
    for (session_id, session_uid, username, seat, path) in &sessions {
        if *session_uid == uid {
            log_dbus_message(&format!(
                "Found session '{}' for user '{}' (UID {}) on seat '{}'",
                session_id, username, session_uid, seat
            ));
            
            // Check if this is a graphical session
            if let Ok(session_proxy) = Proxy::new(
                connection,
                "org.freedesktop.login1",
                path.clone(),
                "org.freedesktop.login1.Session"
            ).await {
                if let Ok(session_type) = session_proxy.get_property::<String>("Type").await {
                    log_dbus_message(&format!("Session '{}' type: {}", session_id, session_type));
                    
                    // Prefer wayland or x11 sessions on seat0
                    if (session_type == "wayland" || session_type == "x11") && seat == "seat0" {
                        log_dbus_message(&format!(
                            "Selected graphical session '{}' (type: {}, seat: {})",
                            session_id, session_type, seat
                        ));
                        return Ok(path.clone());
                    }
                }
            }
        }
    }
    
    // Second pass: just use the first session matching our UID
    for (session_id, session_uid, _username, _seat, path) in sessions {
        if session_uid == uid {
            log_dbus_message(&format!("Using first available session '{}' for UID {}", session_id, uid));
            return Ok(path);
        }
    }
    
    // Method 3: Fallback to PID method (least reliable)
    log_warning_message("No session found by UID, trying PID method");
    let pid = std::process::id();
    let result: Result<zvariant::OwnedObjectPath, zbus::Error> = proxy.call("GetSessionByPID", &(pid,)).await;
    match result {
        Ok(path) => {
            log_dbus_message(&format!("Using session from PID {}: {}", pid, path.as_str()));
            Ok(path)
        }
        Err(e) => {
            Err(zbus::fdo::Error::Failed(format!(
                "Could not find current session (tried XDG_SESSION_ID, UID match, and PID): {}",
                e
            )))
        }
    }
}

// Combined listener that handles suspend, lid, and lock events
pub async fn listen_for_power_events(idle_manager: Arc<Mutex<Manager>>) -> ZbusResult<()> {
    let suspend_manager = Arc::clone(&idle_manager);
    let lid_manager = Arc::clone(&idle_manager);
    let lock_manager = Arc::clone(&idle_manager);
    
    let suspend_handle = tokio::spawn(async move {
        if let Err(e) = listen_for_suspend_events(suspend_manager).await {
            log_dbus_message(&format!("Suspend listener error: {e:?}"));
        }
    });
    
    let lid_handle = tokio::spawn(async move {
        if let Err(e) = listen_for_lid_events(lid_manager).await {
            log_dbus_message(&format!("Lid listener error: {e:?}"));
        }
    });
    
    let lock_handle = tokio::spawn(async move {
        if let Err(e) = listen_for_lock_events(lock_manager).await {
            log_dbus_message(&format!("Lock listener error: {e:?}"));
        }
    });
    
    let _ = tokio::try_join!(suspend_handle, lid_handle, lock_handle);
    Ok(())
}
