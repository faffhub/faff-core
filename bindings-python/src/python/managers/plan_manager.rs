use pyo3::prelude::*;
use pyo3::types::{PyDate, PyDict, PyList};
use std::sync::Arc;

use crate::python::storage::PyStorage;
use faff_core::managers::plan_manager::PlanManager as RustPlanManager;
use faff_core::plugins::models::intent::PyIntent;
use faff_core::plugins::models::plan::PyPlan;
use faff_core::utils::type_mapping::date_py_to_rust;
use faff_core::workspace::Workspace as RustWorkspace;

/// Python wrapper for PlanManager
#[pyclass(name = "PlanManager")]
#[derive(Clone)]
pub struct PyPlanManager {
    manager: Arc<RustPlanManager>,
    workspace: Option<Arc<RustWorkspace>>,
}

impl PyPlanManager {
    /// Create from a Rust PlanManager
    pub fn from_rust(manager: RustPlanManager) -> Self {
        Self {
            manager: Arc::new(manager),
            workspace: None,
        }
    }

    /// Create from an Arc-wrapped Rust PlanManager with workspace reference
    pub fn from_rust_arc(manager: Arc<RustPlanManager>, workspace: Arc<RustWorkspace>) -> Self {
        Self {
            manager,
            workspace: Some(workspace),
        }
    }
}

#[pymethods]
impl PyPlanManager {
    #[new]
    pub fn new(storage: Py<PyAny>) -> PyResult<Self> {
        let py_storage = PyStorage::new(storage);
        let manager = RustPlanManager::new(Arc::new(py_storage));
        Ok(Self {
            manager: Arc::new(manager),
            workspace: None, // Standalone construction doesn't have workspace reference
        })
    }

    /// Get all plans valid for a given date
    ///
    /// Returns: dict[str, Plan] - mapping of source names to Plans
    pub fn get_plans(&self, py: Python, date: Bound<'_, PyDate>) -> PyResult<Py<PyAny>> {
        let naive_date = date_py_to_rust(date)?;
        let plans = tokio::runtime::Runtime::new()
            .unwrap()
            .block_on(self.manager.get_plans(naive_date))
            .map_err(|e| pyo3::exceptions::PyRuntimeError::new_err(e.to_string()))?;

        let dict = PyDict::new(py);
        for (source, plan) in plans {
            let py_plan = PyPlan { inner: plan };
            dict.set_item(source, py_plan)?;
        }

        Ok(dict.into())
    }

    /// Get all intents from plans valid for a given date
    ///
    /// Returns: list[Intent]
    pub fn get_intents(&self, py: Python, date: Bound<'_, PyDate>) -> PyResult<Py<PyAny>> {
        let naive_date = date_py_to_rust(date)?;
        let intents = tokio::runtime::Runtime::new()
            .unwrap()
            .block_on(self.manager.get_intents(naive_date))
            .map_err(|e| pyo3::exceptions::PyRuntimeError::new_err(e.to_string()))?;

        let list = PyList::empty(py);
        for intent in intents {
            let py_intent = PyIntent { inner: intent };
            list.append(py_intent)?;
        }

        Ok(list.into())
    }

    /// Get all roles from plans valid for a given date
    ///
    /// Returns: list[str]
    pub fn get_roles(&self, py: Python, date: Bound<'_, PyDate>) -> PyResult<Py<PyAny>> {
        let naive_date = date_py_to_rust(date)?;
        let roles = tokio::runtime::Runtime::new()
            .unwrap()
            .block_on(self.manager.get_roles(naive_date))
            .map_err(|e| pyo3::exceptions::PyRuntimeError::new_err(e.to_string()))?;

        let list = PyList::empty(py);
        for role in roles {
            list.append(role)?;
        }

        Ok(list.into())
    }

    /// Get all objectives from plans valid for a given date
    ///
    /// Returns: list[str]
    pub fn get_objectives(&self, py: Python, date: Bound<'_, PyDate>) -> PyResult<Py<PyAny>> {
        let naive_date = date_py_to_rust(date)?;
        let objectives = tokio::runtime::Runtime::new()
            .unwrap()
            .block_on(self.manager.get_objectives(naive_date))
            .map_err(|e| pyo3::exceptions::PyRuntimeError::new_err(e.to_string()))?;

        let list = PyList::empty(py);
        for objective in objectives {
            list.append(objective)?;
        }

        Ok(list.into())
    }

    /// Get all actions from plans valid for a given date
    ///
    /// Returns: list[str]
    pub fn get_actions(&self, py: Python, date: Bound<'_, PyDate>) -> PyResult<Py<PyAny>> {
        let naive_date = date_py_to_rust(date)?;
        let actions = tokio::runtime::Runtime::new()
            .unwrap()
            .block_on(self.manager.get_actions(naive_date))
            .map_err(|e| pyo3::exceptions::PyRuntimeError::new_err(e.to_string()))?;

        let list = PyList::empty(py);
        for action in actions {
            list.append(action)?;
        }

        Ok(list.into())
    }

    /// Get all subjects from plans valid for a given date
    ///
    /// Returns: list[str]
    pub fn get_subjects(&self, py: Python, date: Bound<'_, PyDate>) -> PyResult<Py<PyAny>> {
        let naive_date = date_py_to_rust(date)?;
        let subjects = tokio::runtime::Runtime::new()
            .unwrap()
            .block_on(self.manager.get_subjects(naive_date))
            .map_err(|e| pyo3::exceptions::PyRuntimeError::new_err(e.to_string()))?;

        let list = PyList::empty(py);
        for subject in subjects {
            list.append(subject)?;
        }

        Ok(list.into())
    }

    /// Get all trackers from plans valid for a given date
    ///
    /// Returns: dict[str, str] - mapping of tracker IDs to names
    pub fn get_trackers(&self, py: Python, date: Bound<'_, PyDate>) -> PyResult<Py<PyAny>> {
        let naive_date = date_py_to_rust(date)?;
        let trackers = tokio::runtime::Runtime::new()
            .unwrap()
            .block_on(self.manager.get_trackers(naive_date))
            .map_err(|e| pyo3::exceptions::PyRuntimeError::new_err(e.to_string()))?;

        let bound = pythonize::pythonize(py, &trackers)
            .map_err(|e| pyo3::exceptions::PyRuntimeError::new_err(e.to_string()))?;
        Ok(bound.unbind())
    }

    /// Get the plan containing a specific tracker ID
    ///
    /// Returns: Plan or None if tracker not found
    pub fn get_plan_by_tracker_id(
        &self,
        tracker_id: &str,
        date: Bound<'_, PyDate>,
    ) -> PyResult<Option<PyPlan>> {
        let naive_date = date_py_to_rust(date)?;
        let plan = tokio::runtime::Runtime::new()
            .unwrap()
            .block_on(self.manager.get_plan_by_tracker_id(tracker_id, naive_date))
            .map_err(|e| pyo3::exceptions::PyRuntimeError::new_err(e.to_string()))?;

        Ok(plan.map(|inner| PyPlan { inner }))
    }

    /// Get the local plan for a given date (returns None if it doesn't exist)
    ///
    /// Returns: Plan or None
    pub fn get_local_plan(&self, date: Bound<'_, PyDate>) -> PyResult<Option<PyPlan>> {
        let naive_date = date_py_to_rust(date)?;
        let plan = tokio::runtime::Runtime::new()
            .unwrap()
            .block_on(self.manager.get_local_plan(naive_date))
            .map_err(|e| pyo3::exceptions::PyRuntimeError::new_err(e.to_string()))?;

        Ok(plan.map(|inner| PyPlan { inner }))
    }

    /// Get the local plan for a given date (creates empty one if it doesn't exist)
    ///
    /// Returns: Plan
    pub fn get_local_plan_or_create(&self, date: Bound<'_, PyDate>) -> PyResult<PyPlan> {
        let naive_date = date_py_to_rust(date)?;
        let plan = tokio::runtime::Runtime::new()
            .unwrap()
            .block_on(self.manager.get_local_plan_or_create(naive_date))
            .map_err(|e| pyo3::exceptions::PyRuntimeError::new_err(e.to_string()))?;

        Ok(PyPlan { inner: plan })
    }

    /// Write a plan to storage
    pub fn write_plan(&self, plan: &PyPlan) -> PyResult<()> {
        tokio::runtime::Runtime::new()
            .unwrap()
            .block_on(self.manager.write_plan(&plan.inner))
            .map_err(|e| pyo3::exceptions::PyRuntimeError::new_err(e.to_string()))
    }

    /// Find an intent by ID across all plan files
    ///
    /// Returns: tuple (source, Intent, plan_file_path) or None if not found
    pub fn find_intent_by_id(&self, py: Python, intent_id: &str) -> PyResult<Option<Py<PyAny>>> {
        let result = tokio::runtime::Runtime::new()
            .unwrap()
            .block_on(self.manager.find_intent_by_id(intent_id))
            .map_err(|e| pyo3::exceptions::PyRuntimeError::new_err(e.to_string()))?;

        match result {
            Some((source, intent, path)) => {
                let py_intent = PyIntent { inner: intent };
                let path_str = path.to_string_lossy().to_string();
                let tuple = (source, py_intent, path_str).into_pyobject(py)?;
                Ok(Some(tuple.unbind().into()))
            }
            None => Ok(None),
        }
    }

    /// Update an intent by ID across all plan files
    ///
    /// Returns: updated Plan or None if intent not found
    pub fn update_intent_by_id(
        &self,
        intent_id: &str,
        updated_intent: &PyIntent,
    ) -> PyResult<Option<PyPlan>> {
        let result = tokio::runtime::Runtime::new()
            .unwrap()
            .block_on(
                self.manager
                    .update_intent_by_id(intent_id, updated_intent.inner.clone()),
            )
            .map_err(|e| pyo3::exceptions::PyRuntimeError::new_err(e.to_string()))?;

        Ok(result.map(|inner| PyPlan { inner }))
    }

    /// Get plan remote plugin instances
    ///
    /// This delegates to the Rust PlanManager's remotes() method.
    pub fn remotes(&self, _py: Python<'_>) -> PyResult<Vec<Py<PyAny>>> {
        let workspace = self.workspace.as_ref().ok_or_else(|| {
            pyo3::exceptions::PyRuntimeError::new_err(
                "PlanManager has no workspace reference. This should not happen.",
            )
        })?;

        tokio::runtime::Runtime::new()
            .unwrap()
            .block_on(self.manager.remotes(workspace.plugins()))
            .map_err(|e| pyo3::exceptions::PyRuntimeError::new_err(e.to_string()))
    }

    /// Replace a field value across all plans
    ///
    /// Returns tuple of (plans_updated, intents_updated)
    pub fn replace_field_in_all_plans(
        &self,
        field: &str,
        old_value: &str,
        new_value: &str,
    ) -> PyResult<(usize, usize)> {
        tokio::runtime::Runtime::new()
            .unwrap()
            .block_on(
                self.manager
                    .replace_field_in_all_plans(field, old_value, new_value),
            )
            .map_err(|e| pyo3::exceptions::PyRuntimeError::new_err(e.to_string()))
    }

    /// Get usage statistics for a field across all plans
    ///
    /// Returns dict of field value -> intent count
    pub fn get_field_usage_stats(&self, field: &str, py: Python<'_>) -> PyResult<Py<PyDict>> {
        let stats = tokio::runtime::Runtime::new()
            .unwrap()
            .block_on(self.manager.get_field_usage_stats(field))
            .map_err(|e| pyo3::exceptions::PyRuntimeError::new_err(e.to_string()))?;

        let dict = PyDict::new(py);
        for (key, value) in stats {
            dict.set_item(key, value)?;
        }
        Ok(dict.into())
    }
}

pub fn register(m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_class::<PyPlanManager>()?;
    Ok(())
}
