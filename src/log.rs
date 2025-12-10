use std::fs::{OpenOptions, create_dir_all, metadata, remove_file};
use std::io::Write;
use std::path::PathBuf;
use chrono::Local;
use once_cell::sync::Lazy;
use std::sync::{Mutex, Once};

/// Maximum log file size in bytes before rotation (50 MB)
const MAX_LOG_SIZE: u64 = 50 * 1024 * 1024;

#[derive(PartialEq, PartialOrd, Clone, Debug)]
pub enum LogLevel {
    Error = 1,
    Warn = 2,
    Info = 3,
    Debug = 4,
}

pub struct Config {
    pub level: LogLevel,
}

pub static GLOBAL_CONFIG: Lazy<Mutex<Config>> = Lazy::new(|| {
    Mutex::new(Config {
        level: LogLevel::Info, // default
    })
});

static SESSION_SEPARATOR: Once = Once::new();

pub fn set_verbose(enabled: bool) {
    let mut config = GLOBAL_CONFIG.lock().unwrap();
    config.level = if enabled { LogLevel::Debug } else { LogLevel::Info };
}

pub fn set_log_level(level: LogLevel) {
    let mut config = GLOBAL_CONFIG.lock().unwrap();
    config.level = level;
}

fn log(level: LogLevel, prefix: &str, message: &str) {
    let config = GLOBAL_CONFIG.lock().unwrap();
    let timestamp = Local::now().format("%Y-%m-%d %H:%M:%S");
    let msg = format!("[{}][{}] {}", timestamp, prefix, message);

    // Write to file based on level:
    // - Always for Error, Warn, Info
    // - Only if verbose/debug for Debug messages
    match level {
        LogLevel::Debug => {
            if config.level == LogLevel::Debug {
                log_to_cache(&msg);
                println!("{}", &msg);
            }
        }
        _ => {
            log_to_cache(&msg);

            // Only print to console if verbose/debug mode is on
            if config.level == LogLevel::Debug {
                if level == LogLevel::Error {
                    eprintln!("{}", &msg);
                } else {
                    println!("{}", &msg);
                }
            }
        }
    }
}


/// Get log file path
pub fn log_path() -> PathBuf {
    let mut path = dirs::cache_dir().unwrap_or_else(|| PathBuf::from("/tmp"));
    path.push("stasis");
    if !path.exists() {
        let _ = create_dir_all(&path);
    }
    path.push("stasis.log");
    path
}

fn rotate_log_if_needed(path: &PathBuf) {
    if let Ok(meta) = metadata(path) {
        if meta.len() >= MAX_LOG_SIZE {
            let _ = remove_file(path);
        }
    }
}

fn ensure_session_newline_once(path: &PathBuf) {
    SESSION_SEPARATOR.call_once(|| {
        if let Ok(meta) = metadata(path) {
            if meta.len() > 0 {
                if let Ok(mut file) = OpenOptions::new().append(true).open(path) {
                    let _ = writeln!(file);
                }
            }
        }
    });
}

/// Write message to log file
pub fn log_to_cache(message: &str) {
    let path = log_path();
    rotate_log_if_needed(&path);
    ensure_session_newline_once(&path);

    let mut file = OpenOptions::new()
        .create(true)
        .append(true)
        .open(&path)
        .unwrap();

    let _ = writeln!(file, "{}", message);
}

/// Public logging helpers
pub fn log_message(message: &str) {
    log(LogLevel::Info, "Stasis", message);
}

pub fn log_dbus_message(message: &str) {
    log(LogLevel::Debug, "D-Bus", message);
}

pub fn log_debug_message(message: &str) {
    log(LogLevel::Debug, "Debug", message);
}

pub fn log_warning_message(message: &str) {
    log(LogLevel::Warn, "Warning", message);
}

pub fn log_error_message(message: &str) {
    log(LogLevel::Error, "Error", message);
}

pub fn log_media_bridge_message(message: &str) {
    log(LogLevel::Debug, "Media", message);
}

pub fn log_wayland_message(message: &str) {
    log(LogLevel::Debug, "Wayland", message);
}
