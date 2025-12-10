use tokio::task::JoinHandle;

use crate::sinfo;

/// Hard cap on concurrent background tasks.
const MAX_SPAWNED_TASKS: usize = 10;

pub struct TaskManager {
    pub spawned_tasks: Vec<JoinHandle<()>>,
    pub idle_task_handle: Option<JoinHandle<()>>,
    pub lock_task_handle: Option<JoinHandle<()>>,
    pub media_task_handle: Option<JoinHandle<()>>,
    pub input_task_handle: Option<JoinHandle<()>>,
}

impl TaskManager {
    pub fn new() -> Self {
        Self {
            spawned_tasks: Vec::new(),
            idle_task_handle: None,
            lock_task_handle: None,
            media_task_handle: None,
            input_task_handle: None,
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
            sinfo!("Stasis", "Max spawned tasks reached, skipping task spawn");
        }
    }

    /// Abort all tasks including optional handles
    pub fn abort_all(&mut self) {
        if let Some(handle) = self.idle_task_handle.take() { handle.abort(); }
        if let Some(handle) = self.lock_task_handle.take() { handle.abort(); }
        if let Some(handle) = self.media_task_handle.take() { handle.abort(); }

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
        sinfo!("Stasis", "Max spawned tasks reached, skipping task spawn");
    }
}


