//! Event stream for watching changes to storage files.
//!
//! This module provides filesystem watching that emits semantic events when
//! log or plan files change. This is currently only implemented for
//! FileSystemStorage, but could be extended to other storage backends
//! that support change notifications.

use notify::{Config, Event, RecommendedWatcher, RecursiveMode, Watcher};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, LazyLock, Mutex};
use std::time::Duration;
use tokio::sync::broadcast;

/// Semantic events emitted when storage files change.
#[derive(Clone, Debug)]
pub enum StorageEvent {
    /// A log file was modified, created, or removed.
    LogChanged(PathBuf),
    /// A plan file was modified, created, or removed.
    PlanChanged(PathBuf),
}

/// Handle to an event stream.
///
/// This handle allows multiple subscribers to receive events via broadcast channels.
pub struct EventStreamHandle {
    sender: broadcast::Sender<StorageEvent>,
}

impl EventStreamHandle {
    /// Subscribe to the event stream.
    ///
    /// Returns a receiver that will receive all future events.
    /// Multiple subscribers can be created from the same handle.
    pub fn subscribe(&self) -> broadcast::Receiver<StorageEvent> {
        self.sender.subscribe()
    }
}

struct Route {
    logs_dir: PathBuf,
    plans_dir: PathBuf,
    sender: broadcast::Sender<StorageEvent>,
    last_event_time: HashMap<PathBuf, std::time::Instant>,
}

struct GlobalWatcher {
    watcher: RecommendedWatcher,
    routes: Arc<Mutex<Vec<Route>>>,
}

// Single process-level OS watcher. notify's FSEvents backend (macOS) is designed
// for one watcher per process — multiple concurrent watchers contend on the FSEvents
// callback thread and cause spurious event delivery failures.
static GLOBAL_WATCHER: LazyLock<Mutex<GlobalWatcher>> = LazyLock::new(|| {
    let routes: Arc<Mutex<Vec<Route>>> = Arc::new(Mutex::new(Vec::new()));
    let routes_for_thread = routes.clone();

    let (event_tx, event_rx) = std::sync::mpsc::channel();
    let watcher =
        RecommendedWatcher::new(event_tx, Config::default()).expect("Failed to create watcher");

    std::thread::spawn(move || {
        let debounce_duration = Duration::from_millis(200);
        loop {
            match event_rx.recv() {
                Ok(Ok(event)) => {
                    let mut routes = routes_for_thread.lock().unwrap();
                    for route in routes.iter_mut() {
                        if let Some(storage_event) =
                            process_event(&event, &route.logs_dir, &route.plans_dir)
                        {
                            let path = match &storage_event {
                                StorageEvent::LogChanged(p) | StorageEvent::PlanChanged(p) => {
                                    p.clone()
                                }
                            };
                            let now = std::time::Instant::now();
                            let should_emit = route
                                .last_event_time
                                .get(&path)
                                .map(|t| now.duration_since(*t) > debounce_duration)
                                .unwrap_or(true);
                            if should_emit {
                                route.last_event_time.insert(path, now);
                                let _ = route.sender.send(storage_event);
                            }
                        }
                    }
                }
                Ok(Err(e)) => {
                    eprintln!("[faff-core] Filesystem watcher error: {:?}", e);
                }
                Err(_) => break,
            }
        }
    });

    Mutex::new(GlobalWatcher { watcher, routes })
});

/// Register a filesystem watcher for the given base directory.
///
/// This is used internally by FileSystemStorage to implement event support.
/// The watcher monitors the `logs/` and `plans/` subdirectories and emits
/// semantic events when files change.
///
/// Events are debounced by ~200ms and filtered to only include actual
/// content changes (not metadata-only changes).
pub(crate) fn spawn_filesystem_watcher(base_dir: PathBuf) -> EventStreamHandle {
    let (tx, _rx) = broadcast::channel(100);

    let logs_dir = base_dir
        .join("logs")
        .canonicalize()
        .unwrap_or_else(|_| base_dir.join("logs"));
    let plans_dir = base_dir
        .join("plans")
        .canonicalize()
        .unwrap_or_else(|_| base_dir.join("plans"));

    let mut global = GLOBAL_WATCHER.lock().unwrap();

    if logs_dir.exists() {
        global
            .watcher
            .watch(&logs_dir, RecursiveMode::NonRecursive)
            .expect("Failed to watch logs directory");
    }
    if plans_dir.exists() {
        global
            .watcher
            .watch(&plans_dir, RecursiveMode::NonRecursive)
            .expect("Failed to watch plans directory");
    }

    global.routes.lock().unwrap().push(Route {
        logs_dir,
        plans_dir,
        sender: tx.clone(),
        last_event_time: HashMap::new(),
    });

    EventStreamHandle { sender: tx }
}

/// Process a raw filesystem event and convert it to a semantic StorageEvent.
fn process_event(event: &Event, logs_dir: &Path, plans_dir: &Path) -> Option<StorageEvent> {
    // Only care about content changes, creates, and removes - ignore metadata-only changes
    match event.kind {
        notify::EventKind::Create(_) | notify::EventKind::Remove(_) => {}
        notify::EventKind::Modify(modify_kind) => {
            // Process data content changes and name changes (for iCloud sync compatibility)
            match modify_kind {
                notify::event::ModifyKind::Data(_) => {}
                notify::event::ModifyKind::Name(_) => {
                    // iCloud Drive syncs files by atomically replacing them,
                    // which generates Name modification events instead of Data events
                }
                _ => return None, // Ignore other metadata changes
            }
        }
        _ => return None,
    }

    // Check each path in the event
    for path in &event.paths {
        if let Some(parent) = path.parent() {
            let canonical_parent = parent
                .canonicalize()
                .unwrap_or_else(|_| parent.to_path_buf());
            if canonical_parent == logs_dir {
                if path.extension().and_then(|s| s.to_str()) == Some("toml") {
                    return Some(StorageEvent::LogChanged(path.clone()));
                }
            } else if canonical_parent == plans_dir {
                if path.extension().and_then(|s| s.to_str()) == Some("toml") {
                    return Some(StorageEvent::PlanChanged(path.clone()));
                }
            }
        }
    }

    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    // Write a file repeatedly until the watcher delivers a matching event, with an
    // overall 5s deadline. This avoids sleep-based synchronization: if FSEvents hasn't
    // activated the watch path yet on the first write, the retry catches it once it has.
    async fn write_until_event(
        file: &std::path::Path,
        content: &[u8],
        rx: &mut broadcast::Receiver<StorageEvent>,
        check: impl Fn(&StorageEvent) -> bool,
    ) -> StorageEvent {
        tokio::time::timeout(Duration::from_secs(5), async {
            loop {
                fs::write(file, content).unwrap();
                if let Ok(Ok(event)) =
                    tokio::time::timeout(Duration::from_millis(500), rx.recv()).await
                {
                    if check(&event) {
                        return event;
                    }
                }
            }
        })
        .await
        .expect("Timed out waiting for filesystem event")
    }

    #[tokio::test]
    async fn test_filesystem_watcher_detects_log_changes() {
        let temp_dir = TempDir::new().unwrap();
        let faff_dir = temp_dir.path().to_path_buf();

        let logs_dir = faff_dir.join("logs");
        fs::create_dir_all(&logs_dir).unwrap();

        let handle = spawn_filesystem_watcher(faff_dir);
        let mut rx = handle.subscribe();
        let log_file = logs_dir.join("2024-01-01.toml");

        let event = write_until_event(&log_file, b"test content", &mut rx, |e| {
            matches!(e, StorageEvent::LogChanged(_))
        })
        .await;

        match event {
            StorageEvent::LogChanged(path) => {
                assert_eq!(
                    std::fs::canonicalize(path).unwrap(),
                    std::fs::canonicalize(&log_file).unwrap()
                );
            }
            _ => unreachable!(),
        }
    }

    #[tokio::test]
    async fn test_filesystem_watcher_detects_plan_changes() {
        let temp_dir = TempDir::new().unwrap();
        let faff_dir = temp_dir.path().to_path_buf();

        let plans_dir = faff_dir.join("plans");
        fs::create_dir_all(&plans_dir).unwrap();

        let handle = spawn_filesystem_watcher(faff_dir);
        let mut rx = handle.subscribe();
        let plan_file = plans_dir.join("test.toml");

        let event = write_until_event(&plan_file, b"test content", &mut rx, |e| {
            matches!(e, StorageEvent::PlanChanged(_))
        })
        .await;

        match event {
            StorageEvent::PlanChanged(path) => {
                assert_eq!(
                    std::fs::canonicalize(path).unwrap(),
                    std::fs::canonicalize(&plan_file).unwrap()
                );
            }
            _ => unreachable!(),
        }
    }
}
