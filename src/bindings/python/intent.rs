use pyo3::prelude::*;
use pyo3::types::{PyType, PyDict};
use pyo3::types::PyAny;
use std::collections::HashMap;
use crate::models::intent::Intent as RustIntent;
use crate::models::valuetype::ValueType;

#[pyclass(name = "Intent")]
#[derive(Clone)]
pub struct PyIntent {
    pub inner: RustIntent,
}

pub fn register(m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_class::<PyIntent>()?;
    Ok(())
}

// Helper function for creating intents from dicts (used by Plan binding)
pub(crate) fn intent_from_dict_internal(dict: &Bound<'_, PyDict>) -> PyResult<PyIntent> {
    let mut data = HashMap::new();

    for (k, v) in dict.iter() {
        let key: String = k.extract()?;

        if v.is_instance_of::<pyo3::types::PyString>() {
            let s: String = v.extract()?;
            data.insert(key, ValueType::String(s));
        } else if v.is_instance_of::<pyo3::types::PyList>() {
            let list: Vec<String> = v.extract()?;
            data.insert(key, ValueType::List(list));
        } else {
            return Err(pyo3::exceptions::PyValueError::new_err(format!(
                "Unsupported type for key '{}'", key
            )));
        }
    }

    match RustIntent::from_dict(data) {
        Ok(intent) => Ok(PyIntent { inner: intent }),
        Err(e) => Err(pyo3::exceptions::PyValueError::new_err(e)),
    }
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
        let py_dict = dict.downcast::<PyDict>()?;

        let mut data = HashMap::new();

        for (k, v) in py_dict.iter() {
            let key: String = k.extract()?;

            if v.is_instance_of::<pyo3::types::PyString>() {
                let s: String = v.extract()?; 
                data.insert(key, ValueType::String(s));
            } else if v.is_instance_of::<pyo3::types::PyList>() {
                let list: Vec<String> = v.extract()?; 
                data.insert(key, ValueType::List(list));
            } else {
                return Err(pyo3::exceptions::PyValueError::new_err(format!(
                    "Unsupported type for key '{}'", key
                )));
            }
        }
        
        match RustIntent::from_dict(data) {
            Ok(intent) => Ok(PyIntent { inner: intent }),
            Err(e) => Err(pyo3::exceptions::PyValueError::new_err(e)),
        }
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
        Python::with_gil(|py| {
            let d = PyDict::new(py);
            if let Some(alias) = &self.inner.alias {
                d.set_item("alias", alias)?;
            }
            if let Some(role) = &self.inner.role {
                d.set_item("role", role)?;
            }
            if let Some(objective) = &self.inner.objective {
                d.set_item("objective", objective)?;
            }
            if let Some(action) = &self.inner.action {
                d.set_item("action", action)?;
            }
            if let Some(subject) = &self.inner.subject {
                d.set_item("subject", subject)?;
            }
            d.set_item("trackers", self.inner.trackers.clone())?;
            Ok(d.into())
        })
    }

    fn __getstate__(&self) -> PyResult<Py<PyDict>> {
        self.as_dict()
    }

    fn __repr__(&self) -> PyResult<String> {
        Ok(format!(
            "Intent(alias={:?}, role={:?}, objective={:?}, action={:?}, subject={:?}, trackers={:?})",
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

    // XXX: I'm acutely aware that I do not know what this is or how it works.
    // It _is_ needed, however.
    fn __reduce__(&self, py: Python) -> PyResult<(PyObject, (Option<String>, Option<String>, Option<String>, Option<String>, Option<String>, Vec<String>))> {
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
            )
        ))
    }
}
