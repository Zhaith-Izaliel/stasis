pub mod cli;
pub mod client;
pub mod config;
pub mod core;
pub mod daemon;
pub mod ipc;
pub mod log;
pub mod media_bridge;

use std::{env::var, fs, process::exit};
use clap::Parser;
use tokio::net::{UnixListener, UnixStream};
use crate::cli::Args;

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
        eprintln!("Fatal error: {:?}", err);
        exit(1);
    }
}

async fn real_main() -> Result<(), AppError> {
    let args = Args::parse();

    log::set_verbose(args.verbose);

    // --- Handle subcommands via socket FIRST ---
    if let Some(cmd) = &args.command {
        client::handle_client_command(cmd)
            .await
            .map_err(|_| AppError::ClientCommandFailed)?;
        return Ok(());
    }

    // --- Now check for Wayland ---
    if var("WAYLAND_DISPLAY").is_err() {
        eprintln!("Warn: Stasis requires wayland to run.");
        exit(1);
    }

    // --- Single Instance enforcement ---
    let just_help_or_version = std::env::args().any(|a| {
        matches!(a.as_str(), "-V" | "--version" | "-h" | "--help" | "help")
    });

    if UnixStream::connect(SOCKET_PATH).await.is_ok() {
        if !just_help_or_version {
            eprintln!("Another instance of Stasis is already running");
        }
        return Ok(());
    }

    let _ = fs::remove_file(SOCKET_PATH);

    let listener = UnixListener::bind(SOCKET_PATH)
        .map_err(|_| AppError::SocketBindFailed)?;

    // --- Ensure user config exists ---
    if let Err(e) = config::bootstrap::ensure_user_config_exists() {
        eprintln!("Could not initialize config: {}", e);
    }

    // --- Run daemon ---
    daemon::run_daemon(listener, args.verbose)
        .await
        .map_err(|_| AppError::DaemonFailed)?;

    Ok(())
}
