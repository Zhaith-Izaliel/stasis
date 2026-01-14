use std::sync::Arc;
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::UnixListener,
    time::{timeout, Duration},
    task::JoinHandle,
};
use crate::{
    core::manager::Manager,
    daemon::ShutdownSender,
};
use eventline::{event_debug_scoped, event_error_scoped};
use crate::scopes::Scope;
use super::router;

/// Spawns the IPC socket server that listens for incoming commands
/// Returns a JoinHandle so the spawned task can be aborted on shutdown
pub async fn spawn_ipc_socket_with_listener(
    manager: Arc<tokio::sync::Mutex<Manager>>,
    listener: UnixListener,
    shutdown_tx: ShutdownSender,
) -> JoinHandle<()> {
    // Spawn the listener loop into a background task and return the handle
    tokio::spawn(async move {
        loop {
            match listener.accept().await {
                Ok((stream, _addr)) => {
                    let manager = Arc::clone(&manager);
                    let shutdown_tx_clone = shutdown_tx.clone();
                    tokio::spawn(async move {
                        let stream_owned = stream; // move ownership
                        let manager_owned = Arc::clone(&manager);
                        let result = timeout(Duration::from_secs(10), async move {
                            if let Err(err) =
                                handle_connection(stream_owned, manager_owned, shutdown_tx_clone).await
                            {
                                let err_owned = err.to_string();
                                event_error_scoped!(
                                    "IPC Connection",
                                    "Error handling IPC connection: {}",
                                    err_owned
                                );
                            }
                        })
                        .await;
                        if result.is_err() {
                            event_error_scoped!(
                                "IPC Connection",
                                "IPC connection timed out after 10 seconds"
                            );
                        }
                    });
                }
                Err(e) => {
                    let e_owned = e.to_string();
                    event_error_scoped!(
                        Scope::Ipc.to_string(),
                        "Failed to accept IPC connection: {}",
                        e_owned
                    );
                }
            }
        }
    })
}

/// Handles a single IPC connection
async fn handle_connection(
    mut stream: tokio::net::UnixStream, // owned
    manager: Arc<tokio::sync::Mutex<Manager>>,
    shutdown_tx: ShutdownSender,
) -> std::io::Result<()> {
    let mut buf = vec![0u8; 256];
    let n = stream.read(&mut buf).await?;
    if n == 0 {
        return Ok(());
    }
    let cmd_owned = String::from_utf8_lossy(&buf[..n]).trim().to_string();
    let manager_owned = Arc::clone(&manager);
    // Detect "silent" commands (JSON info / Waybar polling)
    let is_silent = cmd_owned.starts_with("info") && cmd_owned.contains("--json");
    if is_silent {
        // Directly route without any Eventline scopes or debug logs
        let response = router::route_command(&cmd_owned, manager_owned, shutdown_tx).await;
        let _ = stream.write_all(response.as_bytes()).await;
        let _ = stream.flush().await;
        return Ok(());
    }
    // Normal IPC command, wrap in scope and log
    let cmd_for_log = cmd_owned.clone(); // CLONE it for logging
        let manager_for_macro = Arc::clone(&manager_owned);
        let mut stream_for_macro = stream;
        event_debug_scoped!(
            "IPC Command",
            "Received IPC command: {}",
            cmd_for_log
        );
        let response = router::route_command(&cmd_owned, manager_for_macro, shutdown_tx).await;
        let _ = stream_for_macro.write_all(response.as_bytes()).await;
        let _ = stream_for_macro.flush().await;
    Ok(())
}
