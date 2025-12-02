//! Event stream for watching changes to storage files.
//!
//! This module provides filesystem watching that emits semantic events when
//! log or plan files change. This is currently only implemented for
//! FileSystemStorage, but could be extended to other storage backends
//! that support change notifications.

use notify::{Config, Event, RecommendedWatcher, RecursiveMode, Watcher};
use std::path::{Path, PathBuf};
use std::sync::Arc;
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
/// The watcher continues running until all handles and receivers are dropped.
pub struct EventStreamHandle {
    sender: broadcast::Sender<StorageEvent>,
    // Keep a reference to prevent the sender from being dropped
    _handle: Arc<std::sync::Mutex<Option<std::thread::JoinHandle<()>>>>,
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

/// Spawn a filesystem watcher for the given base directory.
///
/// This is used internally by FileSystemStorage to implement event support.
/// The watcher monitors the `logs/` and `plans/` directories and emits
/// semantic events when files change.
///
/// Events are debounced by ~200ms and filtered to only include actual
/// content changes (not metadata-only changes).
pub(crate) fn spawn_filesystem_watcher(base_dir: PathBuf) -> EventStreamHandle {
    // Create broadcast channel with capacity for 100 events
    let (tx, _rx) = broadcast::channel(100);

    let tx_clone = tx.clone();

    // Canonicalize paths to resolve symlinks
    let logs_dir = base_dir
        .join("logs")
        .canonicalize()
        .unwrap_or_else(|_| base_dir.join("logs"));
    let plans_dir = base_dir
        .join("plans")
        .canonicalize()
        .unwrap_or_else(|_| base_dir.join("plans"));

    // Spawn watcher thread
    let handle = std::thread::spawn(move || {
        // Create a channel for receiving raw filesystem events
        let (event_tx, event_rx) = std::sync::mpsc::channel();

        // Create watcher
        let mut watcher = RecommendedWatcher::new(event_tx, Config::default())
            .expect("Failed to create filesystem watcher");

        // Watch logs and plans directories (non-recursive)
        if logs_dir.exists() {
            watcher
                .watch(&logs_dir, RecursiveMode::NonRecursive)
                .expect("Failed to watch logs directory");
        }

        if plans_dir.exists() {
            watcher
                .watch(&plans_dir, RecursiveMode::NonRecursive)
                .expect("Failed to watch plans directory");
        }

        // Process events with debouncing
        let mut last_event_time: std::collections::HashMap<PathBuf, std::time::Instant> =
            std::collections::HashMap::new();
        let debounce_duration = Duration::from_millis(200);

        loop {
            match event_rx.recv() {
                Ok(Ok(event)) => {
                    if let Some(storage_event) = process_event(&event, &logs_dir, &plans_dir) {
                        // Get the path for debouncing
                        let path = match &storage_event {
                            StorageEvent::LogChanged(p) | StorageEvent::PlanChanged(p) => {
                                p.clone()
                            }
                        };

                        // Check if we should emit this event (debounce)
                        let now = std::time::Instant::now();
                        let should_emit = if let Some(last_time) = last_event_time.get(&path) {
                            now.duration_since(*last_time) > debounce_duration
                        } else {
                            true
                        };

                        if should_emit {
                            last_event_time.insert(path, now);
                            // Send event (ignore if no receivers)
                            let _ = tx_clone.send(storage_event);
                        }
                    }
                }
                Ok(Err(e)) => {
                    eprintln!("[faff-core] Filesystem watcher error: {:?}", e);
                }
                Err(_) => {
                    // Channel closed, exit thread
                    break;
                }
            }
        }
    });

    EventStreamHandle {
        sender: tx,
        _handle: Arc::new(std::sync::Mutex::new(Some(handle))),
    }
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
        // Determine if this is a log or plan file
        if let Some(parent) = path.parent() {
            if parent == logs_dir {
                // Check if it's a .toml file
                if path.extension().and_then(|s| s.to_str()) == Some("toml") {
                    return Some(StorageEvent::LogChanged(path.clone()));
                }
            } else if parent == plans_dir {
                // Check if it's a .toml file
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

    #[tokio::test]
    async fn test_filesystem_watcher_detects_log_changes() {
        let temp_dir = TempDir::new().unwrap();
        let faff_dir = temp_dir.path().to_path_buf();

        // Create logs directory
        let logs_dir = faff_dir.join("logs");
        fs::create_dir_all(&logs_dir).unwrap();

        // Spawn watcher
        let handle = spawn_filesystem_watcher(faff_dir);
        let mut rx = handle.subscribe();

        // Give watcher time to start
        tokio::time::sleep(Duration::from_millis(100)).await;

        // Create a log file
        let log_file = logs_dir.join("2024-01-01.toml");
        fs::write(&log_file, "test content").unwrap();

        // Should receive LogChanged event
        let event = tokio::time::timeout(Duration::from_secs(2), rx.recv())
            .await
            .expect("Timeout waiting for event")
            .expect("Failed to receive event");

        match event {
            StorageEvent::LogChanged(path) => {
                assert_eq!(path, log_file);
            }
            _ => panic!("Expected LogChanged event"),
        }
    }

    #[tokio::test]
    async fn test_filesystem_watcher_detects_plan_changes() {
        let temp_dir = TempDir::new().unwrap();
        let faff_dir = temp_dir.path().to_path_buf();

        // Create plans directory
        let plans_dir = faff_dir.join("plans");
        fs::create_dir_all(&plans_dir).unwrap();

        // Spawn watcher
        let handle = spawn_filesystem_watcher(faff_dir);
        let mut rx = handle.subscribe();

        // Give watcher time to start
        tokio::time::sleep(Duration::from_millis(100)).await;

        // Create a plan file
        let plan_file = plans_dir.join("test.toml");
        fs::write(&plan_file, "test content").unwrap();

        // Should receive PlanChanged event
        let event = tokio::time::timeout(Duration::from_secs(2), rx.recv())
            .await
            .expect("Timeout waiting for event")
            .expect("Failed to receive event");

        match event {
            StorageEvent::PlanChanged(path) => {
                assert_eq!(path, plan_file);
            }
            _ => panic!("Expected PlanChanged event"),
        }
    }
}
