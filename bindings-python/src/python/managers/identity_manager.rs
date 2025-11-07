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
    ///     Dictionary with 'signing_key' and 'verifying_key' as bytes
    #[pyo3(signature = (name, overwrite=false))]
    pub fn create_identity<'py>(
        &self,
        py: Python<'py>,
        name: &str,
        overwrite: bool,
    ) -> PyResult<HashMap<String, Bound<'py, PyBytes>>> {
        let signing_key = tokio::runtime::Runtime::new()
            .unwrap()
            .block_on(self.manager.create_identity(name, overwrite))
            .map_err(|e| pyo3::exceptions::PyRuntimeError::new_err(e.to_string()))?;

        let mut result = HashMap::new();
        result.insert(
            "signing_key".to_string(),
            PyBytes::new(py, &signing_key.to_bytes()),
        );
        result.insert(
            "verifying_key".to_string(),
            PyBytes::new(py, signing_key.verifying_key().as_bytes()),
        );

        Ok(result)
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
        let signing_key = tokio::runtime::Runtime::new()
            .unwrap()
            .block_on(self.manager.get_identity(name))
            .map_err(|e| pyo3::exceptions::PyRuntimeError::new_err(e.to_string()))?;

        Ok(signing_key.map(|key| PyBytes::new(py, &key.to_bytes())))
    }

    /// List all identities
    ///
    /// Returns:
    ///     Dictionary mapping identity names to signing keys (as bytes)
    pub fn list_identities<'py>(
        &self,
        py: Python<'py>,
    ) -> PyResult<HashMap<String, Bound<'py, PyBytes>>> {
        let identities = tokio::runtime::Runtime::new()
            .unwrap()
            .block_on(self.manager.list_identities())
            .map_err(|e| pyo3::exceptions::PyRuntimeError::new_err(e.to_string()))?;

        let mut result = HashMap::new();
        for (name, key) in identities {
            result.insert(name, PyBytes::new(py, &key.to_bytes()));
        }

        Ok(result)
    }

    /// Check if an identity exists
    ///
    /// Args:
    ///     name: Identity name
    ///
    /// Returns:
    ///     True if the identity exists, False otherwise
    pub fn identity_exists(&self, name: &str) -> bool {
        self.manager.identity_exists(name)
    }

    /// Delete an identity
    ///
    /// Args:
    ///     name: Identity name
    pub fn delete_identity(&self, name: &str) -> PyResult<()> {
        tokio::runtime::Runtime::new()
            .unwrap()
            .block_on(self.manager.delete_identity(name))
            .map_err(|e| pyo3::exceptions::PyRuntimeError::new_err(e.to_string()))
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
