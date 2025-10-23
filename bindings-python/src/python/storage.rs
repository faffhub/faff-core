use anyhow::{Context, Result};
use pyo3::prelude::*;
use pyo3::types::PyBytes;
use std::path::{Path, PathBuf};

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
    fn root_dir(&self) -> PathBuf {
        Python::attach(|py| {
            let result = self
                .py_obj
                .call_method0(py, "root_dir")
                .expect("Failed to call root_dir");
            let path_str: String = result.extract(py).expect("root_dir must return str");
            PathBuf::from(path_str)
        })
    }

    fn log_dir(&self) -> PathBuf {
        Python::attach(|py| {
            let result = self
                .py_obj
                .call_method0(py, "log_dir")
                .expect("Failed to call log_dir");
            let path_str: String = result.extract(py).expect("log_dir must return str");
            PathBuf::from(path_str)
        })
    }

    fn plan_dir(&self) -> PathBuf {
        Python::attach(|py| {
            let result = self
                .py_obj
                .call_method0(py, "plan_dir")
                .expect("Failed to call plan_dir");
            let path_str: String = result.extract(py).expect("plan_dir must return str");
            PathBuf::from(path_str)
        })
    }

    fn identity_dir(&self) -> PathBuf {
        Python::attach(|py| {
            let result = self
                .py_obj
                .call_method0(py, "identity_dir")
                .expect("Failed to call identity_dir");
            let path_str: String = result.extract(py).expect("identity_dir must return str");
            PathBuf::from(path_str)
        })
    }

    fn timesheet_dir(&self) -> PathBuf {
        Python::attach(|py| {
            let result = self
                .py_obj
                .call_method0(py, "timesheet_dir")
                .expect("Failed to call timesheet_dir");
            let path_str: String = result.extract(py).expect("timesheet_dir must return str");
            PathBuf::from(path_str)
        })
    }

    fn config_file(&self) -> PathBuf {
        Python::attach(|py| {
            let result = self
                .py_obj
                .call_method0(py, "config_file")
                .expect("Failed to call config_file");
            let path_str: String = result.extract(py).expect("config_file must return str");
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
