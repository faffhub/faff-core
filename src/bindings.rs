use pyo3::prelude::*;
use pyo3::types::{PyType, PyDict};
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

    #[classmethod]
    fn from_dict(_cls: &Bound<'_, PyType>, dict: &Bound<'_, PyAny>) -> PyResult<Self> {
        let dict = dict.downcast::<PyDict>()?;
        let alias = match dict.get_item("alias")? {
            Some(v) => Some(v.extract()?),
            None => None,
        };

        let role = match dict.get_item("role")? {
            Some(v) => Some(v.extract()?),
            None => None,
        };

        let objective = match dict.get_item("objective")? {
            Some(v) => Some(v.extract()?),
            None => None,
        };

        let action = match dict.get_item("action")? {
            Some(v) => Some(v.extract()?),
            None => None,
        };

        let subject = match dict.get_item("subject")? {
            Some(v) => Some(v.extract()?),
            None => None,
        };

        let trackers = if let Some(v) = dict.get_item("trackers")? {
            v.extract()?
        } else {
            Vec::new()
        };

        Ok(Self {
            inner: RustIntent::new(alias, role, objective, action, subject, trackers),
        })
    }

    fn __hash__(&self) -> u64 { 
        use std::collections::hash_map::DefaultHasher;
        use std::hash::{Hash, Hasher};
        let mut hasher = DefaultHasher::new();
        self.inner.hash(&mut hasher);
        hasher.finish()
    }
}
