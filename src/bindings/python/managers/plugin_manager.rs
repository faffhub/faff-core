use crate::bindings::python::storage::PyStorage;
use crate::models::Config;
use crate::plugins::{
    AudiencePlugin as RustAudiencePlugin, PlanSourcePlugin as RustPlanSourcePlugin,
    PluginManager as RustPluginManager,
};
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
        use crate::storage::Storage;

        let py_storage = PyStorage::new(storage);
        let storage_arc: Arc<dyn Storage> = Arc::new(py_storage);

        // Read config from storage
        let config_path = storage_arc.config_file();
        let config_str = storage_arc.read_string(&config_path)
            .map_err(|e| pyo3::exceptions::PyRuntimeError::new_err(e.to_string()))?;
        let config = Config::from_toml(&config_str)
            .map_err(|e| pyo3::exceptions::PyRuntimeError::new_err(format!("Failed to parse config: {}", e)))?;

        let manager = RustPluginManager::new(storage_arc, config);
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

        let plugins = manager
            .load_plugins()
            .map_err(|e| pyo3::exceptions::PyRuntimeError::new_err(e.to_string()))?;

        let result = PyDict::new(py);
        for (name, plugin_class) in plugins.iter() {
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
            .map_err(|e| pyo3::exceptions::PyValueError::new_err(format!("Invalid config: {}", e)))?;

        let defaults_map: HashMap<String, toml::Value> = pythonize::depythonize(&defaults)
            .map_err(|e| pyo3::exceptions::PyValueError::new_err(format!("Invalid defaults: {}", e)))?;

        manager
            .instantiate_plugin(plugin_name, instance_name, config_map, defaults_map)
            .map_err(|e| pyo3::exceptions::PyRuntimeError::new_err(e.to_string()))
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

        manager
            .plan_remotes()
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

        manager
            .audiences()
            .map_err(|e| pyo3::exceptions::PyRuntimeError::new_err(e.to_string()))
    }
}

impl PyPluginManager {
    pub fn from_rust(manager: Arc<Mutex<RustPluginManager>>) -> Self {
        Self { manager }
    }
}

/// Python wrapper for PlanSourcePlugin
#[pyclass(name = "PlanSourcePlugin")]
pub struct PyPlanSourcePlugin {
    plugin: Arc<RustPlanSourcePlugin>,
}

#[pymethods]
impl PyPlanSourcePlugin {
    #[new]
    pub fn new(instance: Py<PyAny>) -> Self {
        Self {
            plugin: Arc::new(RustPlanSourcePlugin::new(instance)),
        }
    }

    /// Pull a plan for the given date
    ///
    /// Args:
    ///     date: The date to pull the plan for (Python date object)
    ///
    /// Returns:
    ///     A Plan object
    pub fn pull_plan<'py>(&self, py: Python<'py>, date: Bound<'_, PyAny>) -> PyResult<Py<PyAny>> {
        // Convert Python date to NaiveDate
        let date_str: String = date.call_method0("isoformat")?.extract()?;
        let naive_date = chrono::NaiveDate::parse_from_str(&date_str, "%Y-%m-%d")
            .map_err(|e| pyo3::exceptions::PyValueError::new_err(e.to_string()))?;

        let plan = self
            .plugin
            .pull_plan(naive_date)
            .map_err(|e| pyo3::exceptions::PyRuntimeError::new_err(e.to_string()))?;

        // Convert Plan to PyPlan
        use crate::bindings::python::models::plan::PyPlan;
        let pyplan = Py::new(py, PyPlan { inner: plan })?;
        Ok(pyplan.into())
    }
}

/// Python wrapper for AudiencePlugin
#[pyclass(name = "AudiencePlugin")]
pub struct PyAudiencePlugin {
    plugin: Arc<RustAudiencePlugin>,
}

#[pymethods]
impl PyAudiencePlugin {
    #[new]
    pub fn new(instance: Py<PyAny>) -> Self {
        Self {
            plugin: Arc::new(RustAudiencePlugin::new(instance)),
        }
    }

    /// Compile a timesheet for the given log
    ///
    /// Args:
    ///     log: The Log object to compile a timesheet from
    ///
    /// Returns:
    ///     A Timesheet object
    pub fn compile_timesheet<'py>(&self, py: Python<'py>, log: Py<PyAny>) -> PyResult<Py<PyAny>> {
        // Extract the Rust Log from PyLog
        use crate::bindings::python::models::log::PyLog;
        let pylog: PyRef<PyLog> = log.extract(py)?;
        let rust_log = &pylog.inner;

        let timesheet = self
            .plugin
            .compile_timesheet(rust_log)
            .map_err(|e| pyo3::exceptions::PyRuntimeError::new_err(e.to_string()))?;

        // Convert Timesheet to PyTimesheet
        use crate::bindings::python::models::timesheet::PyTimesheet;
        let pytimesheet = Py::new(py, PyTimesheet { inner: timesheet })?;
        Ok(pytimesheet.into())
    }

    /// Submit a timesheet
    ///
    /// Args:
    ///     timesheet: The Timesheet object to submit
    pub fn submit_timesheet(&self, py: Python<'_>, timesheet: Py<PyAny>) -> PyResult<()> {
        // Extract the Rust Timesheet from PyTimesheet
        use crate::bindings::python::models::timesheet::PyTimesheet;
        let pytimesheet: PyRef<PyTimesheet> = timesheet.extract(py)?;
        let rust_timesheet = &pytimesheet.inner;

        self.plugin
            .submit_timesheet(rust_timesheet)
            .map_err(|e| pyo3::exceptions::PyRuntimeError::new_err(e.to_string()))
    }
}

pub fn register(m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_class::<PyPluginManager>()?;
    m.add_class::<PyPlanSourcePlugin>()?;
    m.add_class::<PyAudiencePlugin>()?;
    Ok(())
}
