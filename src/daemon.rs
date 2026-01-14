use std::sync::Arc;
use eventline::{event_error_scoped, event_info_scoped};
use eventline::runtime::log_level::{set_log_level, LogLevel};
use tokio::{
    net::UnixListener,
    sync::{Mutex, mpsc},
    time::Duration,
};

use crate::{
    SOCKET_PATH, 
    config::parser::load_combined_config,
    scopes::Scope,
    core::{
        manager::{Manager, idle_loops::{spawn_idle_task, spawn_lock_watcher}},
        services::{
            app_inhibit::spawn_app_inhibit_task, 
            browser_media::spawn_browser_bridge_detector, 
            dbus::listen_for_power_events, 
            input::spawn_input_task, 
            media::spawn_media_monitor_dbus, 
            power_detection::{detect_initial_power_state, spawn_power_source_monitor}, 
            wayland::setup as setup_wayland
        }
    }, 
    ipc
};

// Global shutdown channel sender - IPC handlers can use this
pub type ShutdownSender = mpsc::Sender<&'static str>;

#[derive(Debug)]
pub enum DaemonError {
    ConfigLoadFailed,
    WaylandSetupFailed,
}

/// Spawn the daemon with all its background services
pub async fn run_daemon(listener: UnixListener, verbose: bool) -> Result<(), DaemonError> {
    // Load config
    // Set log level based on verbose flag
    if verbose {
        set_log_level(LogLevel::Debug);
        event_info_scoped!(Scope::Core, "Verbose mode enabled, log level set to DEBUG");
    } else {
        set_log_level(LogLevel::Info);
    }

    let combined_cfg = load_combined_config().await
        .map_err(|_| DaemonError::ConfigLoadFailed)?;
    let cfg = Arc::new(combined_cfg.base.clone());
    let manager = Manager::new_with_profiles(&combined_cfg);
    let manager = Arc::new(Mutex::new(manager));

    // Create shutdown channel
    let (shutdown_tx, mut shutdown_rx) = mpsc::channel::<&'static str>(1);

    // Spawn internal background tasks
    {
        let mut mgr = manager.lock().await;
        mgr.tasks.spawn_limited(spawn_idle_task(Arc::clone(&manager)));
        mgr.tasks.spawn_limited(spawn_lock_watcher(Arc::clone(&manager)));
        mgr.tasks.spawn_limited(spawn_input_task(Arc::clone(&manager)));
    }

    // Spawn suspend event listener
    let dbus_manager = Arc::clone(&manager);
    tokio::spawn(async move {
        if let Err(e) = listen_for_power_events(dbus_manager).await {
            event_error_scoped!(Scope::DBus, "Suspend event listener failed: {}", e);
        }
    });

    // Initial AC/battery detection (synchronously)
    detect_initial_power_state(&manager).await;

    // AC/battery detection (background task)
    let laptop_manager = Arc::clone(&manager);
    tokio::spawn(spawn_power_source_monitor(laptop_manager));

    // Immediately trigger instant actions at startup
    {
        let mut mgr = manager.lock().await;
        mgr.trigger_instant_actions().await;
    }

    // Spawn app inhibit task
    {
        let (app_inhibitor, app_inhibitor_handle) = spawn_app_inhibit_task(Arc::clone(&manager)).await;
        let mut mgr_guard = manager.lock().await;
        mgr_guard.tasks.app_inhibitor_task_handle = Some(app_inhibitor_handle);
        mgr_guard.state.app.attach_inhibitor(app_inhibitor);
    }

    // Spawn media monitors
    if cfg.monitor_media {
        // MPRIS monitor
        if let Err(e) = spawn_media_monitor_dbus(Arc::clone(&manager)).await {
            event_error_scoped!(Scope::Media, "Failed to spawn media monitor: {}", e);
        }

        // Browser bridge detector
        spawn_browser_bridge_detector(Arc::clone(&manager)).await;
    }

    // Wayland inhibitors integration loop setup
    let wayland_manager = Arc::clone(&manager);
    setup_wayland(
        wayland_manager,
        cfg.respect_wayland_inhibitors,
    )
    .await
    .map_err(|_| DaemonError::WaylandSetupFailed)?;

    // IPC control socket - pass shutdown sender so IPC stop command can trigger shutdown
    ipc::spawn_ipc_socket_with_listener(
        Arc::clone(&manager),
        listener,
        shutdown_tx.clone(),
    ).await;

    // Shutdown watcher loops
    setup_shutdown_handler(shutdown_tx.clone()).await;
    spawn_wayland_monitor(shutdown_tx.clone()).await;

    // Log startup message
    event_info_scoped!(Scope::Core, "Stasis started! Idle actions loaded: {}", cfg.actions.len());

    // Wait for shutdown signal
    let shutdown_reason = shutdown_rx.recv().await.unwrap_or("unknown");
    
    event_info_scoped!("Stasis", "Shutdown initiated: {}", shutdown_reason);
    
    // Perform cleanup
    manager.lock().await.shutdown().await;
    
    // Give shutdown events time to be logged
    tokio::time::sleep(Duration::from_millis(100)).await;
    
    // Clean up socket
    let _ = std::fs::remove_file(SOCKET_PATH);
    
    event_info_scoped!(Scope::Core, "Shutdown complete, goodbye!");
    
    // Give final log time to complete
    tokio::time::sleep(Duration::from_millis(100)).await;
    
    Ok(())
}

/// Async shutdown handler (Ctrl+C / SIGTERM)
async fn setup_shutdown_handler(
    shutdown_tx: mpsc::Sender<&'static str>,
) {
    let mut sigint = tokio::signal::unix::signal(tokio::signal::unix::SignalKind::interrupt()).unwrap();
    let mut sigterm = tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate()).unwrap();
    let mut sighup = tokio::signal::unix::signal(tokio::signal::unix::SignalKind::hangup()).unwrap();

    tokio::spawn(async move {
        let signal_name = tokio::select! {
            _ = sigint.recv() => "SIGINT",
            _ = sigterm.recv() => "SIGTERM",
            _ = sighup.recv() => "SIGHUP",
        };

        event_info_scoped!(Scope::Core, "Received {}", signal_name);
        let _ = shutdown_tx.send(signal_name).await;
    });
}

// Watches for wayland socket being dead and stops Stasis
async fn spawn_wayland_monitor(
    shutdown_tx: mpsc::Sender<&'static str>,
) {
    use tokio::net::UnixStream;

    // Capture env vars once
    let wayland_display = match std::env::var("WAYLAND_DISPLAY") {
        Ok(display) => display,
        Err(_) => return,
    };
    let xdg_runtime = std::env::var("XDG_RUNTIME_DIR").unwrap_or_else(|_| "/run/user/1000".to_string());
    let socket_path = format!("{}/{}", xdg_runtime, wayland_display);

    tokio::spawn(async move {
        loop {
            tokio::time::sleep(Duration::from_secs(2)).await;

            // Try connecting to the Wayland socket
            if UnixStream::connect(&socket_path).await.is_err() {
                event_info_scoped!(Scope::Core, "Wayland compositor is no longer responding");
                let _ = shutdown_tx.send("Wayland disconnect").await;
                break;
            }
        }
    });
}
