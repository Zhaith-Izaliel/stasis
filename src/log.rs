use std::fs::{OpenOptions, create_dir_all, metadata, remove_file};
use std::io::Write;
use std::path::PathBuf;
use chrono::Local;
use once_cell::sync::Lazy;
use std::sync::{Mutex, Once};
use std::fmt::Arguments;

/// Maximum log file size in bytes before rotation (50 MB)
const MAX_LOG_SIZE: u64 = 50 * 1024 * 1024;

#[derive(PartialEq, PartialOrd, Clone, Debug)]
pub enum LogLevel {
    Error = 1,
    Warn  = 2,
    Info  = 3,
    Debug = 4,
}

impl LogLevel {
    /// Get ANSI color code for terminal output
    fn color(&self) -> &'static str {
        match self {
            LogLevel::Error => "\x1b[31m", // Red
            LogLevel::Warn  => "\x1b[33m", // Yellow
            LogLevel::Info  => "\x1b[36m", // Cyan
            LogLevel::Debug => "\x1b[90m", // Gray
        }
    }
}

const RESET_COLOR: &str = "\x1b[0m";

pub struct Config {
    pub level: LogLevel,
    pub use_colors: bool,
}

pub static GLOBAL_CONFIG: Lazy<Mutex<Config>> = Lazy::new(|| {
    Mutex::new(Config {
        level: LogLevel::Info,
        use_colors: atty::is(atty::Stream::Stdout), // Auto-detect if terminal supports colors
    })
});

static SESSION_SEPARATOR: Once = Once::new();

/// Set verbose/debug mode
pub fn set_verbose(enabled: bool) {
    let mut config = GLOBAL_CONFIG.lock().unwrap();
    config.level = if enabled { LogLevel::Debug } else { LogLevel::Info };
}

/// Set the minimum log level
pub fn set_log_level(level: LogLevel) {
    let mut config = GLOBAL_CONFIG.lock().unwrap();
    config.level = level;
}

/// Core logging function
pub fn log_message(level: LogLevel, prefix: &str, args: Arguments) {
    let config = GLOBAL_CONFIG.lock().unwrap();
    
    // Skip message if level is lower than configured
    if level > config.level {
        return;
    }
    
    let timestamp = Local::now().format("%Y-%m-%d %H:%M:%S");
    let level_str = match level {
        LogLevel::Error => "ERR",
        LogLevel::Warn  => "WRN",
        LogLevel::Info  => "INF",
        LogLevel::Debug => "DBG",
    };
    
    // File format (with short level indicator)
    let file_line = format!("[{}][{}][{}] {}", timestamp, level_str, prefix, args);
    
    // Console format (with colored bullet if enabled)
    let console_line = if config.use_colors {
        format!("{}●{} [{}][{}] {}", 
            level.color(),
            RESET_COLOR,
            timestamp,
            prefix,
            args)
    } else {
        file_line.clone()
    };
    
    // Write to log file
    if let Err(e) = write_line_to_log(&file_line) {
        eprintln!("Failed to write log: {}", e);
    }
    
    // Print to console if debug mode or error
    if config.level == LogLevel::Debug || level == LogLevel::Error {
        match level {
            LogLevel::Error => eprintln!("{}", console_line),
            _ => println!("{}", console_line),
        }
    }
}

/// Flexible macro to allow formatted logging
#[macro_export]
macro_rules! slog {
    ($level:expr, $prefix:expr, $($arg:tt)*) => {
        $crate::log::log_message($level, $prefix, format_args!($($arg)*))
    };
}

/// Convenience macros
#[macro_export]
macro_rules! sinfo {
    ($prefix:expr, $($arg:tt)*) => { $crate::slog!($crate::log::LogLevel::Info, $prefix, $($arg)*) };
}

#[macro_export]
macro_rules! swarn {
    ($prefix:expr, $($arg:tt)*) => { $crate::slog!($crate::log::LogLevel::Warn, $prefix, $($arg)*) };
}

#[macro_export]
macro_rules! serror {
    ($prefix:expr, $($arg:tt)*) => { $crate::slog!($crate::log::LogLevel::Error, $prefix, $($arg)*) };
}

#[macro_export]
macro_rules! sdebug {
    ($prefix:expr, $($arg:tt)*) => { $crate::slog!($crate::log::LogLevel::Debug, $prefix, $($arg)*) };
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

/// Rotate log if bigger than MAX_LOG_SIZE
fn rotate_log_if_needed(path: &PathBuf) {
    if let Ok(meta) = metadata(path) {
        if meta.len() >= MAX_LOG_SIZE {
            let _ = remove_file(path);
        }
    }
}

/// Ensure session newline once
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

/// Write a line to the log file
fn write_line_to_log(line: &str) -> std::io::Result<()> {
    let path = log_path();
    rotate_log_if_needed(&path);
    ensure_session_newline_once(&path);
    
    let mut file = OpenOptions::new()
        .create(true)
        .append(true)
        .open(&path)?;
    
    writeln!(file, "{}", line)?;
    Ok(())
}
