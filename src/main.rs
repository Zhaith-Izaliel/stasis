pub mod cli;
pub mod client;
pub mod config;
pub mod core;
pub mod daemon;
pub mod ipc;
pub mod media_bridge;
pub mod scopes;
pub mod utils;

use cli::Args;
use scopes::Scope;
use utils::save_journal;

use std::{env::var, fs, process::exit};
use clap::Parser;
use eventline::runtime;
use eventline::runtime::log_level::{set_log_level, LogLevel};
use eventline::{event_info_scoped, event_warn_scoped, event_error_scoped, event_debug_scoped};
use tokio::net::{UnixListener, UnixStream};

pub const SOCKET_PATH: &str = "/tmp/stasis.sock";
#[derive(Debug)]
pub enum AppError {
    ClientCommandFailed,
    SocketBindFailed,
    DaemonFailed,
}
#[tokio::main(flavor = "current_thread")]
async fn main() {
    if let Err(err) = real_main().await {
        event_error_scoped!("AppError", "Fatal error: {:?}", err);
        save_journal().await;
        exit(1);
    }
    
    // Force exit - don't wait for Tokio runtime cleanup
    exit(0);
}
async fn real_main() -> Result<(), AppError> {
    let args = Args::parse();
    let verbose = args.verbose;
    let command_opt = args.command.clone();
    let is_client = command_opt.is_some();
    // --- Only initialize logging for daemon ---
    if !is_client {
        runtime::init().await;
        runtime::enable_console_output(true);
        runtime::enable_console_color(true);
        if verbose {
            set_log_level(LogLevel::Debug);
        } else {
            set_log_level(LogLevel::Info);
        }
        // Pass `daemon=true` so init_logging does not print anything for clients
        utils::init_logging(verbose).await;
        let log_path = utils::get_log_path();
        utils::mark_new_run(&log_path, "Stasis start"); // Only once per daemon start
    }
    // --- Handle client commands ---
    if let Some(cmd) = command_opt {
        client::handle_client_command(&cmd).await.map_err(|_| {
            event_error_scoped!(Scope::Client, "Client command failed");
            AppError::ClientCommandFailed
        })?;
        return Ok(());
    }
    // --- Ensure Wayland ---
    if var("WAYLAND_DISPLAY").is_err() {
        event_warn_scoped!(Scope::Wayland, "Stasis requires Wayland to run.");
        exit(1);
    }
    // --- Single-instance enforcement ---
    let help_or_version = std::env::args()
        .any(|a| matches!(a.as_str(), "-V" | "--version" | "-h" | "--help" | "help"));
    if UnixStream::connect(SOCKET_PATH).await.is_ok() {
        if !help_or_version {
            event_warn_scoped!(Scope::Core, "Another instance of Stasis is already running");
        }
        return Ok(());
    }
    // Remove old socket
    if let Err(e) = fs::remove_file(SOCKET_PATH) {
        if e.kind() != std::io::ErrorKind::NotFound {
            event_error_scoped!(Scope::Core, "Failed to remove existing socket: {}", e);
            return Err(AppError::SocketBindFailed);
        }
    }
    let listener = UnixListener::bind(SOCKET_PATH).map_err(|_| {
        event_error_scoped!(
            Scope::Core,
            "Failed to bind control socket. Another instance may be running."
        );
        AppError::SocketBindFailed
    })?;
    event_info_scoped!(Scope::Core, "Control socket bound at {}", SOCKET_PATH);
    // --- Ensure user config ---
    if let Err(e) = config::bootstrap::ensure_user_config_exists() {
        event_warn_scoped!(Scope::Config, "Could not initialize config: {}", e);
    } else {
        event_debug_scoped!(Scope::Config, "User config initialized");
    }

    // --- Run daemon ---
    event_info_scoped!(Scope::Daemon, "Starting daemon...");
    daemon::run_daemon(listener, verbose).await.map_err(|_| {
        event_error_scoped!(Scope::Daemon, "Daemon failed to start");
        AppError::DaemonFailed
    })?;
    event_info_scoped!(Scope::Daemon, "Daemon stopped cleanly");

    runtime::runtime_summary(true, None, false).await;
    Ok(())
}
