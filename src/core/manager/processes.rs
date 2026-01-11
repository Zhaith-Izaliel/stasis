use std::time::Duration;
use std::fmt;
use tokio::process::Command;
use std::process::Stdio;

use crate::{sdebug, sinfo};

#[derive(Debug)]
pub enum ProcessError {
    EmptyCommand,
    CommandFailed(String),
    Timeout,
    SpawnFailed(std::io::Error),
    PidNotAvailable,
}

impl fmt::Display for ProcessError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ProcessError::EmptyCommand => write!(f, "Empty command"),
            ProcessError::CommandFailed(msg) => write!(f, "Command failed: {}", msg),
            ProcessError::Timeout => write!(f, "Command timed out"),
            ProcessError::SpawnFailed(e) => write!(f, "Failed to spawn process: {}", e),
            ProcessError::PidNotAvailable => write!(f, "Failed to get child PID"),
        }
    }
}

impl std::error::Error for ProcessError {}

impl From<std::io::Error> for ProcessError {
    fn from(err: std::io::Error) -> Self {
        ProcessError::SpawnFailed(err)
    }
}

/// Information about a spawned process
#[derive(Debug, Clone)]
pub struct ProcessInfo {
    pub pid: u32,
    pub pgid: u32,
    pub command: String,
    pub expected_process_name: Option<String>,
}

/// Run a shell command silently (log to /tmp/stasis.log)
pub async fn run_command_silent(cmd: &str) -> Result<(), ProcessError> {
    let log_file = "/tmp/stasis.log";
    let fut = async {
        let mut child = Command::new("sh")
            .arg("-c")
            .arg(format!("{cmd} >> {log_file} 2>&1"))
            .envs(std::env::vars())
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()?;
        let status = child.wait().await?;
        if !status.success() {
            return Err(ProcessError::CommandFailed(
                format!("Command '{}' exited with status {:?}", cmd, status.code())
            ));
        }
        Ok::<(), ProcessError>(())
    };
    
    tokio::time::timeout(Duration::from_secs(30), fut)
        .await
        .map_err(|_| ProcessError::Timeout)??;
    Ok(())
}

/// Run a command detached and return comprehensive process info
pub async fn run_command_detached(command: &str) -> Result<ProcessInfo, ProcessError> {
    if command.trim().is_empty() {
        return Err(ProcessError::EmptyCommand);
    }

    // Create a new process group by using setsid
    let child = Command::new("sh")
        .arg("-c")
        .arg(command)
        .envs(std::env::vars())
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .process_group(0) // Create new process group
        .spawn()?;

    let pid = child.id().ok_or(ProcessError::PidNotAvailable)?;
    
    // Get the process group ID (usually same as PID for process group leader)
    let pgid = get_pgid(pid).await.unwrap_or(pid);
    
    // Extract expected process name from command for later verification
    let expected_name = extract_expected_process_name(command);
   
    sdebug!("Stasis", "Spawned process: PID={}, PGID={}, expected_name={:?}", pid, pgid, expected_name);

    Ok(ProcessInfo {
        pid,
        pgid,
        command: command.to_string(),
        expected_process_name: expected_name,
    })
}

/// Extract the expected process name from a command
fn extract_expected_process_name(command: &str) -> Option<String> {
    let first_word = command.split_whitespace().next()?;
    
    // Get just the binary name (last component of the path)
    let binary_name = std::path::Path::new(first_word)
        .file_name()
        .and_then(|s| s.to_str())?
        .to_string();
    
    Some(binary_name)
}

/// Get process group ID for a PID
async fn get_pgid(pid: u32) -> Option<u32> {
    let stat_path = format!("/proc/{}/stat", pid);
    let contents = tokio::fs::read_to_string(&stat_path).await.ok()?;
    
    // Parse /proc/[pid]/stat - PGID is the 5th field
    let fields: Vec<&str> = contents.split_whitespace().collect();
    if fields.len() > 4 {
        fields[4].parse().ok()
    } else {
        None
    }
}

/// Check if a process or its descendants are still running
pub async fn is_process_active(info: &ProcessInfo) -> bool {
    // Strategy 1: Check if original PID still exists
    if std::path::Path::new(&format!("/proc/{}", info.pid)).exists() {
        return true;
    }
    
    // Strategy 2: Check process group for any surviving members
    if let Some(pids) = get_process_group_members(info.pgid).await {
        if !pids.is_empty() {
            sinfo!("Stasis", "Original PID {} dead, but process group {} has {} member(s)", info.pid, info.pgid, pids.len());
            return true;
        }
    }
    
    // Strategy 3: If we know the expected process name, search for it
    if let Some(ref name) = info.expected_process_name {
        if is_process_running(name).await {
            sinfo!("Stasis", "Process group empty, but found '{}' by name", name);
            return true;
        }
    }
    
    false
}

/// Get all PIDs in a process group
async fn get_process_group_members(pgid: u32) -> Option<Vec<u32>> {
    let output = Command::new("ps")
        .arg("-eo")
        .arg("pid,pgid")
        .output()
        .await
        .ok()?;
    
    if output.stdout.is_empty() {
        return Some(Vec::new());
    }
    
    let pids: Vec<u32> = String::from_utf8_lossy(&output.stdout)
        .lines()
        .skip(1) // Skip header
        .filter_map(|line| {
            let parts: Vec<&str> = line.split_whitespace().collect();
            if parts.len() >= 2 {
                let pid: u32 = parts[0].parse().ok()?;
                let proc_pgid: u32 = parts[1].parse().ok()?;
                if proc_pgid == pgid {
                    Some(pid)
                } else {
                    None
                }
            } else {
                None
            }
        })
        .collect();
    
    Some(pids)
}

/// Check if a process matching `cmd` is running (by name)
pub async fn is_process_running(cmd: &str) -> bool {
    if cmd.trim().is_empty() {
        return false;
    }
    
    // Extract the actual binary name from the command
    let first_word = cmd.split_whitespace().next().unwrap_or("");
    if first_word.is_empty() {
        return false;
    }
    
    // Get just the binary name (last component of the path)
    let binary_name = std::path::Path::new(first_word)
        .file_name()
        .and_then(|s| s.to_str())
        .unwrap_or(first_word);
    
    match Command::new("pgrep").arg("-x").arg(binary_name).output().await {
        Ok(output) => !output.stdout.is_empty(),
        Err(_) => false,
    }
}

/// Forcefully kill a process and its entire process group
pub async fn kill_process_group(info: &ProcessInfo) -> Result<(), ProcessError> {
    sinfo!("Stasis", "Killing process group {} (original PID: {})", info.pgid, info.pid);
    
    // Kill entire process group
    let _ = Command::new("kill")
        .arg("-TERM")
        .arg(format!("-{}", info.pgid)) // Negative PGID kills the group
        .output()
        .await;
    
    // Give processes time to terminate gracefully
    tokio::time::sleep(Duration::from_millis(500)).await;
    
    // Force kill if still alive
    if is_process_active(info).await {
        sinfo!("Stasis", "Process group {} still alive, sending SIGKILL", info.pgid);
        let _ = Command::new("kill")
            .arg("-KILL")
            .arg(format!("-{}", info.pgid))
            .output()
            .await;
    }
    
    Ok(())
}

pub async fn is_session_locked_logind() -> bool {
    // Determine the correct login1 session path
    let session_id = std::env::var("XDG_SESSION_ID").unwrap_or_else(|_| "self".into());
    let session_path = format!("/org/freedesktop/login1/session/{}", session_id);

    // Query LockedHint
    let output = Command::new("busctl")
        .args([
            "get-property",
            "--system",
            "--",
            "org.freedesktop.login1",
            &session_path,
            "org.freedesktop.login1.Session",
            "LockedHint",
        ])
        .output()
        .await;

    match output {
        Ok(result) => {
            let stdout = String::from_utf8_lossy(&result.stdout);
            let trimmed = stdout.trim();
            sdebug!("Stasis", "logind LockHin ({}): {}", session_id, trimmed);

            trimmed.ends_with("true")
        }
        Err(e) => {
            sdebug!("Stasis", "Failed to query logind LockedHint: {}", e);
            false
        }
    }
}
