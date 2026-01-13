use std::{collections::HashMap, sync::Arc};
use futures::StreamExt;
use tokio::sync::Mutex;
use zbus::{Connection, fdo::Result as ZbusResult, Proxy, MatchRule};
use zvariant::Value;

use crate::core::events::handlers::{handle_event, Event};
use crate::core::manager::Manager;
use eventline::{event_info_scoped, event_debug_scoped, event_warn_scoped, event_error_scoped};

pub async fn listen_for_suspend_events(idle_manager: Arc<Mutex<Manager>>) -> ZbusResult<()> {
    let connection = Connection::system().await?;
    let proxy = Proxy::new(
        &connection,
        "org.freedesktop.login1",
        "/org/freedesktop/login1",
        "org.freedesktop.login1.Manager"
    ).await?;
    
    let mut stream = proxy.receive_signal("PrepareForSleep").await?;
    event_info_scoped!("D-Bus", "Listening for system suspend events").await;

    while let Some(signal) = stream.next().await {
        let going_to_sleep: bool = match signal.body().deserialize() {
            Ok(val) => val,
            Err(e) => {
                event_warn_scoped!("D-Bus", "Failed to parse suspend signal: {e:?}").await;
                continue;
            }
        };

        let mgr = Arc::clone(&idle_manager);
        if going_to_sleep {
            event_info_scoped!("Power", "System preparing to suspend").await;
            handle_event(&mgr, Event::Suspend).await;
        } else {
            event_info_scoped!("Power", "System woke from suspend").await;
            handle_event(&mgr, Event::Wake).await;
        }
    }

    Ok(())
}

pub async fn listen_for_lid_events(idle_manager: Arc<Mutex<Manager>>) -> ZbusResult<()> {
    let connection = Connection::system().await?;
    event_info_scoped!("D-Bus", "Listening for lid open/close events via UPower").await;

    let rule = MatchRule::builder()
        .msg_type(zbus::message::Type::Signal)
        .interface("org.freedesktop.DBus.Properties")?
        .member("PropertiesChanged")?
        .path("/org/freedesktop/UPower")?
        .build();
    
    let mut stream = zbus::MessageStream::for_match_rule(rule, &connection, None).await?;

    while let Some(msg) = stream.next().await {
        let msg = match msg {
            Ok(m) => m,
            Err(e) => {
                event_error_scoped!("D-Bus", "Error receiving lid message: {e:?}").await;
                continue;
            }
        };

        let body = msg.body();
        let (iface, changed, _): (String, HashMap<String, Value>, Vec<String>) = match body.deserialize() {
            Ok(val) => val,
            Err(e) => {
                event_warn_scoped!("D-Bus", "Failed to parse lid event: {e:?}").await;
                continue;
            }
        };

        if iface == "org.freedesktop.UPower" {
            if let Some(val) = changed.get("LidIsClosed") {
                // downcast returns Result<bool, Error>
                if let Ok(lid_closed) = val.clone().downcast::<bool>() {
                    let mgr = Arc::clone(&idle_manager);
                    if lid_closed {
                        event_info_scoped!("Power", "Lid closed").await;
                        handle_event(&mgr, Event::LidClosed).await;
                    } else {
                        event_info_scoped!("Power", "Lid opened").await;
                        handle_event(&mgr, Event::LidOpened).await;
                    }
                }
            }
        }
    }

    Ok(())
}

pub async fn listen_for_lock_events(idle_manager: Arc<Mutex<Manager>>) -> ZbusResult<()> {
    let connection = Connection::system().await?;
    event_info_scoped!("D-Bus", "Listening for loginctl Lock/Unlock events").await;

    let session_path = get_current_session_path(&connection).await?;
    let session_path_clone = session_path.clone();
    event_info_scoped!("D-Bus", "Monitoring session {}", session_path_clone.as_str()).await;

    let proxy = Proxy::new(
        &connection,
        "org.freedesktop.login1",
        session_path.clone(),
        "org.freedesktop.login1.Session"
    ).await?;

    let mut lock_stream = proxy.receive_signal("Lock").await?;
    let mut unlock_stream = proxy.receive_signal("Unlock").await?;

    let lock_mgr = Arc::clone(&idle_manager);
    let unlock_mgr = Arc::clone(&idle_manager);

    let lock_task = tokio::spawn(async move {
        while let Some(_sig) = lock_stream.next().await {
            event_info_scoped!("Session", "Received loginctl Lock").await;
            handle_event(&lock_mgr, Event::LoginctlLock).await;
        }
    });

    let unlock_task = tokio::spawn(async move {
        while let Some(_sig) = unlock_stream.next().await {
            event_info_scoped!("Session", "Received loginctl Unlock").await;
            handle_event(&unlock_mgr, Event::LoginctlUnlock).await;
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

    if let Ok(session_id) = std::env::var("XDG_SESSION_ID") {
        let session_id_clone = session_id.clone();
        event_debug_scoped!("D-Bus", "Trying XDG_SESSION_ID: {}", session_id_clone).await;
        let result: Result<zvariant::OwnedObjectPath, zbus::Error> =
            proxy.call("GetSession", &(session_id.as_str(),)).await;

        if let Ok(path) = result {
            let path_clone = path.clone();
            event_info_scoped!("Session", "Using session {} from XDG_SESSION_ID", path_clone.as_str()).await;
            return Ok(path);
        } else if let Err(e) = result {
            event_warn_scoped!("Session", "XDG_SESSION_ID lookup failed: {e}").await;
        }
    }

    let uid = unsafe { libc::getuid() };
    event_debug_scoped!("Session", "Searching for sessions for UID {}", uid).await;

    let sessions: Vec<(String, u32, String, String, zvariant::OwnedObjectPath)> =
        proxy.call("ListSessions", &()).await?;

    // clone per-loop for static lifetime
    for (session_id, session_uid, username, seat, path) in sessions.clone() {
        if session_uid == uid {
            let session_id_c = session_id.clone();
            let username_c = username.clone();
            let seat_c = seat.clone();
            let _path_c = path.clone();

            event_debug_scoped!(
                "Session",
                "Found session '{}' (user: {}, seat: {})",
                session_id_c,
                username_c,
                seat_c
            ).await;

            if let Ok(sproxy) = Proxy::new(
                connection,
                "org.freedesktop.login1",
                path.clone(),
                "org.freedesktop.login1.Session"
            ).await {
                if let Ok(session_type) = sproxy.get_property::<String>("Type").await {
                    let stype = session_type.clone();
                    let session_id_c2 = session_id.clone();

                    // clone specifically for debug macro
                    let stype_dbg = stype.clone();
                    event_debug_scoped!(
                        "Session",
                        "Session '{}' type: {}",
                        session_id_c2,
                        stype_dbg
                    ).await;

                    if (session_type == "wayland" || session_type == "x11") && seat == "seat0" {
                        let session_id_c3 = session_id.clone();
                        let stype_c3 = stype.clone();
                        event_info_scoped!(
                            "Session",
                            "Selected active graphical session '{}' (type: {})",
                            session_id_c3,
                            stype_c3
                        ).await;
                        return Ok(path);
                    }
                }
            }
        }
    }

    // fallback: first session for UID
    for (_session_id, session_uid, _username, _seat, path) in sessions {
        if session_uid == uid {
            event_info_scoped!("Session", "Using first session for UID {}", uid).await;
            return Ok(path);
        }
    }

    // fallback PID
    event_warn_scoped!("Session", "No session found for UID {}, falling back to PID method", uid).await;
    let pid = std::process::id();
    let result: Result<zvariant::OwnedObjectPath, zbus::Error> =
        proxy.call("GetSessionByPID", &(pid,)).await;

    if let Ok(path) = result {
        let path_clone = path.clone();
        event_info_scoped!("Session", "Using session {} from PID {}", path_clone.as_str(), pid).await;
        Ok(path)
    } else if let Err(e) = result {
        Err(zbus::fdo::Error::Failed(format!(
            "Could not resolve session by XDG_SESSION_ID, UID, or PID: {e}"
        )))
    } else {
        unreachable!()
    }
}

// Combined listener
pub async fn listen_for_power_events(idle_manager: Arc<Mutex<Manager>>) -> ZbusResult<()> {
    let m1 = Arc::clone(&idle_manager);
    let m2 = Arc::clone(&idle_manager);
    let m3 = Arc::clone(&idle_manager);

    let suspend_handle = tokio::spawn(async move {
        if let Err(e) = listen_for_suspend_events(m1).await {
            event_error_scoped!("Power", "Suspend listener error: {e:?}").await;
        }
    });

    let lid_handle = tokio::spawn(async move {
        if let Err(e) = listen_for_lid_events(m2).await {
            event_error_scoped!("Power", "Lid listener error: {e:?}").await;
        }
    });

    let lock_handle = tokio::spawn(async move {
        if let Err(e) = listen_for_lock_events(m3).await {
            event_error_scoped!("Power", "Lock listener error: {e:?}").await;
        }
    });

    let _ = tokio::try_join!(suspend_handle, lid_handle, lock_handle);
    Ok(())
}
