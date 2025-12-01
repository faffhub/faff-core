//! Python bindings for faff-core event stream.

use faff_core::storage::StorageEvent;
use pyo3::exceptions::PyStopIteration;
use pyo3::prelude::*;
use std::path::PathBuf;

/// Python-facing event type.
#[pyclass(name = "FaffEvent")]
#[derive(Clone)]
pub struct PyFaffEvent {
    #[pyo3(get)]
    event_type: String,
    #[pyo3(get)]
    path: String,
}

impl From<StorageEvent> for PyFaffEvent {
    fn from(event: StorageEvent) -> Self {
        match event {
            StorageEvent::LogChanged(path) => PyFaffEvent {
                event_type: "log_changed".to_string(),
                path: path.to_string_lossy().to_string(),
            },
            StorageEvent::PlanChanged(path) => PyFaffEvent {
                event_type: "plan_changed".to_string(),
                path: path.to_string_lossy().to_string(),
            },
        }
    }
}

#[pymethods]
impl PyFaffEvent {
    fn __repr__(&self) -> String {
        format!("FaffEvent(type='{}', path='{}')", self.event_type, self.path)
    }

    fn __str__(&self) -> String {
        self.__repr__()
    }
}

/// Python-facing event stream iterator.
///
/// This type implements Python's iterator protocol and yields FaffEvent objects
/// when log or plan files change in the watched directory.
#[pyclass(name = "EventStream")]
pub struct PyEventStream {
    rx: tokio::sync::broadcast::Receiver<StorageEvent>,
    runtime: std::sync::Arc<tokio::runtime::Runtime>,
}

#[pymethods]
impl PyEventStream {
    /// Make this object iterable in Python.
    fn __iter__(slf: PyRef<'_, Self>) -> PyRef<'_, Self> {
        slf
    }

    /// Return the next event from the stream.
    ///
    /// This blocks until an event is available. If the stream is closed,
    /// raises StopIteration. If events are lagged (too many events buffered),
    /// skips the lagged events and continues.
    fn __next__(&mut self, py: Python<'_>) -> PyResult<PyFaffEvent> {
        // Release GIL and poll with timeout to allow Ctrl+C
        loop {
            let result = py.allow_threads(|| {
                self.runtime.block_on(async {
                    tokio::time::timeout(
                        std::time::Duration::from_millis(100),
                        self.rx.recv()
                    ).await
                })
            });

            match result {
                Ok(Ok(event)) => return Ok(PyFaffEvent::from(event)),
                Ok(Err(tokio::sync::broadcast::error::RecvError::Closed)) => {
                    return Err(PyStopIteration::new_err("Event stream closed"));
                }
                Ok(Err(tokio::sync::broadcast::error::RecvError::Lagged(_))) => {
                    return Err(PyStopIteration::new_err(
                        "Event stream lagged (too slow consuming events)",
                    ));
                }
                Err(_timeout) => {
                    // Check for Python interrupts (Ctrl+C)
                    py.check_signals()?;
                    // Continue loop to wait for more events
                    continue;
                }
            }
        }
    }

    /// Close the event stream.
    fn close(&mut self) {
        // Closing is implicit when the receiver is dropped
        // This method exists for explicit cleanup if needed
    }
}

/// Start watching a faff repository for changes.
///
/// Args:
///     path: Path to the faff repository root (e.g., "~/.faff")
///
/// Returns:
///     EventStream: An iterator that yields FaffEvent objects
///
/// Example:
///     >>> from faff_core import start_watching
///     >>> stream = start_watching("~/.faff")
///     >>> for event in stream:
///     ...     print(f"Got event: {event}")
#[pyfunction]
pub fn start_watching(path: String) -> PyResult<PyEventStream> {
    use faff_core::storage::{FileSystemStorage, Storage};

    // Expand home directory if needed
    let expanded_path = if path.starts_with('~') {
        if let Some(home) = dirs::home_dir() {
            home.join(path.trim_start_matches("~/"))
        } else {
            PathBuf::from(path)
        }
    } else {
        PathBuf::from(path)
    };

    // Create a tokio runtime for async operations
    let runtime = tokio::runtime::Runtime::new()
        .map_err(|e| pyo3::exceptions::PyRuntimeError::new_err(format!("Failed to create runtime: {}", e)))?;

    // Create a FileSystemStorage and spawn its event stream
    let storage = FileSystemStorage::at_path(expanded_path);
    let handle = storage.spawn_event_stream()
        .ok_or_else(|| pyo3::exceptions::PyRuntimeError::new_err("Event streams not supported"))?;
    let rx = handle.subscribe();

    Ok(PyEventStream {
        rx,
        runtime: std::sync::Arc::new(runtime),
    })
}

/// Register the events module with Python.
pub(crate) fn register_module(parent_module: &Bound<'_, PyModule>) -> PyResult<()> {
    parent_module.add_function(wrap_pyfunction!(start_watching, parent_module)?)?;
    parent_module.add_class::<PyFaffEvent>()?;
    parent_module.add_class::<PyEventStream>()?;
    Ok(())
}
