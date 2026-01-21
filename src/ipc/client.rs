// Author: Dustin Pilgrim
// License: MIT

use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::UnixStream,
    time::{timeout, Duration},
};

pub async fn send_raw(cmd: &str) -> Result<String, String> {
    let path = crate::ipc::socket_path()?;
    
    if !path.exists() {
        return Err("daemon not running".to_string());
    }

    let mut stream = match timeout(Duration::from_secs(2), UnixStream::connect(&path)).await {
        Ok(Ok(s)) => s,
        Ok(Err(e)) => return Err(format!("failed to connect to {}: {e}", path.display())),
        Err(_) => return Err("timeout connecting to daemon".to_string()),
    };

    timeout(Duration::from_secs(2), stream.write_all(cmd.as_bytes()))
        .await
        .map_err(|_| "timeout writing to daemon".to_string())?
        .map_err(|e| format!("write failed: {e}"))?;

    timeout(Duration::from_secs(2), stream.shutdown())
        .await
        .map_err(|_| "timeout finalizing request".to_string())?
        .map_err(|e| format!("shutdown failed: {e}"))?;

    let mut resp = Vec::new();
    timeout(Duration::from_secs(2), stream.read_to_end(&mut resp))
        .await
        .map_err(|_| "timeout reading response".to_string())?
        .map_err(|e| format!("read failed: {e}"))?;

    Ok(String::from_utf8_lossy(&resp).to_string())
}
