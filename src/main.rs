pub mod cli;
pub mod client;
pub mod config;
pub mod core;
pub mod daemon;
pub mod ipc;
pub mod media_bridge;
pub mod scopes;
pub mod utils;

use std::{env::var, fs, process::exit};
use clap::Parser;
use tokio::net::{UnixListener, UnixStream};

use crate::cli::Args;
use utils::{init_logging, save_journal};
use eventline::runtime;
use eventline::runtime::log_level::{set_log_level, LogLevel};
use eventline::{event_info_scoped, event_warn_scoped, event_error_scoped, event_debug_scoped};

pub const SOCKET_PATH: &str = "/tmp/stasis.sock";

#[derive(Debug)]
pub enum AppError {
    ClientCommandFailed,
    SocketBindFailed,
    DaemonFailed,
}

#[tokio::main(flavor = "current_thread")]
async fn main() {
    let result = real_main().await;

    if let Err(err) = result {
        event_error_scoped!("AppError", "Fatal error: {:?}", err).await;
        save_journal().await;
        exit(1);
    }

    save_journal().await;
}

async fn real_main() -> Result<(), AppError> {
    let args = Args::parse();
    let verbose = args.verbose;
    let command_opt = args.command.clone();

    // --- Initialize Eventline runtime ---
    runtime::init().await;
    runtime::enable_console_output(true);
    runtime::enable_console_color(true);
    if verbose {
        set_log_level(LogLevel::Debug);
    } else {
        set_log_level(LogLevel::Info);
    }

    // --- Initialize logging and mark new run ---
    init_logging(verbose).await;
    let log_path = utils::get_log_path();
    crate::utils::mark_new_run(&log_path, "Stasis start");
  
    // --- Handle client commands ---
    if let Some(cmd) = command_opt {
        let cmd_clone = cmd.clone(); // clone for logging
        event_debug_scoped!("Client", "Handling client command: {:?}", cmd_clone).await;

        client::handle_client_command(&cmd).await.map_err(|_| {
            futures::executor::block_on(event_error_scoped!("Client", "Client command failed"));
            AppError::ClientCommandFailed
        })?;

        return Ok(());
    }


    // --- Ensure Wayland ---
    if var("WAYLAND_DISPLAY").is_err() {
        event_warn_scoped!("Wayland", "Stasis requires Wayland to run.").await;
        exit(1);
    }

    // --- Single-instance enforcement and daemon startup ---
    let help_or_version = std::env::args()
        .any(|a| matches!(a.as_str(), "-V" | "--version" | "-h" | "--help" | "help"));

    if UnixStream::connect(SOCKET_PATH).await.is_ok() {
        if !help_or_version {
            event_warn_scoped!("Core", "Another instance of Stasis is already running").await;
        }
        return Ok(());
    }

    // Remove old socket if present
    if let Err(e) = fs::remove_file(SOCKET_PATH) {
        if e.kind() != std::io::ErrorKind::NotFound {
            event_error_scoped!("Core", "Failed to remove existing socket: {}", e).await;
            return Err(AppError::SocketBindFailed);
        }
    }

    // Bind control socket
    let listener = UnixListener::bind(SOCKET_PATH).map_err(|_| {
        futures::executor::block_on(event_error_scoped!("Core", "Failed to bind control socket. Another instance may be running."));
        AppError::SocketBindFailed
    })?;
    event_info_scoped!("Core", "Control socket bound at {}", SOCKET_PATH).await;

    // --- Ensure user config ---
    if let Err(e) = config::bootstrap::ensure_user_config_exists() {
        event_warn_scoped!("Config", "Could not initialize config: {}", e).await;
    } else {
        event_debug_scoped!("Config", "User config initialized").await;
    }

    // --- Run daemon ---
    event_info_scoped!("Daemon", "Starting daemon...").await;
    daemon::run_daemon(listener, verbose).await.map_err(|_| {
        futures::executor::block_on(event_error_scoped!("Daemon", "Daemon failed to start"));
        AppError::DaemonFailed
    })?;
    event_info_scoped!("Daemon", "Daemon stopped cleanly").await;

    Ok(())
}
