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
use eyre::Result;
use tokio::net::{UnixListener, UnixStream};
use crate::cli::Args;

pub const SOCKET_PATH: &str = "/tmp/stasis.sock";

#[tokio::main(flavor = "current_thread")]
async fn main() -> Result<()> {
    let args = Args::parse();
    
    if args.verbose {
        log::set_verbose(true);
    } else {
        log::set_verbose(false);
    }
    
    // --- Handle subcommands via socket FIRST (before Wayland check) ---
    if let Some(cmd) = &args.command {
        return client::handle_client_command(cmd).await;
    }
    
    // --- Now check for Wayland (only needed for daemon) ---
    if var("WAYLAND_DISPLAY").is_err() {
        eprintln!("Warn: Stasis requires wayland to run.");
        exit(1);
    }
    
    // --- Single Instance enforcement ---
    let just_help_or_version = std::env::args().any(|a| matches!(a.as_str(), "-V" | "--version" | "-h" | "--help" | "help"));
    if UnixStream::connect(SOCKET_PATH).await.is_ok() {
        if !just_help_or_version {
            eprintln!("Another instance of Stasis is already running");
        }
        return Ok(());
    }
    
    let _ = fs::remove_file(SOCKET_PATH);
    let listener = UnixListener::bind(SOCKET_PATH).map_err(|_| {
        eyre::eyre!("Failed to bind control socket. Another instance may be running.")
    })?;
    
    // --- Ensure user config exists ---
    if let Err(e) = config::bootstrap::ensure_user_config_exists() {
        eprintln!("Could not initialize config: {}", e);
    }
    
    // --- Run daemon ---
    daemon::run_daemon(listener, args.verbose).await
}
