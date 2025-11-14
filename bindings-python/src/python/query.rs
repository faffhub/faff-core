use pyo3::prelude::*;
use pyo3::types::PyDate;

use faff_core::utils::query::{Filter as RustFilter, FilterError, FilterField, FilterOperator};
use faff_core::utils::type_mapping::date_py_to_rust;

/// Python wrapper for Rust Filter
#[pyclass(name = "Filter")]
#[derive(Clone)]
pub struct PyFilter {
    inner: RustFilter,
}

#[pymethods]
impl PyFilter {
    /// Parse a filter from a string
    ///
    /// Format: key=value, key~value, or key!=value
    ///
    /// Supported keys: alias, role, objective, action, subject, note
    /// Operators:
    ///   - = : equals
    ///   - ~ : contains (case-insensitive)
    ///   - != : not equals
    ///
    /// Examples:
    ///   - "role=engineer"
    ///   - "objective~planning"
    ///   - "note!=standup"
    #[staticmethod]
    fn parse(filter_str: &str) -> PyResult<Self> {
        let inner: RustFilter = filter_str
            .parse()
            .map_err(|e: FilterError| pyo3::exceptions::PyValueError::new_err(e.to_string()))?;
        Ok(Self { inner })
    }

    /// Get the field name
    fn field(&self) -> String {
        match self.inner.field {
            FilterField::Alias => "alias".to_string(),
            FilterField::Role => "role".to_string(),
            FilterField::Objective => "objective".to_string(),
            FilterField::Action => "action".to_string(),
            FilterField::Subject => "subject".to_string(),
            FilterField::Note => "note".to_string(),
        }
    }

    /// Get the operator
    fn operator(&self) -> String {
        match self.inner.operator {
            FilterOperator::Equals => "=".to_string(),
            FilterOperator::Contains => "~".to_string(),
            FilterOperator::NotEquals => "!=".to_string(),
        }
    }

    /// Get the filter value
    fn value(&self) -> String {
        self.inner.value.clone()
    }

    fn __repr__(&self) -> String {
        format!(
            "Filter(field='{}', operator='{}', value='{}')",
            self.field(),
            self.operator(),
            self.value()
        )
    }
}

/// Query sessions across multiple logs
///
/// Args:
///     logs: List of Log objects to query
///     filters: List of Filter objects to apply
///     from_date: Optional start date (inclusive)
///     to_date: Optional end date (inclusive)
///
/// Returns:
///     Dictionary where keys are tuples of filter field values and values are durations in seconds
#[pyfunction]
fn query_sessions<'py>(
    py: Python<'py>,
    logs: Vec<faff_core::plugins::models::log::PyLog>,
    filters: Vec<PyFilter>,
    from_date: Option<Bound<'_, PyDate>>,
    to_date: Option<Bound<'_, PyDate>>,
) -> PyResult<Bound<'py, pyo3::types::PyDict>> {
    // Convert Python logs to Rust logs (cloned to own the data)
    let rust_logs: Vec<_> = logs.iter().map(|py_log| py_log.inner.clone()).collect();

    // Convert Python filters to Rust filters (cloned to own the data)
    let rust_filters: Vec<_> = filters
        .iter()
        .map(|py_filter| py_filter.inner.clone())
        .collect();

    // Convert Python dates to Rust dates
    let from_date_rust = from_date.map(|d| date_py_to_rust(d)).transpose()?;
    let to_date_rust = to_date.map(|d| date_py_to_rust(d)).transpose()?;

    // Call Rust query function
    let results = faff_core::utils::query::query_sessions(
        &rust_logs,
        &rust_filters,
        from_date_rust,
        to_date_rust,
    )
    .map_err(|e| pyo3::exceptions::PyRuntimeError::new_err(e.to_string()))?;

    // Convert to Python dict with tuple keys (tuples are hashable, lists are not)
    let py_dict = pyo3::types::PyDict::new(py);
    for (key_vec, duration) in results {
        let key_tuple = pyo3::types::PyTuple::new(py, key_vec.iter())?;
        py_dict.set_item(key_tuple, duration.num_seconds())?;
    }

    Ok(py_dict)
}

pub fn register(m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_class::<PyFilter>()?;
    m.add_function(wrap_pyfunction!(query_sessions, m)?)?;
    Ok(())
}
