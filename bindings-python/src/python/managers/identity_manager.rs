use crate::python::storage::PyStorage;
use faff_core::managers::IdentityManager as RustIdentityManager;
use pyo3::prelude::*;
use pyo3::types::PyBytes;
use std::collections::HashMap;
use std::sync::Arc;

/// Python wrapper for IdentityManager
#[pyclass(name = "IdentityManager")]
#[derive(Clone)]
pub struct PyIdentityManager {
    manager: Arc<RustIdentityManager>,
}

#[pymethods]
impl PyIdentityManager {
    #[new]
    pub fn new(storage: Py<PyAny>) -> PyResult<Self> {
        let py_storage = PyStorage::new(storage);
        let manager = RustIdentityManager::new(Arc::new(py_storage));
        Ok(Self {
            manager: Arc::new(manager),
        })
    }

    /// Create a new Ed25519 identity keypair
    ///
    /// Args:
    ///     name: Identity name
    ///     overwrite: Whether to overwrite if identity already exists
    ///
    /// Returns:
    ///     The private signing key as bytes
    #[pyo3(signature = (name, overwrite=false))]
    pub fn create_identity<'py>(
        &self,
        py: Python<'py>,
        name: &str,
        overwrite: bool,
    ) -> PyResult<Bound<'py, PyBytes>> {
        let signing_key = self
            .manager
            .create_identity(name, overwrite)
            .map_err(|e| pyo3::exceptions::PyRuntimeError::new_err(e.to_string()))?;

        Ok(PyBytes::new(py, &signing_key.to_bytes()))
    }

    /// Get a specific identity by name
    ///
    /// Args:
    ///     name: Identity name
    ///
    /// Returns:
    ///     The private signing key as bytes, or None if not found
    pub fn get_identity<'py>(
        &self,
        py: Python<'py>,
        name: &str,
    ) -> PyResult<Option<Bound<'py, PyBytes>>> {
        let signing_key = self
            .manager
            .get_identity(name)
            .map_err(|e| pyo3::exceptions::PyRuntimeError::new_err(e.to_string()))?;

        Ok(signing_key.map(|key| PyBytes::new(py, &key.to_bytes())))
    }

    /// Get all identities
    ///
    /// Returns:
    ///     Dictionary mapping identity names to signing keys (as bytes)
    pub fn get<'py>(&self, py: Python<'py>) -> PyResult<HashMap<String, Bound<'py, PyBytes>>> {
        let identities = self
            .manager
            .get()
            .map_err(|e| pyo3::exceptions::PyRuntimeError::new_err(e.to_string()))?;

        let mut result = HashMap::new();
        for (name, key) in identities {
            result.insert(name, PyBytes::new(py, &key.to_bytes()));
        }

        Ok(result)
    }
}

impl PyIdentityManager {
    pub fn from_rust(manager: Arc<RustIdentityManager>) -> Self {
        Self { manager }
    }
}

pub fn register(m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_class::<PyIdentityManager>()?;
    Ok(())
}
