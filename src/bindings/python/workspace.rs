use crate::bindings::python::storage::PyStorage;
use crate::bindings::python::type_mapping::{date_rust_to_py, datetime_rust_to_py};
use crate::models::Config as RustConfig;
use crate::workspace::Workspace as RustWorkspace;
use pyo3::prelude::*;
use pyo3::types::{PyDate, PyDateTime};
use std::sync::Arc;

#[pyclass(name = "Workspace")]
pub struct PyWorkspace {
    inner: RustWorkspace,
}

pub fn register(m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_class::<PyWorkspace>()?;
    Ok(())
}

#[pymethods]
impl PyWorkspace {
    #[new]
    fn py_new(storage: Py<PyAny>) -> PyResult<Self> {
        let py_storage = PyStorage::new(storage);
        let inner = RustWorkspace::new(Arc::new(py_storage))
            .map_err(|e| pyo3::exceptions::PyRuntimeError::new_err(e.to_string()))?;
        Ok(Self { inner })
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

    fn __repr__(&self) -> String {
        format!("Workspace(timezone={})", self.inner.timezone().name())
    }
}
