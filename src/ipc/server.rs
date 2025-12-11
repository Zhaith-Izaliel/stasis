use std::sync::Arc;
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::UnixListener,
    time::{Duration, timeout},
};
use crate::{
    core::manager::Manager,
    sdebug, serror,
};
use super::router::route_command;

/// Spawns the IPC socket server that listens for incoming commands
pub async fn spawn_ipc_socket_with_listener(
    manager: Arc<tokio::sync::Mutex<Manager>>,
    listener: UnixListener,
) {
    tokio::spawn(async move {
        loop {
            match listener.accept().await {
                Ok((mut stream, _addr)) => {
                    let manager = Arc::clone(&manager);
                    
                    tokio::spawn(async move {
                        let result = timeout(Duration::from_secs(10), async {
                            if let Err(e) = handle_connection(&mut stream, manager).await {
                                serror!("Stasis", "Error handling IPC connection: {}", e);
                            }
                        }).await;
                        
                        if result.is_err() {
                            serror!("Stasis", "IPC connection timed out after 10 seconds");
                        }
                        
                        let _ = stream.shutdown().await;
                    });
                }
                Err(e) => serror!("Stasis", "Failed to accept IPC connection: {}", e)
            }
        }
    });
}

/// Handles a single IPC connection
async fn handle_connection(
    stream: &mut tokio::net::UnixStream,
    manager: Arc<tokio::sync::Mutex<Manager>>,
) -> std::io::Result<()> {
    let mut buf = vec![0u8; 256];
    let n = stream.read(&mut buf).await?;
    
    if n == 0 {
        return Ok(());
    }
    
    let cmd = String::from_utf8_lossy(&buf[..n]).trim().to_string();
    
    if !cmd.contains("--json") {
        sdebug!("Stasis", "Received IPC command: {}", cmd);
    }
    
    let response = route_command(&cmd, manager).await;
    
    stream.write_all(response.as_bytes()).await?;
    stream.flush().await?;
    
    Ok(())
}
