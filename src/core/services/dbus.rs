use std::{collections::HashMap, sync::Arc};
use futures::StreamExt;
use tokio::sync::Mutex;
use zbus::{Connection, fdo::Result as ZbusResult, Proxy, MatchRule};
use zvariant::Value;

use crate::core::events::handlers::{handle_event, Event};
use crate::core::manager::Manager;
use crate::{sinfo, sdebug, swarn, serror};

pub async fn listen_for_suspend_events(idle_manager: Arc<Mutex<Manager>>) -> ZbusResult<()> {
    let connection = Connection::system().await?;
    let proxy = Proxy::new(
        &connection,
        "org.freedesktop.login1",
        "/org/freedesktop/login1",
        "org.freedesktop.login1.Manager"
    ).await?;
    
    let mut stream = proxy.receive_signal("PrepareForSleep").await?;
    sinfo!("D-Bus", "Listening for system suspend events");

    while let Some(signal) = stream.next().await {
        let going_to_sleep: bool = match signal.body().deserialize() {
            Ok(val) => val,
            Err(e) => {
                swarn!("D-Bus", "Failed to parse suspend signal: {e:?}");
                continue;
            }
        };

        let mgr = Arc::clone(&idle_manager);
        if going_to_sleep {
            sinfo!("Power", "System preparing to suspend");
            handle_event(&mgr, Event::Suspend).await;
        } else {
            sinfo!("Power", "System woke from suspend");
            handle_event(&mgr, Event::Wake).await;
        }
    }

    Ok(())
}

pub async fn listen_for_lid_events(idle_manager: Arc<Mutex<Manager>>) -> ZbusResult<()> {
    let connection = Connection::system().await?;
    sinfo!("D-Bus", "Listening for lid open/close events via UPower");

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
                serror!("D-Bus", "Error receiving lid message: {e:?}");
                continue;
            }
        };

        let body = msg.body();
        let (iface, changed, _): (String, HashMap<String, Value>, Vec<String>) = match body.deserialize() {
            Ok(val) => val,
            Err(e) => {
                swarn!("D-Bus", "Failed to parse lid event: {e:?}");
                continue;
            }
        };

        if iface == "org.freedesktop.UPower" {
            if let Some(val) = changed.get("LidIsClosed") {
                match val.downcast_ref::<bool>() {
                    Ok(lid_closed) => {
                        let mgr = Arc::clone(&idle_manager);
                        if lid_closed {
                            sinfo!("Power", "Lid closed");
                            handle_event(&mgr, Event::LidClosed).await;
                        } else {
                            sinfo!("Power", "Lid opened");
                            handle_event(&mgr, Event::LidOpened).await;
                        }
                    }
                    Err(e) => {
                        serror!("D-Bus", "Failed to downcast LidIsClosed value: {e:?}");
                    }
                }
            }
        }
    }

    Ok(())
}

pub async fn listen_for_lock_events(idle_manager: Arc<Mutex<Manager>>) -> ZbusResult<()> {
    let connection = Connection::system().await?;
    sinfo!("D-Bus", "Listening for loginctl Lock/Unlock events");

    let session_path = get_current_session_path(&connection).await?;
    sinfo!("D-Bus", "Monitoring session {}", session_path.as_str());

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
            sinfo!("Session", "Received loginctl Lock");
            handle_event(&lock_mgr, Event::LoginctlLock).await;
        }
    });

    let unlock_task = tokio::spawn(async move {
        while let Some(_sig) = unlock_stream.next().await {
            sinfo!("Session", "Received loginctl Unlock");
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
        sdebug!("D-Bus", "Trying XDG_SESSION_ID: {}", session_id);
        let result: Result<zvariant::OwnedObjectPath, zbus::Error> =
            proxy.call("GetSession", &(session_id.as_str(),)).await;

        match result {
            Ok(path) => {
                sinfo!("Session", "Using session {} from XDG_SESSION_ID", path.as_str());
                return Ok(path);
            }
            Err(e) => {
                swarn!("Session", "XDG_SESSION_ID lookup failed: {e}");
            }
        }
    }

    let uid = unsafe { libc::getuid() };
    sdebug!("Session", "Searching for sessions for UID {}", uid);

    let sessions: Vec<(String, u32, String, String, zvariant::OwnedObjectPath)> =
        proxy.call("ListSessions", &()).await?;

    for (session_id, session_uid, username, seat, path) in &sessions {
        if *session_uid == uid {
            sdebug!(
                "Session",
                "Found session '{}' (user: {}, seat: {})",
                session_id,
                username,
                seat
            );

            if let Ok(sproxy) = Proxy::new(
                connection,
                "org.freedesktop.login1",
                path.clone(),
                "org.freedesktop.login1.Session"
            ).await {
                if let Ok(session_type) = sproxy.get_property::<String>("Type").await {
                    sdebug!("Session", "Session '{}' type: {}", session_id, session_type);

                    if (session_type == "wayland" || session_type == "x11") && seat == "seat0" {
                        sinfo!(
                            "Session",
                            "Selected active graphical session '{}' (type: {})",
                            session_id,
                            session_type
                        );
                        return Ok(path.clone());
                    }
                }
            }
        }
    }

    for (session_id, session_uid, _username, _seat, path) in sessions {
        if session_uid == uid {
            sinfo!("Session", "Using first session '{}' for UID {}", session_id, uid);
            return Ok(path);
        }
    }

    swarn!("Session", "No session found for UID {}, falling back to PID method", uid);

    let pid = std::process::id();
    let result: Result<zvariant::OwnedObjectPath, zbus::Error> =
        proxy.call("GetSessionByPID", &(pid,)).await;

    match result {
        Ok(path) => {
            sinfo!("Session", "Using session {} from PID {}", path.as_str(), pid);
            Ok(path)
        }
        Err(e) => Err(zbus::fdo::Error::Failed(format!(
            "Could not resolve session by XDG_SESSION_ID, UID, or PID: {e}"
        ))),
    }
}

// Combined listener
pub async fn listen_for_power_events(idle_manager: Arc<Mutex<Manager>>) -> ZbusResult<()> {
    let m1 = Arc::clone(&idle_manager);
    let m2 = Arc::clone(&idle_manager);
    let m3 = Arc::clone(&idle_manager);

    let suspend_handle = tokio::spawn(async move {
        if let Err(e) = listen_for_suspend_events(m1).await {
            serror!("Power", "Suspend listener error: {e:?}");
        }
    });

    let lid_handle = tokio::spawn(async move {
        if let Err(e) = listen_for_lid_events(m2).await {
            serror!("Power", "Lid listener error: {e:?}");
        }
    });

    let lock_handle = tokio::spawn(async move {
        if let Err(e) = listen_for_lock_events(m3).await {
            serror!("Power", "Lock listener error: {e:?}");
        }
    });

    let _ = tokio::try_join!(suspend_handle, lid_handle, lock_handle);
    Ok(())
}
