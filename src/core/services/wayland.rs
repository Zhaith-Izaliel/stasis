use eyre::Result;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use crate::core::manager::Manager;
use crate::log::{log_error_message, log_wayland_message};

use tokio::sync::Notify;

use wayland_client::{
    protocol::{wl_registry, wl_seat::WlSeat},
    Connection, Dispatch, QueueHandle,
};
use wayland_protocols::ext::idle_notify::v1::client::{
    ext_idle_notifier_v1::ExtIdleNotifierV1,
    ext_idle_notification_v1::{ExtIdleNotificationV1, Event as IdleEvent},
};
use wayland_protocols::wp::idle_inhibit::zv1::client::{
    zwp_idle_inhibit_manager_v1::{ZwpIdleInhibitManagerV1, Event as InhibitMgrEvent},
    zwp_idle_inhibitor_v1::{ZwpIdleInhibitorV1, Event as InhibitorEvent},
};

pub struct WaylandIdleData {
    pub manager: Arc<tokio::sync::Mutex<Manager>>,
    pub idle_notifier: Option<ExtIdleNotifierV1>,
    pub seat: Option<WlSeat>,
    pub notification: Option<ExtIdleNotificationV1>,
    pub inhibit_manager: Option<ZwpIdleInhibitManagerV1>,
    pub active_inhibitors: u32,
    pub respect_inhibitors: bool,
    pub shutdown: Arc<Notify>,
    pub should_stop: Arc<AtomicBool>,
}

impl WaylandIdleData {
    pub fn new(manager: Arc<tokio::sync::Mutex<Manager>>, respect_inhibitors: bool) -> Self {
        Self {
            manager,
            idle_notifier: None,
            seat: None,
            notification: None,
            inhibit_manager: None,
            active_inhibitors: 0,
            respect_inhibitors,
            shutdown: Arc::new(Notify::new()),
            should_stop: Arc::new(AtomicBool::new(false)),
        }
    }

    pub fn is_inhibited(&self) -> bool {
        self.respect_inhibitors && self.active_inhibitors > 0
    }
}


/// Bind registry globals
impl Dispatch<wl_registry::WlRegistry, ()> for WaylandIdleData {
    fn event(
        state: &mut Self,
        registry: &wl_registry::WlRegistry,
        event: wl_registry::Event,
        _: &(),
        _: &Connection,
        qh: &QueueHandle<Self>,
    ) {
        if let wl_registry::Event::Global { name, interface, .. } = event {
            match interface.as_str() {
                "ext_idle_notifier_v1" => {
                    state.idle_notifier =
                        Some(registry.bind::<ExtIdleNotifierV1, _, _>(name, 1, qh, ()));
                    log_wayland_message("Binding ext_idle_notifier_v1");
                }
                "wl_seat" => {
                    state.seat = Some(registry.bind::<WlSeat, _, _>(name, 1, qh, ()));
                    log_wayland_message("Binding wl_seat");
                }
                "zwp_idle_inhibit_manager_v1" => {
                    state.inhibit_manager =
                        Some(registry.bind::<ZwpIdleInhibitManagerV1, _, _>(name, 1, qh, ()));
                    log_wayland_message("Binding zwp_idle_inhibit_manager_v1");
                }
                _ => {}
            }
        }
    }
}

impl Dispatch<ExtIdleNotifierV1, ()> for WaylandIdleData {
    fn event(
        _: &mut Self,
        _: &ExtIdleNotifierV1,
        _: <ExtIdleNotifierV1 as wayland_client::Proxy>::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {}
}

impl Dispatch<ExtIdleNotificationV1, ()> for WaylandIdleData {
    fn event(
        state: &mut Self,
        _: &ExtIdleNotificationV1,
        event: IdleEvent,
        _: &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
        let manager = Arc::clone(&state.manager);
        let inhibited = state.is_inhibited();

        tokio::spawn(async move {
            if inhibited {
                log_wayland_message("Idle inhibited; skipping idle trigger");
                return;
            }

            let mut mgr = manager.lock().await;

            match event {
                IdleEvent::Idled => {
                    // Handled internally by libinput
                }
                IdleEvent::Resumed => { 
                    if mgr.state.lock.is_locked && !mgr.state.actions.post_lock_resume_queue.is_empty() {
                        log_wayland_message("Activity detected while locked - firing post-lock resume commands");
                        mgr.fire_post_lock_resume_queue().await;
                    }

                }
                _ => {}
            }
        });
    }
}


impl Dispatch<ZwpIdleInhibitorV1, ()> for WaylandIdleData {
    fn event(
        state: &mut Self,
        _proxy: &ZwpIdleInhibitorV1,
        _event: InhibitorEvent,
        _: &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
        state.active_inhibitors += 1;
        log_wayland_message(&format!("Inhibitor created, count={}", state.active_inhibitors));
    }
}

impl Dispatch<ZwpIdleInhibitManagerV1, ()> for WaylandIdleData {
    fn event(
        state: &mut Self,
        _proxy: &ZwpIdleInhibitManagerV1,
        _event: InhibitMgrEvent,
        _: &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
        if state.active_inhibitors > 0 {
            state.active_inhibitors -= 1;
            log_wayland_message(&format!("Inhibitor removed, count={}", state.active_inhibitors));
        }
    }
}

impl Dispatch<WlSeat, ()> for WaylandIdleData {
    fn event(
        _: &mut Self,
        _: &WlSeat,
        _: wayland_client::protocol::wl_seat::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {}
}


pub async fn setup(
    manager: Arc<tokio::sync::Mutex<Manager>>,
    respect_inhibitors: bool,
) -> Result<Arc<tokio::sync::Mutex<WaylandIdleData>>> {
    log_wayland_message(&format!(
        "Setting up Wayland idle detection (respect_inhibitors={})",
        respect_inhibitors
    ));

    // Connect to Wayland
    let conn = Connection::connect_to_env()
        .map_err(|e| eyre::eyre!("Failed to connect to Wayland: {}", e))?;
    let mut event_queue = conn.new_event_queue();
    let qh = event_queue.handle();
    let display = conn.display();

    // Initialize WaylandIdleData
    let mut app_data = WaylandIdleData::new(manager.clone(), respect_inhibitors);

    // Bind globals
    let _registry = display.get_registry(&qh, ());
    event_queue.roundtrip(&mut app_data)?;

    // Request idle notification if both notifier and seat are available
    if let (Some(notifier), Some(seat)) = (&app_data.idle_notifier, &app_data.seat) {
        let timeout_ms = 100;
        let notification = notifier.get_idle_notification(timeout_ms, seat, &qh, ());
        app_data.notification = Some(notification);
        log_wayland_message("Wayland idle detection active");
    }

    let should_stop = Arc::clone(&app_data.should_stop);
    
    // Wrap in Arc<Mutex>
    let app_data = Arc::new(tokio::sync::Mutex::new(app_data));

    let shutdown_flag = {
        let mgr = manager.lock().await;
        Arc::clone(&mgr.state.shutdown_flag)
    };

    // Spawn task to set stop flag when shutdown is triggered
    tokio::spawn({
        let should_stop = Arc::clone(&should_stop);
        async move {
            shutdown_flag.notified().await;
            should_stop.store(true, Ordering::Relaxed);
        }
    });

    // Event loop using blocking_dispatch in a blocking task
    tokio::task::spawn_blocking({
        let app_data = Arc::clone(&app_data);
        move || {
            log_wayland_message("Wayland event loop started");
            loop {
                if should_stop.load(Ordering::Relaxed) {
                    break;
                }
                
                let mut locked_data = tokio::runtime::Handle::current()
                    .block_on(app_data.lock());
                
                // Use blocking_dispatch which waits for events
                match event_queue.blocking_dispatch(&mut *locked_data) {
                    Ok(_) => {},
                    Err(e) => {
                        log_error_message(&format!("Wayland dispatch error: {}", e));
                        break;
                    }
                }
            }
            log_wayland_message("Wayland event loop shutting down...");
        }
    });

    Ok(app_data)
}
