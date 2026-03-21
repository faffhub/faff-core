use chrono::{Datelike, NaiveDate};
use pyo3::exceptions::PyValueError;
use pyo3::prelude::*;
use pyo3::types::{PyDate, PyDict, PyType};
use std::collections::HashMap;

use crate::models::plan::{Plan as RustPlan, SessionHint};

#[pyclass(name = "Plan")]
#[derive(Clone)]
pub struct PyPlan {
    pub inner: RustPlan,
}

pub fn register(m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_class::<PyPlan>()?;
    Ok(())
}

#[pymethods]
impl PyPlan {
    #[new]
    #[pyo3(signature = (source, valid_from, valid_until=None, roles=vec![], actions=vec![], objectives=vec![], subjects=vec![], trackers=None))]
    /// Python constructor mirrors struct fields, so many arguments are unavoidable
    #[allow(clippy::too_many_arguments)]
    fn py_new(
        source: String,
        valid_from: Bound<'_, PyDate>,
        valid_until: Option<Bound<'_, PyDate>>,
        roles: Vec<String>,
        actions: Vec<String>,
        objectives: Vec<String>,
        subjects: Vec<String>,
        trackers: Option<HashMap<String, String>>,
    ) -> PyResult<Self> {
        // Convert Python dates to NaiveDate
        let valid_from_str: String = valid_from.call_method0("isoformat")?.extract()?;
        let valid_from_date = NaiveDate::parse_from_str(&valid_from_str, "%Y-%m-%d")
            .map_err(|e| PyValueError::new_err(e.to_string()))?;

        let valid_until_date = if let Some(date) = valid_until {
            let date_str: String = date.call_method0("isoformat")?.extract()?;
            Some(
                NaiveDate::parse_from_str(&date_str, "%Y-%m-%d")
                    .map_err(|e| PyValueError::new_err(e.to_string()))?,
            )
        } else {
            None
        };

        Ok(Self {
            inner: RustPlan::new(
                source,
                valid_from_date,
                valid_until_date,
                roles,
                actions,
                objectives,
                subjects,
                trackers.unwrap_or_default(),
            ),
        })
    }

    #[getter]
    fn source(&self) -> String {
        self.inner.source.clone()
    }

    #[getter]
    fn valid_from<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyDate>> {
        PyDate::new(
            py,
            self.inner.valid_from.year(),
            self.inner.valid_from.month() as u8,
            self.inner.valid_from.day() as u8,
        )
    }

    #[getter]
    fn valid_until<'py>(&self, py: Python<'py>) -> PyResult<Option<Bound<'py, PyDate>>> {
        if let Some(date) = self.inner.valid_until {
            Ok(Some(PyDate::new(
                py,
                date.year(),
                date.month() as u8,
                date.day() as u8,
            )?))
        } else {
            Ok(None)
        }
    }

    #[getter]
    fn roles(&self) -> Vec<String> {
        self.inner.roles.clone()
    }

    #[getter]
    fn modes(&self) -> Vec<String> {
        self.inner.modes.clone()
    }

    #[getter]
    fn impacts(&self) -> Vec<String> {
        self.inner.impacts.clone()
    }

    #[getter]
    fn subjects(&self) -> Vec<String> {
        self.inner.subjects.clone()
    }

    #[getter]
    fn trackers(&self) -> HashMap<String, String> {
        self.inner.trackers.clone()
    }

    #[getter]
    fn hints<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, pyo3::types::PyList>> {
        let list = pyo3::types::PyList::empty(py);
        for hint in &self.inner.hints {
            let dict = pyo3::types::PyDict::new(py);
            dict.set_item("title", &hint.title)?;
            dict.set_item("role", hint.role.as_deref().into_pyobject(py)?)?;
            dict.set_item("subject", hint.subject.as_deref().into_pyobject(py)?)?;
            dict.set_item("impact", hint.impact.as_deref().into_pyobject(py)?)?;
            dict.set_item("mode", hint.mode.as_deref().into_pyobject(py)?)?;
            dict.set_item(
                "trackers",
                pyo3::types::PyList::new(py, &hint.trackers)?,
            )?;
            list.append(dict)?;
        }
        Ok(list)
    }

    /// Return a new Plan with the given hints attached.
    ///
    /// Each hint is a dict with keys: title (str), role, subject, impact, mode
    /// (all Optional[str]), trackers (list[str]).
    fn with_hints(&self, py: Python, hints: Vec<Py<PyAny>>) -> PyResult<PyPlan> {
        let parsed: Vec<SessionHint> = hints
            .iter()
            .map(|h| {
                pythonize::depythonize(h.bind(py))
                    .map_err(|e| pyo3::exceptions::PyValueError::new_err(e.to_string()))
            })
            .collect::<PyResult<_>>()?;
        let mut new_inner = self.inner.clone();
        new_inner.hints = parsed;
        Ok(PyPlan { inner: new_inner })
    }

    #[classmethod]
    fn from_dict(_cls: &Bound<'_, PyType>, data: &Bound<'_, PyDict>) -> PyResult<Self> {
        // Extract source
        let source: String = data.get_item("source")?.unwrap().extract()?;

        // Extract valid_from
        let valid_from_str: String = data.get_item("valid_from")?.unwrap().extract()?;
        let valid_from = NaiveDate::parse_from_str(&valid_from_str, "%Y-%m-%d")
            .map_err(|e| PyValueError::new_err(e.to_string()))?;

        // Extract valid_until
        let valid_until = match data.get_item("valid_until")? {
            Some(item) => {
                let date_str: String = item.extract()?;
                Some(
                    NaiveDate::parse_from_str(&date_str, "%Y-%m-%d")
                        .map_err(|e| PyValueError::new_err(e.to_string()))?,
                )
            }
            None => None,
        };

        // Extract lists (with defaults)
        let roles: Vec<String> = data
            .get_item("roles")?
            .and_then(|item| item.extract().ok())
            .unwrap_or_default();

        let actions: Vec<String> = data
            .get_item("actions")?
            .and_then(|item| item.extract().ok())
            .unwrap_or_default();

        let objectives: Vec<String> = data
            .get_item("objectives")?
            .and_then(|item| item.extract().ok())
            .unwrap_or_default();

        let subjects: Vec<String> = data
            .get_item("subjects")?
            .and_then(|item| item.extract().ok())
            .unwrap_or_default();

        let trackers: HashMap<String, String> = data
            .get_item("trackers")?
            .and_then(|item| item.extract().ok())
            .unwrap_or_default();

        Ok(Self {
            inner: RustPlan::new(
                source,
                valid_from,
                valid_until,
                roles,
                actions,
                objectives,
                subjects,
                trackers,
            ),
        })
    }

    fn id(&self) -> String {
        self.inner.id()
    }

    fn to_toml(&self) -> PyResult<String> {
        self.inner
            .to_toml()
            .map_err(|e| pyo3::exceptions::PyValueError::new_err(e.to_string()))
    }

    fn as_dict(&self) -> PyResult<Py<PyDict>> {
        Python::attach(|py| pythonize::pythonize(py, &self.inner)?.extract())
    }

    fn __repr__(&self) -> PyResult<String> {
        Ok(format!(
            "Plan(source={:?}, valid_from={})",
            self.inner.source,
            self.inner.valid_from,
        ))
    }

    fn __str__(&self) -> PyResult<String> {
        self.__repr__()
    }
}
