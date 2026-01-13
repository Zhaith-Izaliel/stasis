use std::sync::Arc;
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::UnixListener,
    time::{timeout, Duration},
};
use crate::core::manager::Manager;
use eventline::{event_debug_scoped, event_error_scoped, event_scope_async};
use crate::scopes::Scope;
use super::router;

/// Spawns the IPC socket server that listens for incoming commands
pub async fn spawn_ipc_socket_with_listener(
    manager: Arc<tokio::sync::Mutex<Manager>>,
    listener: UnixListener,
) {
    // Top-level IPC listener scope
    event_scope_async!(Scope::Ipc.to_string(), {
        loop {
            match listener.accept().await {
                Ok((stream, _addr)) => {
                    let manager = Arc::clone(&manager);

                    tokio::spawn(async move {
                        let stream_owned = stream; // move ownership
                        let manager_owned = Arc::clone(&manager);

                        let result = timeout(Duration::from_secs(10), async move {
                            if let Err(err) =
                                handle_connection(stream_owned, manager_owned).await
                            {
                                let err_owned = err.to_string();
                                event_error_scoped!(
                                    "IPC Connection",
                                    "Error handling IPC connection: {}",
                                    err_owned
                                )
                                .await;
                            }
                        })
                        .await;

                        if result.is_err() {
                            event_error_scoped!(
                                "IPC Connection",
                                "IPC connection timed out after 10 seconds"
                            )
                            .await;
                        }
                    });
                }
                Err(e) => {
                    let e_owned = e.to_string();
                    event_error_scoped!(
                        Scope::Ipc.to_string(),
                        "Failed to accept IPC connection: {}",
                        e_owned
                    )
                    .await;
                }
            }
        }
    })
    .await;
}

/// Handles a single IPC connection
async fn handle_connection(
    mut stream: tokio::net::UnixStream, // owned, not borrowed
    manager: Arc<tokio::sync::Mutex<Manager>>,
) -> std::io::Result<()> {
    let mut buf = vec![0u8; 256];
    let n = stream.read(&mut buf).await?;

    if n == 0 {
        return Ok(());
    }

    let cmd_owned = String::from_utf8_lossy(&buf[..n]).trim().to_string();
    let manager_owned = Arc::clone(&manager);

    // Async block is now fully 'static-safe
    event_scope_async!("IPC Command", {
        let manager_for_macro = Arc::clone(&manager_owned);
        let mut stream_for_macro = stream; // move ownership

        // clone cmd specifically for logging
        let cmd_for_log = cmd_owned.clone();

        if !cmd_for_log.contains("--json") {
            event_debug_scoped!(
                "IPC Command",
                "Received IPC command: {}",
                cmd_for_log
            )
            .await;
        }

        // Use the original cmd_owned for routing
        let response = router::route_command(&cmd_owned, manager_for_macro).await;

        let _ = stream_for_macro.write_all(response.as_bytes()).await;
        let _ = stream_for_macro.flush().await;
    })
    .await;

    Ok(())
}
