use pyo3::prelude::*;
use crate::models::intent::Intent as RustIntent;

#[pyclass(name = "Intent")]
#[derive(Clone)]
pub struct PyIntent {
    inner: RustIntent,
}

pub fn register(m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_class::<PyIntent>()?;
    Ok(())
}

#[pymethods]
impl PyIntent {
    #[new]
    pub fn new(
        alias: Option<String>,
        role: Option<String>,
        objective: Option<String>,
        action: Option<String>,
        subject: Option<String>,
        trackers: Vec<String>,
    ) -> Self {
        Self {
            inner: RustIntent::new(alias, role, objective, action, subject, trackers),
        }
    }

    #[getter]
    fn alias(&self) -> Option<String> {
        self.inner.alias.clone()
    }

    #[getter]
    fn role(&self) -> Option<String> {
        self.inner.role.clone()
    }

    #[getter]
    fn objective(&self) -> Option<String> {
        self.inner.objective.clone()
    }

    #[getter]
    fn action(&self) -> Option<String> {
        self.inner.action.clone()
    }

    #[getter]
    fn subject(&self) -> Option<String> {
        self.inner.subject.clone()
    }

    #[getter]
    fn trackers(&self) -> Vec<String> {
        self.inner.trackers.clone()
    }
}
