use crate::models::session::SessionError;
use crate::models::valuetype::ValueType;
use crate::models::Session as RustSession;
use crate::plugins::models::intent::PyIntent;
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
    // Check if there's a nested intent dict (from saved JSON format)
    if let Some(intent_item) = dict.get_item("intent")? {
        if let Ok(intent_dict) = intent_item.downcast::<PyDict>() {
            // Parse the intent first
            let py_intent = crate::plugins::models::intent::intent_from_dict_internal(intent_dict)?;

            // Extract start/end/note from the session dict
            // Parse RFC3339 datetime (includes offset) and convert to semantic timezone
            let start_str: String = dict.get_item("start")?.unwrap().extract()?;
            let start = chrono::DateTime::parse_from_rfc3339(&start_str)
                .map_err(|e| PyValueError::new_err(format!("Invalid start datetime: {e}")))?
                .with_timezone(&tz);

            let end = dict
                .get_item("end")?
                .and_then(|v| if v.is_none() { None } else { Some(v) })
                .map(|v| v.extract::<String>())
                .transpose()?
                .map(|s| {
                    chrono::DateTime::parse_from_rfc3339(&s)
                        .map(|dt| dt.with_timezone(&tz))
                        .map_err(|e| PyValueError::new_err(format!("Invalid end datetime: {e}")))
                })
                .transpose()?;

            let note = dict
                .get_item("note")?
                .and_then(|v| if v.is_none() { None } else { Some(v) })
                .map(|v| v.extract::<String>())
                .transpose()?;

            let reflection_score = dict
                .get_item("reflection_score")?
                .and_then(|v| if v.is_none() { None } else { Some(v) })
                .map(|v| v.extract::<i32>())
                .transpose()?;

            let reflection = dict
                .get_item("reflection")?
                .and_then(|v| if v.is_none() { None } else { Some(v) })
                .map(|v| v.extract::<String>())
                .transpose()?;

            return Ok(PySession {
                inner: RustSession {
                    intent: py_intent.inner,
                    start,
                    end,
                    note,
                    reflection_score,
                    reflection,
                },
            });
        }
    }

    // Otherwise, use the flat format (for backwards compatibility)
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
    #[pyo3(signature = (intent, start, end=None, note=None))]
    fn py_new<'py>(
        intent: PyIntent,
        start: Bound<'py, PyDateTime>,
        end: Option<Bound<'py, PyDateTime>>,
        note: Option<String>,
    ) -> PyResult<Self> {
        let start = type_mapping::datetime_py_to_rust(start)?;
        let end = match end {
            Some(end_dt) => Some(type_mapping::datetime_py_to_rust(end_dt)?),
            None => None,
        };
        Ok(Self {
            inner: RustSession::new(intent.inner, start, end, note),
        })
    }

    fn __getstate__(&self, py: Python<'_>) -> PyResult<Py<PyAny>> {
        let dict = PyDict::new(py);

        dict.set_item(
            "intent",
            Py::new(
                py,
                PyIntent {
                    inner: self.inner.intent.clone(),
                },
            )?,
        )?;
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
    fn intent(&self) -> PyIntent {
        PyIntent {
            inner: self.inner.intent.clone(),
        }
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

            let intent = &self.inner.intent;
            d.set_item(
                "intent",
                PyIntent {
                    inner: intent.clone(),
                },
            )?;

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
            "Session(intent={:?}, start={:?}, end={:?}, note={:?})",
            self.inner.intent, self.inner.start, self.inner.end, self.inner.note,
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
