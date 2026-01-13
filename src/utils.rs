use std::{fs::{self, OpenOptions}, io::{Read, Write}, path::PathBuf};
use eventline::runtime::log_level::{LogLevel, set_log_level};
use eventline::runtime;
use eventline::{event_info, event_debug};

/// Marks a new run in the live log file.
/// Inserts a newline before the marker only if the file already has content.
pub fn mark_new_run(log_path: &PathBuf, name: &str) {
    // Check if file already has content
    let mut need_newline = false;
    if let Ok(mut file) = OpenOptions::new().read(true).open(log_path) {
        let mut buf = [0u8; 1];
        if file.read(&mut buf).unwrap_or(0) > 0 {
            need_newline = true;
        }
    }

    // Append marker
    if let Ok(mut file) = OpenOptions::new().create(true).append(true).open(log_path) {
        if need_newline {
            let _ = writeln!(file);
        }
        let _ = writeln!(file, "==== NEW RUN: {} ====", name);
    }
}


/// Returns the path for the Eventline journal
pub fn get_log_path() -> PathBuf {
    dirs::cache_dir().unwrap_or_else(|| PathBuf::from("/tmp"))
        .join("stasis")
        .join("eventline.log")
}

/// Ensure the log directory exists
pub fn ensure_log_dir(path: &PathBuf) -> std::io::Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    Ok(())
}

/// Initialize runtime and logging (synchronous)
pub async fn init_logging(verbose: bool) {
    if !runtime::is_initialized().await {
        runtime::init().await;
    }

    runtime::enable_console_output(verbose);
    runtime::enable_console_color(verbose);

    let log_path = get_log_path();
    if let Err(e) = ensure_log_dir(&log_path) {
        eprintln!("Failed to create log dir: {}", e);
    } else {
        runtime::enable_live_logging(log_path.clone());
    }

    if verbose {
        set_log_level(LogLevel::Debug);
    } else {
        set_log_level(LogLevel::Info);
    }

    event_info!("Stasis starting...").await;
}

/// Flush journal to disk (full Eventline-style)
pub async fn save_journal() {
    if !runtime::is_initialized().await {
        eprintln!("Runtime not initialized, skipping journal save");
        return;
    }

    event_debug!("Journal saved (live log writes immediately)").await;
}
