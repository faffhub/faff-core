use chrono::{Datelike, NaiveDate};
use pyo3::exceptions::PyValueError;
use pyo3::prelude::*;
use pyo3::types::{PyDate, PyDict, PyList, PyType};
use std::collections::HashMap;

use crate::models::plan::Plan as RustPlan;
use crate::plugins::models::intent::PyIntent;

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
    #[pyo3(signature = (source, valid_from, valid_until=None, roles=vec![], actions=vec![], objectives=vec![], subjects=vec![], trackers=None, intents=vec![]))]
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
        intents: Vec<PyIntent>,
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

        let rust_intents: Vec<_> = intents.into_iter().map(|i| i.inner).collect();

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
                rust_intents,
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
    fn actions(&self) -> Vec<String> {
        self.inner.actions.clone()
    }

    #[getter]
    fn objectives(&self) -> Vec<String> {
        self.inner.objectives.clone()
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
    fn intents(&self) -> Vec<PyIntent> {
        self.inner
            .intents
            .iter()
            .map(|i| PyIntent { inner: i.clone() })
            .collect()
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

        // Extract intents
        let intents = match data.get_item("intents")? {
            Some(intents_item) => {
                let intents_list = intents_item.downcast::<PyList>()?;
                let mut rust_intents = Vec::new();
                for item in intents_list.iter() {
                    let intent_dict = item.downcast::<PyDict>()?;
                    let py_intent =
                        crate::plugins::models::intent::intent_from_dict_internal(intent_dict)?;
                    rust_intents.push(py_intent.inner);
                }
                rust_intents
            }
            None => vec![],
        };

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
                intents,
            ),
        })
    }

    fn id(&self) -> String {
        self.inner.id()
    }

    fn add_intent(&self, intent: PyIntent) -> PyPlan {
        PyPlan {
            inner: self.inner.add_intent(intent.inner),
        }
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
            "Plan(source={:?}, valid_from={}, intents=[{} intents])",
            self.inner.source,
            self.inner.valid_from,
            self.inner.intents.len()
        ))
    }

    fn __str__(&self) -> PyResult<String> {
        self.__repr__()
    }
}
