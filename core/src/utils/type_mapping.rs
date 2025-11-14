use chrono::{DateTime, Datelike, NaiveDate, TimeZone, Timelike};
use chrono_tz::Tz;
use pyo3::exceptions::PyValueError;
use pyo3::prelude::*;
use pyo3::types::{PyDate, PyDateAccess, PyDateTime, PyTimeAccess, PyTzInfo, PyTzInfoAccess};

pub fn datetime_py_to_rust<'py>(py_dt: Bound<'py, PyDateTime>) -> PyResult<DateTime<Tz>> {
    // Extract datetime components
    let year = py_dt.get_year();
    let month = py_dt.get_month() as u32;
    let day = py_dt.get_day() as u32;
    let hour = py_dt.get_hour() as u32;
    let minute = py_dt.get_minute() as u32;
    let second = py_dt.get_second() as u32;
    let micro = py_dt.get_microsecond();

    // Get tzinfo
    let tzinfo = py_dt
        .get_tzinfo()
        .ok_or_else(|| PyValueError::new_err("Expected timezone-aware datetime"))?;

    // Get the tzinfo name (prefer `.zone` or `.key`, fallback to str(tzinfo))
    let tz_name: String = if let Ok(attr) = tzinfo.getattr("key") {
        attr.extract()?
    } else if let Ok(attr) = tzinfo.getattr("zone") {
        attr.extract()?
    } else {
        tzinfo.str()?.to_str()?.to_owned()
    };

    // Convert tz_name to chrono_tz::Tz
    let tz: Tz = tz_name
        .parse()
        .map_err(|_| PyValueError::new_err(format!("Unrecognized timezone '{tz_name}'")))?;

    // Build datetime in Rust
    tz.with_ymd_and_hms(year, month, day, hour, minute, second)
        .single()
        .ok_or_else(|| PyValueError::new_err("Ambiguous or invalid datetime"))?
        .with_nanosecond(micro * 1000)
        .ok_or_else(|| PyValueError::new_err("Invalid microseconds"))
}

pub fn datetime_rust_to_py<'py>(
    py: Python<'py>,
    dt: &DateTime<Tz>,
) -> PyResult<Bound<'py, PyDateTime>> {
    // Get the timezone name from chrono_tz (e.g. "Europe/London")
    let tz_name = dt.timezone().name();

    // Import zoneinfo and construct a ZoneInfo object
    let zoneinfo_module = py.import("zoneinfo")?;
    let zoneinfo_obj = zoneinfo_module.call_method1("ZoneInfo", (tz_name,))?;
    let zoneinfo = zoneinfo_obj.downcast::<PyTzInfo>()?;

    // Construct the Python datetime.datetime with tzinfo
    PyDateTime::new_with_fold(
        py,
        dt.year(),
        dt.month() as u8,
        dt.day() as u8,
        dt.hour() as u8,
        dt.minute() as u8,
        dt.second() as u8,
        dt.timestamp_subsec_micros(),
        Some(zoneinfo),
        false,
    )
}

pub fn date_py_to_rust<'py>(py_date: Bound<'py, PyDate>) -> PyResult<NaiveDate> {
    let date_str: String = py_date.call_method0("isoformat")?.extract()?;
    NaiveDate::parse_from_str(&date_str, "%Y-%m-%d")
        .map_err(|e| PyValueError::new_err(e.to_string()))
}

pub fn date_rust_to_py<'py>(py: Python<'py>, date: &NaiveDate) -> PyResult<Bound<'py, PyDate>> {
    PyDate::new(py, date.year(), date.month() as u8, date.day() as u8)
}
