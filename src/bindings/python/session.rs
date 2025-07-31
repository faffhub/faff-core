use pyo3::prelude::*;
use pyo3::types::{PyDict, PyType};
use std::collections::HashMap;
use pyo3::types::{PyDateTime, PyDateAccess, PyTimeAccess, PyTzInfoAccess, PyTzInfo};
use crate::bindings::python::intent::PyIntent;
use crate::models::{Session as RustSession, valuetype::ValueType};
use chrono::{NaiveDate, DateTime};
use chrono_tz::Tz;
use chrono::TimeZone;
use chrono::Timelike;
use pyo3::exceptions::PyValueError;
use pyo3::types::PyTypeMethods;


fn datetime_to_py<'py>(py: Python<'py>, dt: &DateTime<Tz>) -> PyResult<Bound<'py, PyAny>> {
    let iso = dt.to_rfc3339();
    py.import("pendulum")?
        .call_method1("parse", (iso,))
}

fn get_timezone(tzinfo: &Bound<'_, PyTzInfo>) -> PyResult<Tz> {
    let tzname: String = tzinfo.getattr("key")?.extract()?;
    tzname
        .to_string()
        .parse::<Tz>()
        .map_err(|_| PyErr::new::<pyo3::exceptions::PyValueError, _>(format!("Unknown timezone: {}", tzname)))
}

fn py_datetime_to_rust_datetime<'py>(py_datetime: Bound<'py, PyDateTime>) -> Result<DateTime<Tz>, PyErr> {
    // Extract datetime components from PyDateTime
    let year = py_datetime.get_year();
    let month = py_datetime.get_month() as u32;
    let day = py_datetime.get_day() as u32;
    let hour = py_datetime.get_hour() as u32;
    let minute = py_datetime.get_minute() as u32;
    let second = py_datetime.get_second() as u32;
    let micro = py_datetime.get_microsecond();

    let tzinfo = py_datetime
        .get_tzinfo()
        .or_else(|| py_datetime.get_tzinfo())
        .ok_or_else(|| PyErr::new::<pyo3::exceptions::PyValueError, _>("No tzinfo found"))?;

    let tz: Tz = get_timezone(&tzinfo)?;

    let dt_tz: DateTime<Tz> = tz
        .with_ymd_and_hms(year.into(), month, day, hour, minute, second)
        .single()
        .ok_or_else(|| PyValueError::new_err("Invalid date or ambiguous due to DST"))?
        .with_nanosecond(micro * 1000)
        .ok_or_else(|| PyValueError::new_err("Invalid microseconds"))?;

    Ok(dt_tz)
}

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

#[pyfunction]
fn frobnicate<'py>(value: &Bound<'py, PyAny>) -> PyResult<Bound<'py, PyAny>> {
    // Use `downcast` instead of `extract` as turning `PyDowncastError` into `PyErr` is quite costly.
    if let Ok(list) = value.downcast::<PyList>() {
        frobnicate_list(list)
    } else if let Ok(vec) = value.extract::<Vec<Bound<'_, PyAny>>>() {
        frobnicate_vec(vec)
    } else {
        Err(PyTypeError::new_err("Cannot frobnicate that type."))
    }
}

#[pymethods]
impl PySession {
    #[new]
    fn py_new<'py>(
        intent: PyIntent,
        start: Bound<'py, PyDateTime>,
        end: Option<Bound<'py, PyDateTime>>,
        note: Option<String>,
    ) -> PyResult<Self> {
        let start = py_datetime_to_rust_datetime(start)?;
        let end = match end {
            Some(end_dt) => Some(py_datetime_to_rust_datetime(end_dt)?),
            None => None,
        };
        Ok(Self {
            inner: RustSession::new(intent.inner, start, end, note),
        })
    }

    fn __getstate__(&self, py: Python<'_>) -> PyResult<PyObject> {
        let dict = PyDict::new(py);

        dict.set_item("intent", Py::new(py, PyIntent { inner: self.inner.intent.clone() })?)?;
        dict.set_item("start", self.inner.start.to_rfc3339())?;
        if let Some(end) = &self.inner.end {
            dict.set_item("end", end.to_rfc3339())?;
        }
        if let Some(note) = &self.inner.note {
            dict.set_item("note", note)?;
        }

        Ok(dict.unbind().into())
    }

    fn frobnicate<'py>(value: &Bound<'py, PyAny>) -> PyResult<Bound<'py, PyAny>> {
        // Use `downcast` instead of `extract` as turning `PyDowncastError` into `PyErr` is quite costly.
        if let Ok(list) = value.downcast::<PyList>() {
            frobnicate_list(list)
        } else if let Ok(vec) = value.extract::<Vec<Bound<'_, PyAny>>>() {
            frobnicate_vec(vec)
        } else {
            Err(PyTypeError::new_err("Cannot frobnicate that type."))
        }
    }

    #[classmethod]
    fn __setstate__<'py>(_cls: &Bound<'py, PyType>, state: &Bound<'py, PyAny>) -> PyResult<Self> {
        let dict = state.downcast::<PyDict>()?;

        let intent: PyIntent = dict
            .get_item("intent")?
            .ok_or_else(|| PyErr::new::<pyo3::exceptions::PyValueError, _>("Missing 'intent'"))?
            .extract()?;

        let start: String = dict
            .get_item("start")?
            .ok_or_else(|| PyErr::new::<pyo3::exceptions::PyValueError, _>("Missing 'start'"))?
            .extract()?;
        let start = start.parse::<DateTime<Tz>>()
            .map_err(|_| PyValueError::new_err("Invalid start datetime"))?;

        let end = if let Some(end_val) = dict.get_item("end") {
            Some(end_val.extract::<String>()?.parse::<DateTime<Tz>>()
                .map_err(|_| PyValueError::new_err("Invalid end datetime"))?)
        } else {
            None
        };

        let note = dict.get_item("note").and_then(|v| v.extract::<String>().ok());

        Ok(Self {
            inner: RustSession::new(intent.inner, start, end, note),
        })
    }



    //#[new]
    //#[pyo3(signature = (intent, start, note))]
    //fn py_new<'py>(intent: PyIntent, start: Bound<'py, PyDateTime>, note: Option<String>) -> PyResult<Self> {
    //    let start = py_datetime_to_rust_datetime(start)?;
    //    Ok(Self {
    //        inner: RustSession::new(intent.inner, start, note),
    //    })
    //}

    #[getter]
    fn intent(&self) -> PyIntent {
        PyIntent { inner: self.inner.intent.clone() }
    }

    #[getter]
    fn start<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        datetime_to_py(py, &self.inner.start)
    }

    #[getter]
    fn end<'py>(&self, py: Python<'py>) -> PyResult<Option<Bound<'py, PyAny>>> {
        match &self.inner.end {
            Some(dt) => Ok(Some(datetime_to_py(py, dt)?)),
            None => Ok(None),
        }
    }

    #[getter]
    fn note(&self) -> Option<String> {
        self.inner.note.clone()
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
            } else {
                return Err(pyo3::exceptions::PyValueError::new_err(format!(
                    "Unsupported type for key '{}'", key
                )));
            }
        }
        let date_str: String = date
            .call_method0("isoformat")?
            .extract()?;

        let date = NaiveDate::parse_from_str(&date_str, "%Y-%m-%d")
            .map_err(|e| pyo3::exceptions::PyValueError::new_err(e.to_string()))?;

        let tz_str: String = tz
            .call_method0("__str__")?
            .extract()?;
        let tz = tz_str.parse::<Tz>()
            .map_err(|e| pyo3::exceptions::PyValueError::new_err(e.to_string()))?;
     
        let inner = RustSession::from_dict_with_tz(data, date, tz)
            .map_err(|e| pyo3::exceptions::PyValueError::new_err(e))?;

        Ok(Self { inner })
    }

    fn with_end<'py>(&self, end: Bound<'py, PyDateTime>) -> PyResult<PySession> {
        let dt_tz = py_datetime_to_rust_datetime(end)?;
        Ok(PySession {
            inner: self.inner.with_end(dt_tz),
        })
    }   

    fn as_dict(&self) -> PyResult<Py<PyDict>> {
        Python::with_gil(|py| {
            let d = PyDict::new(py);

            let intent = &self.inner.intent;
            d.set_item("intent", PyIntent { inner: intent.clone() })?;

            let start = &self.inner.start;
            d.set_item("start", datetime_to_py(py, start)?)?;
            
            if let Some(end) = &self.inner.end {
                d.set_item("end", datetime_to_py(py, end)?)?;
            }
            if let Some(note) = &self.inner.note {
                d.set_item("note", note)?;
            }
            Ok(d.into())
        })
    }

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
    //fn __reduce__(&self, py: Python) -> PyResult<(PyObject, (Option<String>, Option<String>, Option<String>, Option<String>, Option<String>, Vec<String>))> {
    //    let intent_type = py.get_type::<Self>();
    //    Ok((
    //        intent_type.into(),
    //        (
    //            self.inner.alias.clone(),
    //            self.inner.role.clone(),
    //            self.inner.objective.clone(),
    //            self.inner.action.clone(),
    //            self.inner.subject.clone(),
    //            self.inner.trackers.clone(),
    //        )
    //    ))
    //}

}