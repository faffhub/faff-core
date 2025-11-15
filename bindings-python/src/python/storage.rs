use anyhow::{Context, Result};
use async_trait::async_trait;
use pyo3::prelude::*;
use pyo3::types::PyBytes;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use faff_core::storage::FileSystemStorage;
use faff_core::storage::Storage;

/// Python-to-Rust storage adapter: Wraps a Python storage object for use in Rust.
///
/// This implements the Rust `Storage` trait by delegating to a Python object.
/// Direction: Python → Rust
///
/// Use case: When Python code provides a custom storage implementation and passes it
/// to the Rust `Workspace` constructor, we wrap it in `PyStorage` so Rust code can
/// call Storage trait methods on it.
///
/// Example:
/// ```python
/// class MyCustomStorage:
///     def base_dir(self): return "/custom/path"
///     # ... other Storage methods
///
/// ws = Workspace(storage=MyCustomStorage())  # Wrapped in PyStorage internally
/// ```
pub struct PyStorage {
    py_obj: Py<PyAny>,
}

impl PyStorage {
    pub fn new(py_obj: Py<PyAny>) -> Self {
        Self { py_obj }
    }
}

#[async_trait]
impl Storage for PyStorage {
    fn base_dir(&self) -> PathBuf {
        Python::attach(|py| {
            let result = self
                .py_obj
                .call_method0(py, "base_dir")
                .expect("Failed to call base_dir");
            let path_str: String = result.extract(py).expect("base_dir must return str");
            PathBuf::from(path_str)
        })
    }

    async fn read_bytes(&self, path: &Path) -> Result<Vec<u8>> {
        Python::attach(|py| {
            let path_str = path.to_str().context("Path contains invalid UTF-8")?;
            let result = self
                .py_obj
                .call_method1(py, "read_bytes", (path_str,))
                .context("Failed to call read_bytes")?;
            let bytes = result
                .downcast_bound::<PyBytes>(py)
                .map_err(|e| anyhow::anyhow!("read_bytes must return bytes: {}", e))?;
            Ok(bytes.as_bytes().to_vec())
        })
    }

    async fn read_string(&self, path: &Path) -> Result<String> {
        Python::attach(|py| {
            let path_str = path.to_str().context("Path contains invalid UTF-8")?;
            let result = self
                .py_obj
                .call_method1(py, "read_string", (path_str,))
                .context("Failed to call read_string")?;
            result.extract(py).context("read_string must return str")
        })
    }

    async fn write_bytes(&self, path: &Path, data: &[u8]) -> Result<()> {
        Python::attach(|py| {
            let path_str = path.to_str().context("Path contains invalid UTF-8")?;
            let py_bytes = PyBytes::new(py, data);
            self.py_obj
                .call_method1(py, "write_bytes", (path_str, py_bytes))
                .context("Failed to call write_bytes")?;
            Ok(())
        })
    }

    async fn write_string(&self, path: &Path, data: &str) -> Result<()> {
        Python::attach(|py| {
            let path_str = path.to_str().context("Path contains invalid UTF-8")?;
            self.py_obj
                .call_method1(py, "write_string", (path_str, data))
                .context("Failed to call write_string")?;
            Ok(())
        })
    }

    async fn delete(&self, path: &Path) -> Result<()> {
        Python::attach(|py| {
            let path_str = path.to_str().context("Path contains invalid UTF-8")?;
            self.py_obj
                .call_method1(py, "delete", (path_str,))
                .context("Failed to call delete")?;
            Ok(())
        })
    }

    fn exists(&self, path: &Path) -> bool {
        Python::attach(|py| {
            let path_str = path.to_str().expect("Path contains invalid UTF-8");
            let result = self
                .py_obj
                .call_method1(py, "exists", (path_str,))
                .expect("Failed to call exists");
            result.extract(py).expect("exists must return bool")
        })
    }

    async fn create_dir_all(&self, path: &Path) -> Result<()> {
        Python::attach(|py| {
            let path_str = path.to_str().context("Path contains invalid UTF-8")?;
            self.py_obj
                .call_method1(py, "create_dir_all", (path_str,))
                .context("Failed to call create_dir_all")?;
            Ok(())
        })
    }

    async fn list_files(&self, dir: &Path, pattern: &str) -> Result<Vec<PathBuf>> {
        Python::attach(|py| {
            let dir_str = dir
                .to_str()
                .context("Directory path contains invalid UTF-8")?;
            let result = self
                .py_obj
                .call_method1(py, "list_files", (dir_str, pattern))
                .context("Failed to call list_files")?;
            let paths: Vec<String> = result
                .extract(py)
                .context("list_files must return list of str")?;
            Ok(paths.into_iter().map(PathBuf::from).collect())
        })
    }
}

/// Python wrapper for Rust's FileSystemStorage
///
/// This exposes the Rust FileSystemStorage implementation to Python,
/// allowing Python code to use the native Rust storage backend.
#[pyclass(name = "FileSystemStorage")]
#[derive(Clone)]
pub struct PyFileSystemStorage {
    storage: Arc<FileSystemStorage>,
}

#[pymethods]
impl PyFileSystemStorage {
    /// Create a new FileSystemStorage using FAFF_DIR or ~/.faff
    ///
    /// Uses $FAFF_DIR directly if set, otherwise uses ~/.faff (hidden directory in home)
    #[staticmethod]
    pub fn new() -> PyResult<Self> {
        let storage = FileSystemStorage::new()
            .map_err(|e| pyo3::exceptions::PyRuntimeError::new_err(e.to_string()))?;
        Ok(Self {
            storage: Arc::new(storage),
        })
    }

    /// Create a FileSystemStorage at a specific path (doesn't check if directory exists)
    ///
    /// This is useful for initialization where the directory doesn't exist yet.
    #[staticmethod]
    pub fn at_path(path: String) -> Self {
        let storage = FileSystemStorage::at_path(PathBuf::from(path));
        Self {
            storage: Arc::new(storage),
        }
    }

    /// Initialize a new faff repository at the given path
    ///
    /// Creates a FileSystemStorage at the path and initializes it with
    /// the standard faff structure and default config.
    ///
    /// Args:
    ///     path: The directory to use as the faff ledger.
    ///           If None, uses FAFF_DIR environment variable or ~/.faff.
    ///
    /// Returns:
    ///     A new FileSystemStorage instance for the initialized repository
    #[staticmethod]
    #[pyo3(signature = (path=None))]
    pub fn init_at(path: Option<String>) -> PyResult<Self> {
        let path_buf = path.map(PathBuf::from);
        let storage = tokio::runtime::Runtime::new()
            .unwrap()
            .block_on(FileSystemStorage::init_at(path_buf))
            .map_err(|e| pyo3::exceptions::PyRuntimeError::new_err(e.to_string()))?;
        Ok(Self {
            storage: Arc::new(storage),
        })
    }

    /// Get the base directory (the faff ledger directory)
    ///
    /// This is either ~/.faff (default) or the directory specified by FAFF_DIR.
    pub fn base_dir(&self) -> String {
        self.storage.base_dir().to_string_lossy().into_owned()
    }

    /// Get the log directory
    pub fn log_dir(&self) -> String {
        self.storage.log_dir().to_string_lossy().into_owned()
    }

    /// Get the plan directory
    pub fn plan_dir(&self) -> String {
        self.storage.plan_dir().to_string_lossy().into_owned()
    }

    /// Get the identity directory
    pub fn identity_dir(&self) -> String {
        self.storage.identity_dir().to_string_lossy().into_owned()
    }

    /// Get the timesheet directory
    pub fn timesheet_dir(&self) -> String {
        self.storage.timesheet_dir().to_string_lossy().into_owned()
    }

    /// Get the remotes directory
    pub fn remotes_dir(&self) -> String {
        self.storage.remotes_dir().to_string_lossy().into_owned()
    }

    /// Get the config file path
    pub fn config_file(&self) -> String {
        self.storage.config_file().to_string_lossy().into_owned()
    }

    /// Read file as bytes
    pub fn read_bytes<'py>(&self, py: Python<'py>, path: String) -> PyResult<Bound<'py, PyBytes>> {
        let bytes = tokio::runtime::Runtime::new()
            .unwrap()
            .block_on(self.storage.read_bytes(&PathBuf::from(path)))
            .map_err(|e| pyo3::exceptions::PyRuntimeError::new_err(e.to_string()))?;
        Ok(PyBytes::new(py, &bytes))
    }

    /// Read file as string
    pub fn read_string(&self, path: String) -> PyResult<String> {
        tokio::runtime::Runtime::new()
            .unwrap()
            .block_on(self.storage.read_string(&PathBuf::from(path)))
            .map_err(|e| pyo3::exceptions::PyRuntimeError::new_err(e.to_string()))
    }

    /// Write bytes to file
    pub fn write_bytes(&self, path: String, data: Vec<u8>) -> PyResult<()> {
        tokio::runtime::Runtime::new()
            .unwrap()
            .block_on(self.storage.write_bytes(&PathBuf::from(path), &data))
            .map_err(|e| pyo3::exceptions::PyRuntimeError::new_err(e.to_string()))
    }

    /// Write string to file
    pub fn write_string(&self, path: String, data: String) -> PyResult<()> {
        tokio::runtime::Runtime::new()
            .unwrap()
            .block_on(self.storage.write_string(&PathBuf::from(path), &data))
            .map_err(|e| pyo3::exceptions::PyRuntimeError::new_err(e.to_string()))
    }

    /// Delete a file
    pub fn delete(&self, path: String) -> PyResult<()> {
        tokio::runtime::Runtime::new()
            .unwrap()
            .block_on(self.storage.delete(&PathBuf::from(path)))
            .map_err(|e| pyo3::exceptions::PyRuntimeError::new_err(e.to_string()))
    }

    /// Check if a file exists
    pub fn exists(&self, path: String) -> bool {
        self.storage.exists(&PathBuf::from(path))
    }

    /// Create directory and all parent directories
    pub fn create_dir_all(&self, path: String) -> PyResult<()> {
        tokio::runtime::Runtime::new()
            .unwrap()
            .block_on(self.storage.create_dir_all(&PathBuf::from(path)))
            .map_err(|e| pyo3::exceptions::PyRuntimeError::new_err(e.to_string()))
    }

    /// List files matching a pattern
    pub fn list_files(&self, dir: String, pattern: String) -> PyResult<Vec<String>> {
        let paths = tokio::runtime::Runtime::new()
            .unwrap()
            .block_on(self.storage.list_files(&PathBuf::from(dir), &pattern))
            .map_err(|e| pyo3::exceptions::PyRuntimeError::new_err(e.to_string()))?;
        Ok(paths
            .into_iter()
            .map(|p| p.to_string_lossy().into_owned())
            .collect())
    }
}

impl PyFileSystemStorage {
    /// Get the underlying Arc<FileSystemStorage> for use in Rust code
    pub fn storage(&self) -> Arc<dyn Storage> {
        self.storage.clone()
    }
}

/// Rust-to-Python storage adapter: Exposes a Rust Storage trait object to Python.
///
/// This wraps `Arc<dyn Storage>` as a Python class, exposing Storage trait methods.
/// Direction: Rust → Python
///
/// Use case: When Python code needs to access the storage object from a Workspace
/// (e.g., to get directory paths for utilities), we wrap the Rust storage in
/// `PyStorageWrapper` so Python can call methods on it.
///
/// Unlike `PyFileSystemStorage` which wraps a concrete type, this works with any
/// Storage implementation via the trait interface.
///
/// Example:
/// ```python
/// ws = Workspace()
/// storage = ws.storage()  # Returns PyStorageWrapper
/// plan_dir = storage.plan_dir()  # Calls trait method on wrapped Rust object
/// ```
#[pyclass(name = "Storage")]
#[derive(Clone)]
pub struct PyStorageWrapper {
    storage: Arc<dyn Storage>,
}

#[pymethods]
impl PyStorageWrapper {
    /// Get the base directory (the faff ledger directory)
    ///
    /// This is either ~/.faff (default) or the directory specified by FAFF_DIR.
    pub fn base_dir(&self) -> String {
        self.storage.base_dir().to_string_lossy().into_owned()
    }

    /// Get the log directory
    pub fn log_dir(&self) -> String {
        self.storage.log_dir().to_string_lossy().into_owned()
    }

    /// Get the plan directory
    pub fn plan_dir(&self) -> String {
        self.storage.plan_dir().to_string_lossy().into_owned()
    }

    /// Get the identity directory
    pub fn identity_dir(&self) -> String {
        self.storage.identity_dir().to_string_lossy().into_owned()
    }

    /// Get the timesheet directory
    pub fn timesheet_dir(&self) -> String {
        self.storage.timesheet_dir().to_string_lossy().into_owned()
    }

    /// Get the remotes directory
    pub fn remotes_dir(&self) -> String {
        self.storage.remotes_dir().to_string_lossy().into_owned()
    }

    /// Get the config file path
    pub fn config_file(&self) -> String {
        self.storage.config_file().to_string_lossy().into_owned()
    }

    /// Check if a file exists
    pub fn exists(&self, path: String) -> bool {
        self.storage.exists(&PathBuf::from(path))
    }

    /// List files matching a pattern
    pub fn list_files(&self, dir: String, pattern: String) -> PyResult<Vec<String>> {
        let paths = tokio::runtime::Runtime::new()
            .unwrap()
            .block_on(self.storage.list_files(&PathBuf::from(dir), &pattern))
            .map_err(|e| pyo3::exceptions::PyRuntimeError::new_err(e.to_string()))?;
        Ok(paths
            .into_iter()
            .map(|p| p.to_string_lossy().into_owned())
            .collect())
    }
}

impl PyStorageWrapper {
    /// Create a new wrapper from an Arc<dyn Storage>
    pub fn new(storage: Arc<dyn Storage>) -> Self {
        Self { storage }
    }
}

pub fn register(m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_class::<PyFileSystemStorage>()?;
    m.add_class::<PyStorageWrapper>()?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_trait_object_storage() {
        // This test just verifies that PyStorage implements Storage
        // and can be used as a trait object
        fn _accepts_storage(_storage: &dyn Storage) {}
    }
}
