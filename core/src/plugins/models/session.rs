use crate::models::session::SessionError;
use crate::models::valuetype::ValueType;
use crate::models::Session as RustSession;
use chrono::NaiveDate;
use chrono_tz::Tz;
use pyo3::exceptions::PyValueError;
use pyo3::prelude::*;
use pyo3::types::PyDateTime;
use pyo3::types::{PyDelta, PyDict, PyType};
use std::collections::HashMap;

use crate::utils::type_mapping;

/// The Python-visible Session class
#[pyclass(name = "Session")]
#[derive(Clone)]
pub struct PySession {
    pub inner: RustSession,
}

pub fn register(m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_class::<PySession>()?;
    Ok(())
}

// Helper function for creating sessions from dicts (used by Log and Timesheet bindings)
pub(crate) fn session_from_dict_internal(
    dict: &Bound<'_, PyDict>,
    date: NaiveDate,
    tz: Tz,
) -> PyResult<PySession> {
    // Use the flat format (new format without nested intent dict)
    let mut data = HashMap::new();

    for (k, v) in dict.iter() {
        let key: String = k.extract()?;
        if v.is_instance_of::<pyo3::types::PyString>() {
            data.insert(key, ValueType::String(v.extract()?));
        } else if v.is_instance_of::<pyo3::types::PyList>() {
            data.insert(key, ValueType::List(v.extract()?));
        } else if v.is_instance_of::<pyo3::types::PyInt>() {
            data.insert(key, ValueType::Integer(v.extract()?));
        }
        // Skip other types
    }

    let inner = RustSession::from_dict_with_tz(data, date, tz)
        .map_err(pyo3::exceptions::PyValueError::new_err)?;

    Ok(PySession { inner })
}

#[pymethods]
impl PySession {
    #[new]
    #[pyo3(signature = (start, title=None, role=None, impact=None, mode=None, subject=None, trackers=vec![], end=None, note=None))]
    #[allow(clippy::too_many_arguments)]
    fn py_new<'py>(
        start: Bound<'py, PyDateTime>,
        title: Option<String>,
        role: Option<String>,
        impact: Option<String>,
        mode: Option<String>,
        subject: Option<String>,
        trackers: Vec<String>,
        end: Option<Bound<'py, PyDateTime>>,
        note: Option<String>,
    ) -> PyResult<Self> {
        let start = type_mapping::datetime_py_to_rust(start)?;
        let end = match end {
            Some(end_dt) => Some(type_mapping::datetime_py_to_rust(end_dt)?),
            None => None,
        };
        Ok(Self {
            inner: RustSession::new(title, role, impact, mode, subject, trackers, start, end, note),
        })
    }

    fn __getstate__(&self, py: Python<'_>) -> PyResult<Py<PyAny>> {
        let dict = PyDict::new(py);

        if let Some(title) = &self.inner.title {
            dict.set_item("title", title)?;
        }
        if let Some(role) = &self.inner.role {
            dict.set_item("role", role)?;
        }
        if let Some(impact) = &self.inner.impact {
            dict.set_item("impact", impact)?;
        }
        if let Some(mode) = &self.inner.mode {
            dict.set_item("mode", mode)?;
        }
        if let Some(subject) = &self.inner.subject {
            dict.set_item("subject", subject)?;
        }
        if !self.inner.trackers.is_empty() {
            dict.set_item("trackers", self.inner.trackers.clone())?;
        }
        dict.set_item("start", self.inner.start.to_rfc3339())?;
        if let Some(end) = &self.inner.end {
            dict.set_item("end", end.to_rfc3339())?;
        }
        if let Some(note) = &self.inner.note {
            dict.set_item("note", note)?;
        }
        if let Some(score) = self.inner.reflection_score {
            dict.set_item("reflection_score", score)?;
        }
        if let Some(reflection) = &self.inner.reflection {
            dict.set_item("reflection", reflection)?;
        }

        Ok(dict.unbind().into())
    }

    #[getter]
    fn title(&self) -> Option<String> {
        self.inner.title.clone()
    }

    #[getter]
    fn role(&self) -> Option<String> {
        self.inner.role.clone()
    }

    #[getter]
    fn impact(&self) -> Option<String> {
        self.inner.impact.clone()
    }

    #[getter]
    fn mode(&self) -> Option<String> {
        self.inner.mode.clone()
    }

    #[getter]
    fn subject(&self) -> Option<String> {
        self.inner.subject.clone()
    }

    #[getter]
    fn trackers(&self) -> Vec<String> {
        self.inner.trackers.clone()
    }

    #[getter]
    fn start<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyDateTime>> {
        type_mapping::datetime_rust_to_py(py, &self.inner.start)
    }

    #[getter]
    fn end<'py>(&self, py: Python<'py>) -> PyResult<Option<Bound<'py, PyDateTime>>> {
        match &self.inner.end {
            Some(dt) => Ok(Some(type_mapping::datetime_rust_to_py(py, dt)?)),
            None => Ok(None),
        }
    }

    #[getter]
    fn note(&self) -> Option<String> {
        self.inner.note.clone()
    }

    #[getter]
    fn reflection_score(&self) -> Option<i32> {
        self.inner.reflection_score
    }

    #[getter]
    fn reflection(&self) -> Option<String> {
        self.inner.reflection.clone()
    }

    #[getter]
    fn duration<'py>(&self, py: Python<'py>) -> PyResult<pyo3::Bound<'py, pyo3::types::PyDelta>> {
        match self.inner.duration() {
            Ok(dur) => {
                // Convert chrono::Duration to Python timedelta
                let total_micros = dur.num_microseconds().unwrap_or(0);
                let days = (total_micros / 86_400_000_000) as i32;
                let seconds = ((total_micros % 86_400_000_000) / 1_000_000) as i32;
                let micros = (total_micros % 1_000_000) as i32;

                PyDelta::new(py, days, seconds, micros, false)
            }
            Err(SessionError::MissingEnd) => Err(PyValueError::new_err(
                "Cannot compute duration: session has no end time",
            )),
            Err(SessionError::EndBeforeStart) => Err(PyValueError::new_err(
                "Cannot compute duration: end time is before start time",
            )),
        }
    }

    /// Get elapsed time for an open session
    ///
    /// For open sessions, returns time elapsed since start.
    /// Raises ValueError if session is already closed (use duration property instead).
    fn elapsed<'py>(
        &self,
        py: Python<'py>,
        now: Bound<'py, PyDateTime>,
    ) -> PyResult<pyo3::Bound<'py, pyo3::types::PyDelta>> {
        if self.inner.end.is_some() {
            return Err(PyValueError::new_err(
                "elapsed() called on closed session - use duration property instead",
            ));
        }

        let now_dt = type_mapping::datetime_py_to_rust(now)?;
        let dur = self.inner.elapsed(now_dt);

        let total_micros = dur.num_microseconds().unwrap_or(0);
        let days = (total_micros / 86_400_000_000) as i32;
        let seconds = ((total_micros % 86_400_000_000) / 1_000_000) as i32;
        let micros = (total_micros % 1_000_000) as i32;

        PyDelta::new(py, days, seconds, micros, false)
    }

    #[classmethod]
    fn from_dict_with_tz(
        _cls: &Bound<'_, PyType>,
        dict: &Bound<'_, PyAny>,
        date: &Bound<'_, PyAny>,
        tz: &Bound<'_, PyAny>,
    ) -> PyResult<Self> {
        let py_dict = dict.downcast::<PyDict>()?;
        let mut data = HashMap::new();

        for (k, v) in py_dict.iter() {
            let key: String = k.extract()?;
            if v.is_instance_of::<pyo3::types::PyString>() {
                data.insert(key, ValueType::String(v.extract()?));
            } else if v.is_instance_of::<pyo3::types::PyList>() {
                data.insert(key, ValueType::List(v.extract()?));
            } else if v.is_instance_of::<pyo3::types::PyInt>() {
                data.insert(key, ValueType::Integer(v.extract()?));
            } else {
                return Err(pyo3::exceptions::PyValueError::new_err(format!(
                    "Unsupported type for key '{key}'"
                )));
            }
        }
        let date_str: String = date.call_method0("isoformat")?.extract()?;

        let date = NaiveDate::parse_from_str(&date_str, "%Y-%m-%d")
            .map_err(|e| pyo3::exceptions::PyValueError::new_err(e.to_string()))?;

        let tz_str: String = tz.call_method0("__str__")?.extract()?;
        let tz = tz_str
            .parse::<Tz>()
            .map_err(|e| pyo3::exceptions::PyValueError::new_err(e.to_string()))?;

        let inner = RustSession::from_dict_with_tz(data, date, tz)
            .map_err(pyo3::exceptions::PyValueError::new_err)?;

        Ok(Self { inner })
    }

    fn with_end<'py>(&self, end: Bound<'py, PyDateTime>) -> PyResult<PySession> {
        let dt_tz = type_mapping::datetime_py_to_rust(end)?;
        Ok(PySession {
            inner: self.inner.with_end(dt_tz),
        })
    }

    fn with_reflection(
        &self,
        score: Option<i32>,
        reflection: Option<String>,
    ) -> PyResult<PySession> {
        Ok(PySession {
            inner: self.inner.with_reflection(score, reflection),
        })
    }

    fn as_dict(&self) -> PyResult<Py<PyDict>> {
        Python::attach(|py| {
            let d = PyDict::new(py);

            if let Some(title) = &self.inner.title {
                d.set_item("title", title)?;
            }
            if let Some(role) = &self.inner.role {
                d.set_item("role", role)?;
            }
            if let Some(impact) = &self.inner.impact {
                d.set_item("impact", impact)?;
            }
            if let Some(mode) = &self.inner.mode {
                d.set_item("mode", mode)?;
            }
            if let Some(subject) = &self.inner.subject {
                d.set_item("subject", subject)?;
            }
            if !self.inner.trackers.is_empty() {
                d.set_item("trackers", self.inner.trackers.clone())?;
            }

            let start = &self.inner.start;
            d.set_item("start", type_mapping::datetime_rust_to_py(py, start)?)?;

            if let Some(end) = &self.inner.end {
                d.set_item("end", type_mapping::datetime_rust_to_py(py, end)?)?;
            }
            if let Some(note) = &self.inner.note {
                d.set_item("note", note)?;
            }
            if let Some(score) = self.inner.reflection_score {
                d.set_item("reflection_score", score)?;
            }
            if let Some(reflection) = &self.inner.reflection {
                d.set_item("reflection", reflection)?;
            }
            Ok(d.into())
        })
    }

    fn __repr__(&self) -> PyResult<String> {
        Ok(format!(
            "Session(title={:?}, role={:?}, start={:?}, end={:?}, note={:?})",
            self.inner.title, self.inner.role, self.inner.start, self.inner.end, self.inner.note,
        ))
    }

    fn __str__(&self) -> PyResult<String> {
        self.__repr__()
    }

    fn __eq__(&self, other: &PySession) -> bool {
        self.inner == other.inner
    }

    fn __ne__(&self, other: &PySession) -> bool {
        self.inner != other.inner
    }
}
