use pyo3::exceptions::PyValueError;
use pyo3::prelude::*;
use pyo3::types::PyDateTime;
use pyo3::types::{PyBytes, PyDict, PyType};
use std::collections::HashMap;

use crate::models::{
    valuetype::ValueType, SubmittableTimesheet as RustSubmittableTimesheet,
    Timesheet as RustTimesheet, TimesheetMeta as RustTimesheetMeta,
};
use crate::plugins::models::session::PySession;
use chrono::NaiveDate;
use chrono_tz::Tz;

use crate::utils::type_mapping;

/// The Python-visible TimesheetMeta class
#[pyclass(name = "TimesheetMeta")]
#[derive(Clone)]
pub struct PyTimesheetMeta {
    pub inner: RustTimesheetMeta,
}

pub fn register(m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_class::<PyTimesheetMeta>()?;
    m.add_class::<PyTimesheet>()?;
    m.add_class::<PySubmittableTimesheet>()?;
    Ok(())
}

#[pymethods]
impl PyTimesheetMeta {
    #[new]
    #[pyo3(signature = (audience_id, log_hash="".to_string(), submitted_at=None))]
    fn py_new<'py>(
        audience_id: String,
        log_hash: String,
        submitted_at: Option<Bound<'py, PyDateTime>>,
    ) -> PyResult<Self> {
        let submitted_at = match submitted_at {
            Some(dt) => Some(type_mapping::datetime_py_to_rust(dt)?),
            None => None,
        };

        Ok(Self {
            inner: RustTimesheetMeta::new(audience_id, submitted_at, log_hash),
        })
    }

    #[getter]
    fn audience_id(&self) -> String {
        self.inner.audience_id.clone()
    }

    #[getter]
    fn submitted_at<'py>(&self, py: Python<'py>) -> PyResult<Option<Bound<'py, PyDateTime>>> {
        match &self.inner.submitted_at {
            Some(dt) => Ok(Some(type_mapping::datetime_rust_to_py(py, dt)?)),
            None => Ok(None),
        }
    }

    #[getter]
    fn log_hash(&self) -> Option<String> {
        self.inner.log_hash.clone()
    }

    #[getter]
    fn submission_status(&self) -> Option<String> {
        self.inner.submission_status.as_ref().map(|s| match s {
            crate::models::SubmissionStatus::Success => "success".to_string(),
            crate::models::SubmissionStatus::Failed => "failed".to_string(),
            crate::models::SubmissionStatus::Partial => "partial".to_string(),
        })
    }

    #[getter]
    fn submission_error(&self) -> Option<String> {
        self.inner.submission_error.clone()
    }

    #[classmethod]
    fn from_dict(_cls: &Bound<'_, PyType>, dict: &Bound<'_, PyDict>) -> PyResult<Self> {
        let mut data = HashMap::new();

        for (k, v) in dict.iter() {
            let key: String = k.extract()?;
            if v.is_instance_of::<pyo3::types::PyString>() {
                data.insert(key, ValueType::String(v.extract()?));
            } else {
                return Err(PyValueError::new_err(format!(
                    "Unsupported type for key '{key}'"
                )));
            }
        }

        let inner =
            RustTimesheetMeta::from_dict(data).map_err(|e| PyValueError::new_err(e.to_string()))?;

        Ok(Self { inner })
    }

    fn as_dict<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyDict>> {
        let dict = PyDict::new(py);
        dict.set_item("audience_id", &self.inner.audience_id)?;

        if let Some(submitted_at) = &self.inner.submitted_at {
            let dt = type_mapping::datetime_rust_to_py(py, submitted_at)?;
            dict.set_item("submitted_at", dt)?;
        }

        if let Some(log_hash) = &self.inner.log_hash {
            dict.set_item("log_hash", log_hash)?;
        }

        if let Some(status) = &self.inner.submission_status {
            let status_str = match status {
                crate::models::SubmissionStatus::Success => "success",
                crate::models::SubmissionStatus::Failed => "failed",
                crate::models::SubmissionStatus::Partial => "partial",
            };
            dict.set_item("submission_status", status_str)?;
        }

        if let Some(error) = &self.inner.submission_error {
            dict.set_item("submission_error", error)?;
        }

        Ok(dict)
    }
}

/// The Python-visible Timesheet class
#[pyclass(name = "Timesheet")]
#[derive(Clone)]
pub struct PyTimesheet {
    pub inner: RustTimesheet,
}

#[pymethods]
impl PyTimesheet {
    #[new]
    #[pyo3(signature = (*, actor=None, date, compiled, timezone, timeline=None, signatures=None, meta))]
    fn py_new<'py>(
        actor: Option<HashMap<String, String>>,
        date: Bound<'py, pyo3::types::PyDate>,
        compiled: Bound<'py, PyDateTime>,
        timezone: Bound<'py, pyo3::types::PyAny>,
        timeline: Option<Vec<PySession>>,
        signatures: Option<HashMap<String, HashMap<String, String>>>,
        meta: PyTimesheetMeta,
    ) -> PyResult<Self> {
        let date = type_mapping::date_py_to_rust(date)?;
        let compiled = type_mapping::datetime_py_to_rust(compiled)?;

        // Convert timezone (ZoneInfo object)
        let tz_str: String = timezone.call_method0("__str__")?.extract()?;
        let timezone: Tz = tz_str
            .parse()
            .map_err(|_| PyValueError::new_err(format!("Invalid timezone: {tz_str}")))?;

        let timeline = timeline
            .unwrap_or_default()
            .into_iter()
            .map(|s| s.inner)
            .collect();

        Ok(Self {
            inner: RustTimesheet::new(
                actor.unwrap_or_default(),
                date,
                compiled,
                timezone,
                timeline,
                signatures.unwrap_or_default(),
                meta.inner,
            ),
        })
    }

    #[getter]
    fn actor(&self) -> HashMap<String, String> {
        self.inner.actor.clone()
    }

    #[getter]
    fn date<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, pyo3::types::PyDate>> {
        type_mapping::date_rust_to_py(py, &self.inner.date)
    }

    #[getter]
    fn compiled<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyDateTime>> {
        type_mapping::datetime_rust_to_py(py, &self.inner.compiled)
    }

    #[getter]
    fn timezone<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, pyo3::types::PyAny>> {
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

    #[getter]
    fn signatures(&self) -> HashMap<String, HashMap<String, String>> {
        self.inner.signatures.clone()
    }

    #[getter]
    fn meta(&self) -> PyTimesheetMeta {
        PyTimesheetMeta {
            inner: self.inner.meta.clone(),
        }
    }

    fn sign(&self, id: String, signing_key: &Bound<'_, PyBytes>) -> PyResult<Self> {
        let key_bytes = signing_key.as_bytes();
        let inner = self
            .inner
            .sign(&id, key_bytes)
            .map_err(|e| PyValueError::new_err(e.to_string()))?;

        Ok(Self { inner })
    }

    #[pyo3(signature = (audience_id, submitted_at=None))]
    fn update_meta<'py>(
        &self,
        audience_id: String,
        submitted_at: Option<Bound<'py, PyDateTime>>,
    ) -> PyResult<Self> {
        let submitted_at = match submitted_at {
            Some(dt) => Some(type_mapping::datetime_py_to_rust(dt)?),
            None => None,
        };

        Ok(Self {
            inner: self.inner.update_meta(audience_id, submitted_at),
        })
    }

    fn submittable_timesheet(&self) -> PySubmittableTimesheet {
        PySubmittableTimesheet {
            inner: self.inner.submittable_timesheet(),
        }
    }

    #[classmethod]
    fn from_dict(_cls: &Bound<'_, PyType>, dict: &Bound<'_, PyDict>) -> PyResult<Self> {
        // Extract actor
        let actor: HashMap<String, String> = dict
            .get_item("actor")?
            .map(|v| v.extract())
            .transpose()?
            .unwrap_or_default();

        // Extract date
        let date_str: String = dict
            .get_item("date")?
            .ok_or_else(|| PyValueError::new_err("Missing 'date' field"))?
            .extract()?;
        let date = NaiveDate::parse_from_str(&date_str, "%Y-%m-%d")
            .map_err(|e| PyValueError::new_err(format!("Invalid date format: {e}")))?;

        // Extract compiled
        let compiled_str: String = dict
            .get_item("compiled")?
            .ok_or_else(|| PyValueError::new_err("Missing 'compiled' field"))?
            .extract()?;
        let compiled = chrono::DateTime::parse_from_rfc3339(&compiled_str)
            .map_err(|e| PyValueError::new_err(format!("Invalid compiled datetime: {e}")))?
            .with_timezone(&chrono_tz::UTC);

        // Extract timezone
        let timezone_str: String = dict
            .get_item("timezone")?
            .ok_or_else(|| PyValueError::new_err("Missing 'timezone' field"))?
            .extract()?;
        let timezone: Tz = timezone_str
            .parse()
            .map_err(|_| PyValueError::new_err(format!("Invalid timezone: {timezone_str}")))?;

        // Extract timeline
        let mut timeline = Vec::new();
        if let Some(timeline_item) = dict.get_item("timeline")? {
            if let Ok(list) = timeline_item.downcast::<pyo3::types::PyList>() {
                for item in list.iter() {
                    let item_dict: &Bound<'_, PyDict> = item.downcast()?;
                    let session = crate::plugins::models::session::session_from_dict_internal(
                        item_dict, date, timezone,
                    )?;
                    timeline.push(session.inner);
                }
            }
        }

        // Extract signatures
        let signatures: HashMap<String, HashMap<String, String>> = dict
            .get_item("signatures")?
            .map(|v| v.extract())
            .transpose()?
            .unwrap_or_default();

        // Extract meta
        let meta_item = dict
            .get_item("meta")?
            .ok_or_else(|| PyValueError::new_err("Missing 'meta' field"))?;
        let meta_dict: &Bound<'_, PyDict> = meta_item.downcast()?;
        let meta = PyTimesheetMeta::from_dict(_cls, meta_dict)?;

        Ok(Self {
            inner: RustTimesheet::new(
                actor, date, compiled, timezone, timeline, signatures, meta.inner,
            ),
        })
    }
}

/// The Python-visible SubmittableTimesheet class
#[pyclass(name = "SubmittableTimesheet")]
#[derive(Clone)]
pub struct PySubmittableTimesheet {
    pub inner: RustSubmittableTimesheet,
}

#[pymethods]
impl PySubmittableTimesheet {
    #[getter]
    fn actor(&self) -> HashMap<String, String> {
        self.inner.actor.clone()
    }

    #[getter]
    fn date<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, pyo3::types::PyDate>> {
        type_mapping::date_rust_to_py(py, &self.inner.date)
    }

    #[getter]
    fn compiled<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyDateTime>> {
        type_mapping::datetime_rust_to_py(py, &self.inner.compiled)
    }

    #[getter]
    fn timezone<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, pyo3::types::PyAny>> {
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

    #[getter]
    fn signatures(&self) -> HashMap<String, HashMap<String, String>> {
        self.inner.signatures.clone()
    }

    fn canonical_form<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyBytes>> {
        let bytes = self
            .inner
            .canonical_form()
            .map_err(|e| PyValueError::new_err(e.to_string()))?;

        Ok(PyBytes::new(py, &bytes))
    }
}
