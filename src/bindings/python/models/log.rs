use chrono::{Datelike, NaiveDate};
use chrono_tz::Tz;
use pyo3::exceptions::PyValueError;
use pyo3::prelude::*;
use pyo3::types::{PyDate, PyDelta, PyDict, PyType};

use crate::bindings::python::models::session::PySession;
use crate::models::log::{Log as RustLog, LogError};

#[pyclass(name = "Log")]
#[derive(Clone)]
pub struct PyLog {
    pub inner: RustLog,
}

pub fn register(m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_class::<PyLog>()?;
    Ok(())
}

#[pymethods]
impl PyLog {
    #[new]
    #[pyo3(signature = (date, timezone, timeline=vec![]))]
    fn py_new(
        date: Bound<'_, PyDate>,
        timezone: Bound<'_, PyAny>,
        timeline: Vec<PySession>,
    ) -> PyResult<Self> {
        // Convert Python date to NaiveDate
        let date_str: String = date.call_method0("isoformat")?.extract()?;
        let naive_date = NaiveDate::parse_from_str(&date_str, "%Y-%m-%d")
            .map_err(|e| PyValueError::new_err(e.to_string()))?;

        // Convert timezone
        let tz_str: String = timezone.call_method0("__str__")?.extract()?;
        let tz: Tz = tz_str
            .parse()
            .map_err(|e| PyValueError::new_err(format!("Invalid timezone: {}", e)))?;

        // Convert sessions
        let rust_timeline: Vec<_> = timeline.into_iter().map(|s| s.inner).collect();

        Ok(Self {
            inner: RustLog::new(naive_date, tz, rust_timeline),
        })
    }

    #[getter]
    fn date<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyDate>> {
        PyDate::new(
            py,
            self.inner.date.year(),
            self.inner.date.month() as u8,
            self.inner.date.day() as u8,
        )
    }

    #[getter]
    fn timezone<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        let zoneinfo = py.import("zoneinfo")?;
        let zone_info = zoneinfo.call_method1("ZoneInfo", (self.inner.timezone.name(),))?;
        Ok(zone_info)
    }

    #[getter]
    fn timeline(&self) -> Vec<PySession> {
        self.inner
            .timeline
            .iter()
            .map(|s| PySession { inner: s.clone() })
            .collect()
    }

    #[classmethod]
    fn from_dict(_cls: &Bound<'_, PyType>, data: &Bound<'_, PyDict>) -> PyResult<Self> {
        // Extract date
        let date_str: String = data.get_item("date")?.unwrap().extract()?;
        let naive_date = NaiveDate::parse_from_str(&date_str, "%Y-%m-%d")
            .map_err(|e| PyValueError::new_err(e.to_string()))?;

        // Extract timezone
        let tz_str: String = data.get_item("timezone")?.unwrap().extract()?;
        let tz: Tz = tz_str
            .parse()
            .map_err(|e| PyValueError::new_err(format!("Invalid timezone: {}", e)))?;

        // Extract timeline
        let timeline = match data.get_item("timeline")? {
            Some(timeline_item) => match timeline_item.downcast::<pyo3::types::PyList>() {
                Ok(list) => {
                    let mut sessions = Vec::new();
                    for item in list.iter() {
                        let session_dict = item.downcast::<PyDict>()?;
                        let session = crate::bindings::python::models::session::session_from_dict_internal(
                            session_dict,
                            naive_date,
                            tz,
                        )?;
                        sessions.push(session.inner);
                    }
                    sessions
                }
                Err(_) => vec![],
            },
            None => vec![],
        };

        Ok(Self {
            inner: RustLog::new(naive_date, tz, timeline),
        })
    }

    fn append_session(&self, session: PySession) -> PyResult<PyLog> {
        Ok(PyLog {
            inner: self.inner.append_session(session.inner),
        })
    }

    fn active_session(&self) -> Option<PySession> {
        self.inner
            .active_session()
            .map(|s| PySession { inner: s.clone() })
    }

    fn stop_active_session(
        &self,
        stop_time: Bound<'_, pyo3::types::PyDateTime>,
    ) -> PyResult<PyLog> {
        use crate::bindings::python::type_mapping;

        let dt_tz = type_mapping::datetime_py_to_rust(stop_time)?;

        match self.inner.stop_active_session(dt_tz) {
            Ok(log) => Ok(PyLog { inner: log }),
            Err(LogError::NoTimelineEntries) => {
                Err(PyValueError::new_err("No timeline entries to stop."))
            }
        }
    }

    fn is_closed(&self) -> bool {
        self.inner.is_closed()
    }

    fn total_recorded_time<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyDelta>> {
        let duration = self.inner.total_recorded_time();

        let total_micros = duration.num_microseconds().unwrap_or(0);
        let days = (total_micros / 86_400_000_000) as i32;
        let seconds = ((total_micros % 86_400_000_000) / 1_000_000) as i32;
        let micros = (total_micros % 1_000_000) as i32;

        PyDelta::new(py, days, seconds, micros, false)
    }

    fn __repr__(&self) -> PyResult<String> {
        Ok(format!(
            "Log(date={}, timezone={}, timeline=[{} sessions])",
            self.inner.date,
            self.inner.timezone.name(),
            self.inner.timeline.len()
        ))
    }

    fn __str__(&self) -> PyResult<String> {
        self.__repr__()
    }
}
