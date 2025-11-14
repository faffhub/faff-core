use crate::models::intent::Intent as RustIntent;
use pyo3::prelude::*;
use pyo3::types::PyAny;
use pyo3::types::{PyDict, PyType};

/// Arguments tuple for Intent pickle serialization (__reduce__)
/// Contains: (alias, role, objective, action, subject, tags, note)
type PickleArgs = (
    Option<String>,
    Option<String>,
    Option<String>,
    Option<String>,
    Option<String>,
    Vec<String>,
    Option<String>,
);

#[pyclass(name = "Intent")]
#[derive(Clone)]
pub struct PyIntent {
    pub inner: RustIntent,
}

pub fn register(m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_class::<PyIntent>()?;
    Ok(())
}

// Helper function for creating intents from dicts (used by Plan and Session bindings)
pub(crate) fn intent_from_dict_internal(dict: &Bound<'_, PyDict>) -> PyResult<PyIntent> {
    let inner: RustIntent = pythonize::depythonize(dict.as_any())
        .map_err(|e| pyo3::exceptions::PyValueError::new_err(e.to_string()))?;
    Ok(PyIntent { inner })
}

#[pymethods]
impl PyIntent {
    #[new]
    #[pyo3(signature = (alias=None, role=None, objective=None, action=None, subject=None, trackers=vec![], intent_id=None))]
    pub fn new(
        alias: Option<String>,
        role: Option<String>,
        objective: Option<String>,
        action: Option<String>,
        subject: Option<String>,
        trackers: Vec<String>,
        intent_id: Option<String>,
    ) -> Self {
        Self {
            inner: RustIntent::new_with_id(
                intent_id, alias, role, objective, action, subject, trackers,
            ),
        }
    }

    #[getter]
    fn intent_id(&self) -> String {
        self.inner.intent_id.clone()
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
        let py_dict = dict.downcast::<PyDict>()?;
        intent_from_dict_internal(py_dict)
    }

    fn __hash__(&self) -> u64 {
        use std::collections::hash_map::DefaultHasher;
        use std::hash::{Hash, Hasher};
        let mut hasher = DefaultHasher::new();
        self.inner.hash(&mut hasher);
        hasher.finish()
    }

    fn __eq__(&self, other: PyRef<PyIntent>) -> PyResult<bool> {
        Ok(self.inner == other.inner)
    }

    fn __ne__(&self, other: PyRef<PyIntent>) -> PyResult<bool> {
        self.__eq__(other).map(|eq| !eq)
    }

    fn as_dict(&self) -> PyResult<Py<PyDict>> {
        Python::attach(|py| {
            let py_obj = pythonize::pythonize(py, &self.inner)
                .map_err(|e| pyo3::exceptions::PyValueError::new_err(e.to_string()))?;
            Ok(py_obj.downcast::<PyDict>()?.clone().unbind())
        })
    }

    fn __getstate__(&self) -> PyResult<Py<PyDict>> {
        self.as_dict()
    }

    fn __repr__(&self) -> PyResult<String> {
        Ok(format!(
            "Intent(intent_id={:?}, alias={:?}, role={:?}, objective={:?}, action={:?}, subject={:?}, trackers={:?})",
            self.inner.intent_id,
            self.inner.alias,
            self.inner.role,
            self.inner.objective,
            self.inner.action,
            self.inner.subject,
            self.inner.trackers,
        ))
    }

    fn __str__(&self) -> PyResult<String> {
        self.__repr__()
    }

    fn __reduce__(&self, py: Python) -> PyResult<(Py<PyAny>, PickleArgs)> {
        let intent_type = py.get_type::<Self>();
        Ok((
            intent_type.into(),
            (
                self.inner.alias.clone(),
                self.inner.role.clone(),
                self.inner.objective.clone(),
                self.inner.action.clone(),
                self.inner.subject.clone(),
                self.inner.trackers.clone(),
                Some(self.inner.intent_id.clone()),
            ),
        ))
    }
}
