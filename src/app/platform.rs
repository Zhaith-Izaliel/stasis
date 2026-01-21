// Author: Dustin Pilgrim
// License: MIT

use std::io;
use std::os::unix::net::{UnixListener, UnixStream};
use std::path::PathBuf;
use std::time::Duration;

pub fn default_log_path() -> Option<PathBuf> {
    let home = std::env::var_os("HOME")?;
    Some(PathBuf::from(home).join(".cache").join("stasis").join("stasis.log"))
}

// ---------------- single-instance lock ----------------

fn runtime_dir() -> Result<PathBuf, String> {
    std::env::var_os("XDG_RUNTIME_DIR")
        .map(PathBuf::from)
        .ok_or_else(|| "XDG_RUNTIME_DIR is not set (cannot create instance lock)".to_string())
}

fn lock_path() -> Result<PathBuf, String> {
    Ok(runtime_dir()?.join("stasis").join("stasis.lock"))
}

pub fn acquire_single_instance_lock() -> Result<UnixListener, String> {
    let path = lock_path()?;
    if let Some(parent) = path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }

    match UnixListener::bind(&path) {
        Ok(l) => Ok(l),
        Err(e) if e.kind() == io::ErrorKind::AddrInUse => {
            match UnixStream::connect(&path) {
                Ok(_) => Err(format!(
                    "stasis is already running (another instance holds {})",
                    path.display()
                )),
                Err(_) => {
                    let _ = std::fs::remove_file(&path);
                    UnixListener::bind(&path)
                        .map_err(|e| format!("failed to bind instance lock {}: {e}", path.display()))
                }
            }
        }
        Err(e) => Err(format!("failed to bind instance lock {}: {e}", path.display())),
    }
}

// ---------------- wayland check ----------------

pub fn ensure_wayland_alive() -> Result<(), String> {
    let session_type = std::env::var("XDG_SESSION_TYPE")
        .unwrap_or_else(|_| "<unset>".to_string());

    if session_type != "wayland" {
        return Err(format!("not a wayland session: XDG_SESSION_TYPE={}", session_type));
    }

    let display = std::env::var("WAYLAND_DISPLAY")
        .map_err(|_| "WAYLAND_DISPLAY is not set".to_string())?;

    let sock = runtime_dir()?.join(display);

    UnixStream::connect(&sock)
        .map(|_| ())
        .map_err(|e| format!("failed to connect to wayland socket {}: {e}", sock.display()))
}

fn wayland_socket_path() -> Result<PathBuf, String> {
    let display = std::env::var("WAYLAND_DISPLAY")
        .map_err(|_| "WAYLAND_DISPLAY is not set".to_string())?;
    Ok(runtime_dir()?.join(display))
}

pub fn spawn_wayland_socket_watcher(shutdown_tx: tokio::sync::watch::Sender<bool>) {
    let sock = match wayland_socket_path() {
        Ok(p) => p,
        Err(e) => {
            eventline::warn!("wayland watcher disabled: {e}");
            return;
        }
    };

    tokio::spawn(async move {
        let mut failures: u32 = 0;

        loop {
            tokio::time::sleep(Duration::from_secs(2)).await;

            if *shutdown_tx.borrow() {
                break;
            }

            if UnixStream::connect(&sock).is_err() {
                failures += 1;
                if failures >= 3 {
                    eventline::info!(
                        "wayland socket not connectable ({}); shutting down",
                        sock.display()
                    );
                    let _ = shutdown_tx.send(true);
                    break;
                }
            } else {
                failures = 0;
            }
        }
    });
}
