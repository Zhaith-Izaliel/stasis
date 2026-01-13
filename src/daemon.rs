use std::sync::Arc;
use tokio::{
    net::UnixListener,
    sync::Mutex,
    task::LocalSet,
    time::Duration,
};

#[derive(Debug)]
pub enum DaemonError {
    ConfigLoadFailed,
    WaylandSetupFailed,
}

use crate::{
    SOCKET_PATH, config::parser::load_combined_config, core::{
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
use eventline::{event_info_scoped, event_error_scoped};
use eventline::runtime::log_level::{set_log_level, LogLevel};

/// Spawn the daemon with all its background services
pub async fn run_daemon(listener: UnixListener, verbose: bool) -> Result<(), DaemonError> {
    // Load config
    // Set log level based on verbose flag
    if verbose {
        set_log_level(LogLevel::Debug);
        event_info_scoped!("Stasis", "Verbose mode enabled, log level set to DEBUG").await;
    } else {
        set_log_level(LogLevel::Info);
    }

    let combined_cfg = load_combined_config().await
        .map_err(|_| DaemonError::ConfigLoadFailed)?;
    let cfg = Arc::new(combined_cfg.base.clone());
    let manager = Manager::new_with_profiles(&combined_cfg);
    let manager = Arc::new(Mutex::new(manager));

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
            tokio::spawn(event_error_scoped!("D-Bus", "Suspend event listener failed: {}", e));
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
            tokio::spawn(event_error_scoped!("MPRIS", "Failed to spawn media monitor: {}", e));
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

    // IPC control socket
    ipc::spawn_ipc_socket_with_listener(
        Arc::clone(&manager),
        listener,
    ).await;

    // Shutdown watcher loops
    setup_shutdown_handler(Arc::clone(&manager)).await;
    spawn_wayland_monitor(Arc::clone(&manager)).await;

    // Log startup message
    tokio::spawn(event_info_scoped!("Stasis", "Stasis started! Idle actions loaded: {}", cfg.actions.len()));

    // Run main async tasks
    let local = LocalSet::new();
    local.run_until(std::future::pending::<()>()).await;

    Ok(())
}

/// Async shutdown handler (Ctrl+C / SIGTERM)
async fn setup_shutdown_handler(
    manager: Arc<Mutex<Manager>>,
) {
    let mut sigint = tokio::signal::unix::signal(tokio::signal::unix::SignalKind::interrupt()).unwrap();
    let mut sigterm = tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate()).unwrap();
    let mut sighup = tokio::signal::unix::signal(tokio::signal::unix::SignalKind::hangup()).unwrap();

    tokio::spawn({
        let manager = Arc::clone(&manager);
        async move {
            tokio::select! {
                _ = sigint.recv() => {
                    tokio::spawn(event_info_scoped!("Stasis", "Received SIGINT, shutting down..."));
                },
                _ = sigterm.recv() => {
                    tokio::spawn(event_info_scoped!("Stasis", "Received SIGTERM, shutting down..."));
                },
                _ = sighup.recv() => {
                    tokio::spawn(event_info_scoped!("Stasis", "Received SIGHUP, shutting down..."));
                },
            }

            manager.lock().await.shutdown().await;

            let _ = std::fs::remove_file(SOCKET_PATH);
            tokio::spawn(event_info_scoped!("Stasis", "Shutdown complete, goodbye!"));
            std::process::exit(0);
        }
    });
}

// Watches for wayland socket being dead and stops Stasis
async fn spawn_wayland_monitor(
    manager: Arc<Mutex<Manager>>,
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
                tokio::spawn(event_info_scoped!("Stasis", "Wayland compositor is no longer responding, shutting down..."));

                manager.lock().await.shutdown().await;

                let _ = std::fs::remove_file(SOCKET_PATH);
                tokio::spawn(event_info_scoped!("Stasis", "Shutdown complete, goodbye!"));
                std::process::exit(0);
            }
        }
    });
}
