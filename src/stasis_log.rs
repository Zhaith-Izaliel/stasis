// Author: Dustin Pilgrim
// License: MIT

use std::fs;
use std::io::{self, Write};
use std::path::{Path, PathBuf};

const DEFAULT_MAX_BYTES: u64 = 5 * 1024 * 1024; // 5 MiB
const DEFAULT_KEEP_BACKUPS: u32 = 5;

pub struct LogPolicy {
    pub max_bytes: u64,
    pub keep_backups: u32,
}

impl Default for LogPolicy {
    fn default() -> Self {
        Self {
            max_bytes: DEFAULT_MAX_BYTES,
            keep_backups: DEFAULT_KEEP_BACKUPS,
        }
    }
}

/// Ensures the log file exists and rotates it if needed.
/// Returns whether to insert a raw blank line before the next run header.
pub fn prepare_log_file(path: &Path, policy: LogPolicy) -> io::Result<bool> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }

    let meta = match fs::metadata(path) {
        Ok(m) => m,
        Err(e) if e.kind() == io::ErrorKind::NotFound => return Ok(false),
        Err(e) => return Err(e),
    };

    if meta.len() == 0 {
        return Ok(false);
    }

    if meta.len() >= policy.max_bytes {
        rotate(path, policy.keep_backups)?;
        return Ok(false);
    }

    Ok(true)
}

/// Writes a literal blank line, bypassing eventline prefixes.
pub fn write_raw_blank_line(path: &Path) -> io::Result<()> {
    let mut f = fs::OpenOptions::new().create(true).append(true).open(path)?;
    f.write_all(b"\n")?;
    f.flush()?;
    Ok(())
}

pub fn run_header() -> String {
    let pid = std::process::id();
    format!("==================== stasis daemon run start (pid={pid}) ====================")
}

pub fn write_raw_line(path: &Path, line: &str) -> io::Result<()> {
    let mut f = fs::OpenOptions::new().create(true).append(true).open(path)?;
    f.write_all(line.as_bytes())?;
    f.write_all(b"\n")?;
    f.flush()?;
    Ok(())
}

fn rotate(path: &Path, keep_backups: u32) -> io::Result<()> {
    if keep_backups == 0 {
        let _ = fs::remove_file(path);
        return Ok(());
    }

    let base = path.to_path_buf();

    for i in (1..keep_backups).rev() {
        let from = rotated_name(&base, i);
        let to = rotated_name(&base, i + 1);
        if from.exists() {
            let _ = fs::rename(from, to);
        }
    }

    let first = rotated_name(&base, 1);
    let _ = fs::rename(path, first);
    Ok(())
}

fn rotated_name(base: &PathBuf, n: u32) -> PathBuf {
    PathBuf::from(format!("{}.{}", base.display(), n))
}
