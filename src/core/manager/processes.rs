use std::time::Duration;
use std::fmt;
use tokio::process::Command;
use std::process::Stdio;
use eventline::{event_info_scoped, event_debug_scoped};

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

/// Info about a spawned process
#[derive(Debug, Clone)]
pub struct ProcessInfo {
    pub pid: u32,
    pub pgid: u32,
    pub command: String,
    pub expected_process_name: Option<String>,
}

/// Run a shell command silently
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
            return Err(ProcessError::CommandFailed(format!(
                "Command '{}' exited with status {:?}",
                cmd,
                status.code()
            )));
        }
        Ok::<(), ProcessError>(())
    };

    tokio::time::timeout(Duration::from_secs(30), fut)
        .await
        .map_err(|_| ProcessError::Timeout)??;
    Ok(())
}

/// Run a command detached and return process info
pub async fn run_command_detached(command: &str) -> Result<ProcessInfo, ProcessError> {
    if command.trim().is_empty() {
        return Err(ProcessError::EmptyCommand);
    }

    let child = Command::new("sh")
        .arg("-c")
        .arg(command)
        .envs(std::env::vars())
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .process_group(0)
        .spawn()?;

    let pid = child.id().ok_or(ProcessError::PidNotAvailable)?;
    let pgid = get_pgid(pid).await.unwrap_or(pid);
    let expected_name = extract_expected_process_name(command);

    // Clone for macro to satisfy 'static
    let expected_name_for_log = expected_name.clone();

    event_debug_scoped!(
        "Stasis",
        "Spawned process: PID={}, PGID={}, expected_name={:?}",
        pid,
        pgid,
        expected_name_for_log
    ).await;

    Ok(ProcessInfo {
        pid,
        pgid,
        command: command.to_string(),
        expected_process_name: expected_name,
    })
}

/// Extract expected process name
fn extract_expected_process_name(command: &str) -> Option<String> {
    let first_word = command.split_whitespace().next()?;
    Some(
        std::path::Path::new(first_word)
            .file_name()
            .and_then(|s| s.to_str())
            .unwrap_or(first_word)
            .to_string(),
    )
}

/// Get process group ID
async fn get_pgid(pid: u32) -> Option<u32> {
    let stat_path = format!("/proc/{}/stat", pid);
    let contents = tokio::fs::read_to_string(&stat_path).await.ok()?;
    let fields: Vec<&str> = contents.split_whitespace().collect();
    fields.get(4)?.parse().ok()
}

/// Check if process is active
pub async fn is_process_active(info: &ProcessInfo) -> bool {
    let pid_path = format!("/proc/{}", info.pid);
    if std::path::Path::new(&pid_path).exists() {
        return true;
    }

    if let Some(pids) = get_process_group_members(info.pgid).await {
        if !pids.is_empty() {
            let pid = info.pid;
            let pgid = info.pgid;
            event_info_scoped!(
                "Stasis",
                "Original PID {} dead, but process group {} has {} member(s)",
                pid,
                pgid,
                pids.len()
            ).await;
            return true;
        }
    }

    if let Some(ref name) = info.expected_process_name {
        let name_for_log = name.clone();
        if is_process_running(name).await {
            event_info_scoped!(
                "Stasis",
                "Process group empty, but found '{}' by name",
                name_for_log
            ).await;
            return true;
        }
    }

    false
}

/// Get all PIDs in a process group
async fn get_process_group_members(pgid: u32) -> Option<Vec<u32>> {
    let output = Command::new("ps").arg("-eo").arg("pid,pgid").output().await.ok()?;
    if output.stdout.is_empty() {
        return Some(Vec::new());
    }

    let pids: Vec<u32> = String::from_utf8_lossy(&output.stdout)
        .lines()
        .skip(1)
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

/// Check if process is running by name
pub async fn is_process_running(cmd: &str) -> bool {
    if cmd.trim().is_empty() {
        return false;
    }

    let first_word = cmd.split_whitespace().next().unwrap_or("");
    let binary_name = std::path::Path::new(first_word)
        .file_name()
        .and_then(|s| s.to_str())
        .unwrap_or(first_word);

    match Command::new("pgrep").arg("-x").arg(binary_name).output().await {
        Ok(output) => !output.stdout.is_empty(),
        Err(_) => false,
    }
}

/// Kill process group
pub async fn kill_process_group(info: &ProcessInfo) -> Result<(), ProcessError> {
    let pid = info.pid;
    let pgid = info.pgid;
    event_info_scoped!(
        "Stasis",
        "Killing process group {} (original PID: {})",
        pgid,
        pid
    ).await;

    let _ = Command::new("kill")
        .arg("-TERM")
        .arg(format!("-{}", pgid))
        .output()
        .await;

    tokio::time::sleep(Duration::from_millis(500)).await;

    if is_process_active(info).await {
        event_info_scoped!(
            "Stasis",
            "Process group {} still alive, sending SIGKILL",
            pgid
        ).await;
        let _ = Command::new("kill")
            .arg("-KILL")
            .arg(format!("-{}", pgid))
            .output()
            .await;
    }

    Ok(())
}

/// Check logind lock status
pub async fn is_session_locked_logind() -> bool {
    let session_id = std::env::var("XDG_SESSION_ID").unwrap_or_else(|_| "self".into());
    let session_path = format!("/org/freedesktop/login1/session/{}", session_id);

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
            // Convert to owned String
            let stdout_owned = String::from_utf8_lossy(&result.stdout).to_string();
            let trimmed = stdout_owned.trim().to_string();

            // Clone for macro (macro gets its own owned String)
            let trimmed_for_log: String = trimmed.clone();

            event_debug_scoped!(
                "Stasis",
                "logind LockedHint ({}): {}",
                session_id,
                trimmed_for_log
            ).await;

            trimmed == "true"
        }
        Err(e) => {
            let e_owned = e.to_string();
            event_debug_scoped!(
                "Stasis",
                "Failed to query logind LockedHint: {}",
                e_owned
            ).await;
            false
        }
    }
}

