use crate::bindings::python::managers::{
    identity_manager::PyIdentityManager, log_manager::PyLogManager,
    plan_manager::PyPlanManager, plugin_manager::PyPluginManager,
    timesheet_manager::PyTimesheetManager,
};
use crate::bindings::python::storage::PyStorage;
use crate::bindings::python::type_mapping::{date_rust_to_py, datetime_rust_to_py};
use crate::workspace::Workspace as RustWorkspace;
use pyo3::prelude::*;
use pyo3::types::{PyDate, PyDateTime};
use std::sync::Arc;

#[pyclass(name = "Workspace")]
pub struct PyWorkspace {
    inner: Arc<RustWorkspace>,
    // Cache the Python manager wrappers
    plans: PyPlanManager,
    logs: PyLogManager,
    timesheets: PyTimesheetManager,
    identities: PyIdentityManager,
    plugins: PyPluginManager,
}

pub fn register(m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_class::<PyWorkspace>()?;
    Ok(())
}

#[pymethods]
impl PyWorkspace {
    #[new]
    #[pyo3(signature = (storage=None))]
    fn py_new(storage: Option<Py<PyAny>>) -> PyResult<Self> {
        let inner = match storage {
            Some(storage_obj) => {
                let py_storage = PyStorage::new(storage_obj);
                RustWorkspace::with_storage(Arc::new(py_storage))
                    .map_err(|e| pyo3::exceptions::PyRuntimeError::new_err(e.to_string()))?
            }
            None => {
                RustWorkspace::new()
                    .map_err(|e| pyo3::exceptions::PyRuntimeError::new_err(e.to_string()))?
            }
        };

        // Wrap the workspace in Arc so we can share it with managers
        let inner_arc = Arc::new(inner);

        // Create Python manager wrappers from the Rust managers
        // Pass workspace reference to managers that need it
        let plans = PyPlanManager::from_rust_arc(inner_arc.plans(), inner_arc.clone());
        let logs = PyLogManager::from_rust((*inner_arc.logs()).clone(), inner_arc.clone());
        let timesheets = PyTimesheetManager::from_rust(inner_arc.timesheets(), inner_arc.clone());
        let identities = PyIdentityManager::from_rust(inner_arc.identities());
        let plugins = PyPluginManager::from_rust(inner_arc.plugins());

        Ok(Self {
            inner: inner_arc,
            plans,
            logs,
            timesheets,
            identities,
            plugins,
        })
    }

    /// Get the current time in the configured timezone
    fn now<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyDateTime>> {
        let now = self.inner.now();
        datetime_rust_to_py(py, &now)
    }

    /// Get today's date
    fn today<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyDate>> {
        let today = self.inner.today();
        date_rust_to_py(py, &today)
    }

    /// Get the configured timezone
    fn timezone<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        let zoneinfo = py.import("zoneinfo")?;
        let zone_info = zoneinfo.call_method1("ZoneInfo", (self.inner.timezone().name(),))?;
        Ok(zone_info)
    }

    /// Get the config
    fn config(&self) -> crate::bindings::python::models::config::PyConfig {
        crate::bindings::python::models::config::PyConfig {
            inner: self.inner.config().clone(),
        }
    }

    /// Get the PlanManager
    #[getter]
    fn plans(&self) -> PyPlanManager {
        self.plans.clone()
    }

    /// Get the LogManager
    #[getter]
    fn logs(&self) -> PyLogManager {
        self.logs.clone()
    }

    /// Get the TimesheetManager
    #[getter]
    fn timesheets(&self) -> PyTimesheetManager {
        self.timesheets.clone()
    }

    /// Get the IdentityManager
    #[getter]
    fn identities(&self) -> PyIdentityManager {
        self.identities.clone()
    }

    /// Get the PluginManager
    #[getter]
    fn plugins(&self) -> PyPluginManager {
        self.plugins.clone()
    }

    fn __repr__(&self) -> String {
        format!("Workspace(timezone={})", self.inner.timezone().name())
    }
}
