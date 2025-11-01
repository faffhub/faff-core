use crate::models::config::{Config as RustConfig, Role as RustRole};
use pyo3::prelude::*;
use pyo3::types::{PyDict, PyType};

#[pyclass(name = "Config")]
#[derive(Clone)]
pub struct PyConfig {
    pub inner: RustConfig,
}

#[pyclass(name = "Role")]
#[derive(Clone)]
pub struct PyRole {
    pub inner: RustRole,
}

pub fn register(m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_class::<PyConfig>()?;
    m.add_class::<PyRole>()?;
    Ok(())
}

#[pymethods]
impl PyConfig {
    #[classmethod]
    fn from_dict(_cls: &Bound<'_, PyType>, dict: &Bound<'_, PyDict>) -> PyResult<Self> {
        let inner: RustConfig = pythonize::depythonize(dict.as_any())
            .map_err(|e| pyo3::exceptions::PyValueError::new_err(e.to_string()))?;
        Ok(Self { inner })
    }

    #[getter]
    fn timezone<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        let zoneinfo = py.import("zoneinfo")?;
        let zone_info = zoneinfo.call_method1("ZoneInfo", (self.inner.timezone.name(),))?;
        Ok(zone_info)
    }

    #[getter]
    fn roles(&self) -> Vec<PyRole> {
        self.inner
            .role
            .iter()
            .map(|r| PyRole { inner: r.clone() })
            .collect()
    }

    fn __repr__(&self) -> String {
        format!(
            "Config(timezone={}, roles={})",
            self.inner.timezone.name(),
            self.inner.role.len()
        )
    }
}

#[pymethods]
impl PyRole {
    #[getter]
    fn name(&self) -> String {
        self.inner.name.clone()
    }

    #[getter]
    fn config(&self) -> Py<PyDict> {
        Python::attach(|py| {
            let py_obj = pythonize::pythonize(py, &self.inner.config)
                .expect("Failed to convert config to Python");
            py_obj.downcast::<PyDict>().unwrap().clone().unbind()
        })
    }

    fn __repr__(&self) -> String {
        format!("Role(name={})", self.inner.name)
    }
}
