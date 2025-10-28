use anyhow::{Context, Result};
use pyo3::prelude::*;
use pyo3::types::PyBytes;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use faff_core::file_system_storage::FileSystemStorage;
use faff_core::storage::Storage;

/// Python wrapper that implements the Storage trait by delegating to a Python object.
///
/// This allows Python code to provide storage implementations (e.g., FileSystem)
/// that Rust code can use through the Storage trait.
pub struct PyStorage {
    py_obj: Py<PyAny>,
}

impl PyStorage {
    pub fn new(py_obj: Py<PyAny>) -> Self {
        Self { py_obj }
    }
}

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

    fn read_bytes(&self, path: &Path) -> Result<Vec<u8>> {
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

    fn read_string(&self, path: &Path) -> Result<String> {
        Python::attach(|py| {
            let path_str = path.to_str().context("Path contains invalid UTF-8")?;
            let result = self
                .py_obj
                .call_method1(py, "read_string", (path_str,))
                .context("Failed to call read_string")?;
            result.extract(py).context("read_string must return str")
        })
    }

    fn write_bytes(&self, path: &Path, data: &[u8]) -> Result<()> {
        Python::attach(|py| {
            let path_str = path.to_str().context("Path contains invalid UTF-8")?;
            let py_bytes = PyBytes::new(py, data);
            self.py_obj
                .call_method1(py, "write_bytes", (path_str, py_bytes))
                .context("Failed to call write_bytes")?;
            Ok(())
        })
    }

    fn write_string(&self, path: &Path, data: &str) -> Result<()> {
        Python::attach(|py| {
            let path_str = path.to_str().context("Path contains invalid UTF-8")?;
            self.py_obj
                .call_method1(py, "write_string", (path_str, data))
                .context("Failed to call write_string")?;
            Ok(())
        })
    }

    fn delete(&self, path: &Path) -> Result<()> {
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

    fn create_dir_all(&self, path: &Path) -> Result<()> {
        Python::attach(|py| {
            let path_str = path.to_str().context("Path contains invalid UTF-8")?;
            self.py_obj
                .call_method1(py, "create_dir_all", (path_str,))
                .context("Failed to call create_dir_all")?;
            Ok(())
        })
    }

    fn list_files(&self, dir: &Path, pattern: &str) -> Result<Vec<PathBuf>> {
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
    /// Create a new FileSystemStorage by searching for .faff directory
    ///
    /// Starts from the current working directory and searches upward.
    #[staticmethod]
    pub fn new() -> PyResult<Self> {
        let storage = FileSystemStorage::new()
            .map_err(|e| pyo3::exceptions::PyRuntimeError::new_err(e.to_string()))?;
        Ok(Self {
            storage: Arc::new(storage),
        })
    }

    /// Create a new FileSystemStorage by searching for .faff directory starting from a specific path
    #[staticmethod]
    pub fn from_path(path: String) -> PyResult<Self> {
        let storage = FileSystemStorage::from_path(PathBuf::from(path))
            .map_err(|e| pyo3::exceptions::PyRuntimeError::new_err(e.to_string()))?;
        Ok(Self {
            storage: Arc::new(storage),
        })
    }

    /// Create a FileSystemStorage at a specific path (doesn't check if .faff exists)
    ///
    /// This is useful for initialization where .faff doesn't exist yet.
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
    ///     path: The directory path where .faff should be created
    ///     force: If True, override existing .faff or parent .faff checks
    ///
    /// Returns:
    ///     A new FileSystemStorage instance for the initialized repository
    #[staticmethod]
    #[pyo3(signature = (path, force=false))]
    pub fn init_at(path: String, force: bool) -> PyResult<Self> {
        let storage = FileSystemStorage::init_at(PathBuf::from(path), force)
            .map_err(|e| pyo3::exceptions::PyRuntimeError::new_err(e.to_string()))?;
        Ok(Self {
            storage: Arc::new(storage),
        })
    }

    /// Get the root directory (parent of .faff)
    pub fn root_dir(&self) -> String {
        self.storage
            .root_dir()
            .to_string_lossy()
            .into_owned()
    }

    /// Get the base directory (.faff directory)
    pub fn base_dir(&self) -> String {
        self.storage
            .base_dir()
            .to_string_lossy()
            .into_owned()
    }

    /// Get the log directory
    pub fn log_dir(&self) -> String {
        self.storage
            .log_dir()
            .to_string_lossy()
            .into_owned()
    }

    /// Get the plan directory
    pub fn plan_dir(&self) -> String {
        self.storage
            .plan_dir()
            .to_string_lossy()
            .into_owned()
    }

    /// Get the identity directory
    pub fn identity_dir(&self) -> String {
        self.storage
            .identity_dir()
            .to_string_lossy()
            .into_owned()
    }

    /// Get the timesheet directory
    pub fn timesheet_dir(&self) -> String {
        self.storage
            .timesheet_dir()
            .to_string_lossy()
            .into_owned()
    }

    /// Get the config file path
    pub fn config_file(&self) -> String {
        self.storage
            .config_file()
            .to_string_lossy()
            .into_owned()
    }

    /// Read file as bytes
    pub fn read_bytes<'py>(&self, py: Python<'py>, path: String) -> PyResult<Bound<'py, PyBytes>> {
        let bytes = self.storage
            .read_bytes(&PathBuf::from(path))
            .map_err(|e| pyo3::exceptions::PyRuntimeError::new_err(e.to_string()))?;
        Ok(PyBytes::new(py, &bytes))
    }

    /// Read file as string
    pub fn read_string(&self, path: String) -> PyResult<String> {
        self.storage
            .read_string(&PathBuf::from(path))
            .map_err(|e| pyo3::exceptions::PyRuntimeError::new_err(e.to_string()))
    }

    /// Write bytes to file
    pub fn write_bytes(&self, path: String, data: Vec<u8>) -> PyResult<()> {
        self.storage
            .write_bytes(&PathBuf::from(path), &data)
            .map_err(|e| pyo3::exceptions::PyRuntimeError::new_err(e.to_string()))
    }

    /// Write string to file
    pub fn write_string(&self, path: String, data: String) -> PyResult<()> {
        self.storage
            .write_string(&PathBuf::from(path), &data)
            .map_err(|e| pyo3::exceptions::PyRuntimeError::new_err(e.to_string()))
    }

    /// Delete a file
    pub fn delete(&self, path: String) -> PyResult<()> {
        self.storage
            .delete(&PathBuf::from(path))
            .map_err(|e| pyo3::exceptions::PyRuntimeError::new_err(e.to_string()))
    }

    /// Check if a file exists
    pub fn exists(&self, path: String) -> bool {
        self.storage.exists(&PathBuf::from(path))
    }

    /// Create directory and all parent directories
    pub fn create_dir_all(&self, path: String) -> PyResult<()> {
        self.storage
            .create_dir_all(&PathBuf::from(path))
            .map_err(|e| pyo3::exceptions::PyRuntimeError::new_err(e.to_string()))
    }

    /// List files matching a pattern
    pub fn list_files(&self, dir: String, pattern: String) -> PyResult<Vec<String>> {
        let paths = self.storage
            .list_files(&PathBuf::from(dir), &pattern)
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

pub fn register(m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_class::<PyFileSystemStorage>()?;
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
