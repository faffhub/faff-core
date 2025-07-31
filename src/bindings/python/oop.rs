use pyo3::prelude::*;
use pyo3::types::{PyType, PyDict};
use std::collections::HashMap;
use crate::models::{Intent as RustIntent, Session as RustSession};
use crate::bindings::python::intent::PyIntent;
use crate::models::valuetype::ValueType;

use chrono::{NaiveDate, NaiveTime, DateTime, TimeZone, FixedOffset};
use chrono_tz::Tz;

#[pyclass(name = "Session")]
#[derive(Clone)]
pub struct PySession {
    inner: RustSession,
}

pub fn register(m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_class::<PySession>()?;
    Ok(())
}

#[pymethods]
impl PySession {
    #[new]
    pub fn new(
        intent: PyIntent,
        start: &Bound<'_, PyAny>,
        end: Option<&Bound<'_, PyAny>>,
        note: Option<String>,
    ) -> PyResult<Self> {
        // Try to extract start as a string and parse as RFC3339
        let start_str: String = start.extract()?;
        let start_dt = start_str.parse::<DateTime<Tz>>()
            .map_err(|e| pyo3::exceptions::PyValueError::new_err(format!("Invalid start datetime: {}", e)))?;

        let end_dt = match end {
            Some(end_any) => {
                let end_str: String = end_any.extract()?;
                Some(end_str.parse::<DateTime<Tz>>()
                    .map_err(|e| pyo3::exceptions::PyValueError::new_err(format!("Invalid end datetime: {}", e)))?)
            },
            None => None,
        };

        Ok(Self {
            inner: RustSession::new(intent, start_dt, end_dt, note),
        })
    }

    #[getter]
    fn intent(&self) -> PyIntent {
        PyIntent { inner: self.inner.intent.clone() }
    }

//    #[getter]
//    fn start<'py>(&self, py: Python<'py>) -> PyResult<&'py PyAny> {
//        let dt = self.inner.start;
//        let py_dt = pyo3_chrono::to_py_datetime(py, &dt)?;
//        Ok(py_dt)
//    }


    #[getter]
    fn start(&self) -> String {
        self.inner.start.to_rfc3339()
    }

    #[getter]
    fn end(&self) -> Option<String> {
        self.inner.end.as_ref().map(|dt| dt.to_rfc3339())
    }

    #[getter]
    fn note(&self) -> Option<String> {
        self.inner.note.clone()
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

//    fn as_dict(&self) -> PyResult<Py<PyDict>> {
//        Python::with_gil(|py| {
//            let d = PyDict::new(py);
//            if let Some(alias) = &self.inner.alias {
//                d.set_item("alias", alias)?;
//            }
//            if let Some(role) = &self.inner.role {
//                d.set_item("role", role)?;
//            }
//            if let Some(objective) = &self.inner.objective {
//                d.set_item("objective", objective)?;
//            }
//            if let Some(action) = &self.inner.action {
//                d.set_item("action", action)?;
//            }
//            if let Some(subject) = &self.inner.subject {
//                d.set_item("subject", subject)?;
//            }
//            d.set_item("trackers", self.inner.trackers.clone())?;
//            Ok(d.into())
//        })
//    }
//
//    fn __getstate__(&self) -> PyResult<Py<PyDict>> {
//        self.as_dict()
//    }

    fn __repr__(&self) -> PyResult<String> {
        Ok(format!(
            "Session(intent={:?}, start={:?}, end={:?}, note={:?})",
            self.inner.intent,
            self.inner.start,
            self.inner.end,
            self.inner.note,
        ))
    }

    fn __str__(&self) -> PyResult<String> {
        self.__repr__()
    }

    // XXX: I'm acutely aware that I do not know what this is or how it works.
    // It _is_ needed, however.
//    fn __reduce__(&self, py: Python) -> PyResult<(PyObject, (Option<String>, Option<String>, Option<String>, Option<String>, Option<String>, Vec<String>))> {
//        let intent_type = py.get_type::<Self>();
//        Ok((
//            intent_type.into(),
//            (
//                self.inner.intent.clone(),
//                self.inner.start.clone(),
//                self.inner.end.clone(),
//                self.inner.note.clone(),
//            )
//        ))
//    }
}
