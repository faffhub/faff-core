use crate::models::config::{
    Config as RustConfig, PlanDefaults as RustPlanDefaults, PlanRemote as RustPlanRemote,
    Role as RustRole, TimesheetAudience as RustTimesheetAudience,
};
use pyo3::prelude::*;
use pyo3::types::{PyDict, PyType};

#[pyclass(name = "Config")]
#[derive(Clone)]
pub struct PyConfig {
    pub inner: RustConfig,
}

#[pyclass(name = "PlanRemote")]
#[derive(Clone)]
pub struct PyPlanRemote {
    pub inner: RustPlanRemote,
}

#[pyclass(name = "PlanDefaults")]
#[derive(Clone)]
pub struct PyPlanDefaults {
    pub inner: RustPlanDefaults,
}

#[pyclass(name = "TimesheetAudience")]
#[derive(Clone)]
pub struct PyTimesheetAudience {
    pub inner: RustTimesheetAudience,
}

#[pyclass(name = "Role")]
#[derive(Clone)]
pub struct PyRole {
    pub inner: RustRole,
}

pub fn register(m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_class::<PyConfig>()?;
    m.add_class::<PyPlanRemote>()?;
    m.add_class::<PyPlanDefaults>()?;
    m.add_class::<PyTimesheetAudience>()?;
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
    fn plan_remotes(&self) -> Vec<PyPlanRemote> {
        self.inner
            .plan_remote
            .iter()
            .map(|r| PyPlanRemote { inner: r.clone() })
            .collect()
    }

    #[getter]
    fn audiences(&self) -> Vec<PyTimesheetAudience> {
        self.inner
            .timesheet_audience
            .iter()
            .map(|a| PyTimesheetAudience { inner: a.clone() })
            .collect()
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
            "Config(timezone={}, plan_remotes={}, audiences={}, roles={})",
            self.inner.timezone.name(),
            self.inner.plan_remote.len(),
            self.inner.timesheet_audience.len(),
            self.inner.role.len()
        )
    }
}

#[pymethods]
impl PyPlanRemote {
    #[getter]
    fn name(&self) -> String {
        self.inner.name.clone()
    }

    #[getter]
    fn plugin(&self) -> String {
        self.inner.plugin.clone()
    }

    #[getter]
    fn config(&self) -> Py<PyDict> {
        Python::attach(|py| {
            let py_obj = pythonize::pythonize(py, &self.inner.config)
                .expect("Failed to convert config to Python");
            py_obj.downcast::<PyDict>().unwrap().clone().unbind()
        })
    }

    #[getter]
    fn defaults(&self) -> Py<PyDict> {
        Python::attach(|py| {
            let py_obj = pythonize::pythonize(py, &self.inner.defaults)
                .expect("Failed to convert defaults to Python");
            py_obj.downcast::<PyDict>().unwrap().clone().unbind()
        })
    }

    fn __repr__(&self) -> String {
        format!("PlanRemote(name={}, plugin={})", self.inner.name, self.inner.plugin)
    }
}

#[pymethods]
impl PyPlanDefaults {
    #[getter]
    fn roles(&self) -> Vec<String> {
        self.inner.roles.clone()
    }

    #[getter]
    fn objectives(&self) -> Vec<String> {
        self.inner.objectives.clone()
    }

    #[getter]
    fn actions(&self) -> Vec<String> {
        self.inner.actions.clone()
    }

    fn __repr__(&self) -> String {
        format!(
            "PlanDefaults(roles={}, objectives={}, actions={})",
            self.inner.roles.len(),
            self.inner.objectives.len(),
            self.inner.actions.len()
        )
    }
}

#[pymethods]
impl PyTimesheetAudience {
    #[getter]
    fn name(&self) -> String {
        self.inner.name.clone()
    }

    #[getter]
    fn plugin(&self) -> String {
        self.inner.plugin.clone()
    }

    #[getter]
    fn config(&self) -> Py<PyDict> {
        Python::attach(|py| {
            let py_obj = pythonize::pythonize(py, &self.inner.config)
                .expect("Failed to convert config to Python");
            py_obj.downcast::<PyDict>().unwrap().clone().unbind()
        })
    }

    #[getter]
    fn signing_ids(&self) -> Vec<String> {
        self.inner.signing_ids.clone()
    }

    fn __repr__(&self) -> String {
        format!("TimesheetAudience(name={}, plugin={})", self.inner.name, self.inner.plugin)
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
