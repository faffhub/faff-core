use crate::python::storage::PyStorage;
use faff_core::managers::PluginManager as RustPluginManager;
use pyo3::prelude::*;
use pyo3::types::PyDict;
use std::collections::HashMap;
use std::sync::{Arc, Mutex};

/// Python wrapper for PluginManager
#[pyclass(name = "PluginManager")]
#[derive(Clone)]
pub struct PyPluginManager {
    manager: Arc<Mutex<RustPluginManager>>,
}

#[pymethods]
impl PyPluginManager {
    #[new]
    pub fn new(storage: Py<PyAny>) -> PyResult<Self> {
        use faff_core::storage::Storage;

        let py_storage = PyStorage::new(storage);
        let storage_arc: Arc<dyn Storage> = Arc::new(py_storage);

        let manager = RustPluginManager::new(storage_arc);
        Ok(Self {
            manager: Arc::new(Mutex::new(manager)),
        })
    }

    /// Load all available plugins from the plugins directory
    ///
    /// Returns:
    ///     Dictionary of plugin_name -> plugin_class
    pub fn load_plugins<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyDict>> {
        let mut manager = self
            .manager
            .lock()
            .map_err(|e| pyo3::exceptions::PyRuntimeError::new_err(e.to_string()))?;

        // Ensure plugins are loaded
        tokio::runtime::Runtime::new()
            .unwrap()
            .block_on(manager.load_plugins())
            .map_err(|e| pyo3::exceptions::PyRuntimeError::new_err(e.to_string()))?;

        // Access the cache to get the plugins
        let cache = manager.plugins_cache.lock().unwrap();
        let plugins = cache
            .as_ref()
            .ok_or_else(|| pyo3::exceptions::PyRuntimeError::new_err("Plugins not loaded"))?;

        let result = PyDict::new(py);
        for (name, (_path, plugin_class)) in plugins.iter() {
            result.set_item(name, plugin_class.clone_ref(py))?;
        }
        Ok(result)
    }

    /// Instantiate a plugin with the given config
    ///
    /// Args:
    ///     plugin_name: Name of the plugin to instantiate
    ///     instance_name: Name for this instance
    ///     config: Plugin-specific configuration
    ///     defaults: Default configuration values
    ///
    /// Returns:
    ///     The instantiated plugin object
    pub fn instantiate_plugin(
        &self,
        plugin_name: &str,
        instance_name: &str,
        config: Bound<'_, PyDict>,
        defaults: Bound<'_, PyDict>,
    ) -> PyResult<Py<PyAny>> {
        let mut manager = self
            .manager
            .lock()
            .map_err(|e| pyo3::exceptions::PyRuntimeError::new_err(e.to_string()))?;

        // Convert Python dicts to HashMap<String, toml::Value>
        let config_map: HashMap<String, toml::Value> = pythonize::depythonize(&config)
            .map_err(|e| pyo3::exceptions::PyValueError::new_err(format!("Invalid config: {e}")))?;

        let defaults_map: HashMap<String, toml::Value> = pythonize::depythonize(&defaults)
            .map_err(|e| {
                pyo3::exceptions::PyValueError::new_err(format!("Invalid defaults: {e}"))
            })?;

        tokio::runtime::Runtime::new()
            .unwrap()
            .block_on(manager.instantiate_plugin(
                plugin_name,
                instance_name,
                config_map,
                defaults_map,
            ))
            .map_err(|e| pyo3::exceptions::PyRuntimeError::new_err(e.to_string()))
    }

    /// Get remote configurations
    ///
    /// Returns the raw Remote configuration objects for all configured remotes.
    /// Use this to access remote metadata like connection settings, vocabulary, etc.
    ///
    /// Returns:
    ///     List of remote configuration dictionaries
    pub fn get_remote_configs<'py>(&self, py: Python<'py>) -> PyResult<Vec<Bound<'py, PyAny>>> {
        let manager = self
            .manager
            .lock()
            .map_err(|e| pyo3::exceptions::PyRuntimeError::new_err(e.to_string()))?;

        let configs = tokio::runtime::Runtime::new()
            .unwrap()
            .block_on(manager.get_remote_configs())
            .map_err(|e| pyo3::exceptions::PyRuntimeError::new_err(e.to_string()))?;

        // Convert Remote structs to Python objects using pythonize
        let mut result = Vec::new();
        for config in configs {
            let py_obj = pythonize::pythonize(py, &config)
                .map_err(|e| pyo3::exceptions::PyRuntimeError::new_err(e.to_string()))?;
            result.push(py_obj);
        }

        Ok(result)
    }

    /// Get instantiated plan remote plugins based on config
    ///
    /// Returns:
    ///     List of plan remote plugin instances
    pub fn plan_remotes(&self, _py: Python<'_>) -> PyResult<Vec<Py<PyAny>>> {
        let mut manager = self
            .manager
            .lock()
            .map_err(|e| pyo3::exceptions::PyRuntimeError::new_err(e.to_string()))?;

        tokio::runtime::Runtime::new()
            .unwrap()
            .block_on(manager.plan_remotes())
            .map_err(|e| pyo3::exceptions::PyRuntimeError::new_err(e.to_string()))
    }

    /// Get instantiated audience plugins based on config
    ///
    /// Returns:
    ///     List of audience plugin instances
    pub fn audiences(&self, _py: Python<'_>) -> PyResult<Vec<Py<PyAny>>> {
        let mut manager = self
            .manager
            .lock()
            .map_err(|e| pyo3::exceptions::PyRuntimeError::new_err(e.to_string()))?;

        tokio::runtime::Runtime::new()
            .unwrap()
            .block_on(manager.audiences())
            .map_err(|e| pyo3::exceptions::PyRuntimeError::new_err(e.to_string()))
    }

    /// Get a specific audience plugin by ID
    ///
    /// Args:
    ///     audience_id: The ID of the audience to find
    ///
    /// Returns:
    ///     The audience plugin instance, or None if not found
    pub fn get_audience_by_id(
        &self,
        _py: Python<'_>,
        audience_id: &str,
    ) -> PyResult<Option<Py<PyAny>>> {
        let mut manager = self
            .manager
            .lock()
            .map_err(|e| pyo3::exceptions::PyRuntimeError::new_err(e.to_string()))?;

        tokio::runtime::Runtime::new()
            .unwrap()
            .block_on(manager.get_audience_by_id(audience_id))
            .map_err(|e| pyo3::exceptions::PyRuntimeError::new_err(e.to_string()))
    }
}

impl PyPluginManager {
    pub fn from_rust(manager: Arc<Mutex<RustPluginManager>>) -> Self {
        Self { manager }
    }
}

pub fn register(m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_class::<PyPluginManager>()?;
    Ok(())
}
