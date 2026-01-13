use tokio::task::JoinHandle;
use tokio::time::{timeout, Duration};
use eventline::{event_info_scoped, event_error_scoped};

/// Hard cap on concurrent background tasks.
const MAX_SPAWNED_TASKS: usize = 10;

#[derive(Debug)]
pub struct TaskManager {
    pub spawned_tasks: Vec<JoinHandle<()>>,
    pub idle_task_handle: Option<JoinHandle<()>>,
    pub lock_task_handle: Option<JoinHandle<()>>,
    pub media_task_handle: Option<JoinHandle<()>>,
    pub input_task_handle: Option<JoinHandle<()>>,
    pub app_inhibitor_task_handle: Option<JoinHandle<()>>,
}

impl TaskManager {
    pub fn new() -> Self {
        Self {
            spawned_tasks: Vec::new(),
            idle_task_handle: None,
            lock_task_handle: None,
            media_task_handle: None,
            input_task_handle: None,
            app_inhibitor_task_handle: None,
        }
    }

    /// Clean up finished tasks from a vector of JoinHandles.
    fn cleanup_tasks(tasks: &mut Vec<JoinHandle<()>>) {
        tasks.retain(|h| !h.is_finished());
    }

    /// Spawn a task while respecting the MAX_SPAWNED_TASKS limit.
    /// Automatically cleans up completed tasks before spawning.
    pub fn spawn_limited<F>(&mut self, fut: F)
    where
        F: std::future::Future<Output = ()> + Send + 'static,
    {
        Self::cleanup_tasks(&mut self.spawned_tasks);
        if self.spawned_tasks.len() < MAX_SPAWNED_TASKS {
            self.spawned_tasks.push(tokio::spawn(fut));
        } else {
            tokio::spawn(event_info_scoped!("Stasis", "Max spawned tasks reached, skipping task spawn"));
        }
    }

    pub async fn shutdown_all(&mut self) {
        let shutdown_timeout = Duration::from_millis(500);

        // Helper to await with timeout, falling back to abort
        async fn await_or_abort(handle: JoinHandle<()>, name: String, shutdown_timeout: Duration) {
            match timeout(shutdown_timeout, handle).await {
                Ok(Ok(())) => {
                    // Task finished cleanly
                }
                Ok(Err(e)) => {
                    event_error_scoped!("Stasis", "{} task panicked: {}", name, e).await;
                }
                Err(_) => {
                    event_info_scoped!("Stasis", "{} task didn't finish in time, aborting", name).await;
                    // Note: handle was consumed by timeout, already dropped
                }
            }
        }

        // Await all tracked tasks
        if let Some(handle) = self.idle_task_handle.take() {
            await_or_abort(handle, "Idle".to_string(), shutdown_timeout).await;
        }
        if let Some(handle) = self.lock_task_handle.take() {
            await_or_abort(handle, "Lock watcher".to_string(), shutdown_timeout).await;
        }
        if let Some(handle) = self.media_task_handle.take() {
            await_or_abort(handle, "Media monitor".to_string(), shutdown_timeout).await;
        }
        if let Some(handle) = self.input_task_handle.take() {
            await_or_abort(handle, "Input".to_string(), shutdown_timeout).await;
        }
        if let Some(handle) = self.app_inhibitor_task_handle.take() {
            await_or_abort(handle, "App inhibitor".to_string(), shutdown_timeout).await;
        }

        // Await spawned tasks
        for handle in self.spawned_tasks.drain(..) {
            let _ = timeout(shutdown_timeout, handle).await;
        }
    }

    /// This is kept for emergency abort if needed
    pub fn abort_all(&mut self) {
        if let Some(handle) = self.idle_task_handle.take() { handle.abort(); }
        if let Some(handle) = self.lock_task_handle.take() { handle.abort(); }
        if let Some(handle) = self.media_task_handle.take() { handle.abort(); }
        if let Some(handle) = self.input_task_handle.take() { handle.abort(); }
        if let Some(handle) = self.app_inhibitor_task_handle.take() { handle.abort(); }
        
        for handle in self.spawned_tasks.drain(..) {
            handle.abort();
        }
    }
}

/// Clean up finished tasks from a vector of JoinHandles.
pub fn cleanup_tasks(tasks: &mut Vec<JoinHandle<()>>) {
    tasks.retain(|h| !h.is_finished());
}

/// Spawn a task while respecting the MAX_SPAWNED_TASKS limit.
/// Automatically cleans up completed tasks before spawning.
pub fn spawn_task_limited<F>(tasks: &mut Vec<JoinHandle<()>>, fut: F)
where
    F: std::future::Future<Output = ()> + Send + 'static,
{
    cleanup_tasks(tasks);
    if tasks.len() < MAX_SPAWNED_TASKS {
        tasks.push(tokio::spawn(fut));
    } else {
        tokio::spawn(event_info_scoped!("Stasis", "Max spawned tasks reached, skipping task spawn"));
    }
}
