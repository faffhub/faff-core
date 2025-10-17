use pyo3::prelude::*;
use pyo3::types::{PyDate, PyDict, PyList};
use std::sync::Arc;

use crate::bindings::python::models::intent::PyIntent;
use crate::bindings::python::models::plan::PyPlan;
use crate::bindings::python::storage::PyStorage;
use crate::bindings::python::type_mapping::date_py_to_rust;
use crate::managers::plan_manager::PlanManager as RustPlanManager;
use crate::workspace::Workspace as RustWorkspace;

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
        let plans = self
            .manager
            .get_plans(naive_date)
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
        let intents = self
            .manager
            .get_intents(naive_date)
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
        let roles = self
            .manager
            .get_roles(naive_date)
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
        let objectives = self
            .manager
            .get_objectives(naive_date)
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
        let actions = self
            .manager
            .get_actions(naive_date)
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
        let subjects = self
            .manager
            .get_subjects(naive_date)
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
        let trackers = self
            .manager
            .get_trackers(naive_date)
            .map_err(|e| pyo3::exceptions::PyRuntimeError::new_err(e.to_string()))?;

        let bound = pythonize::pythonize(py, &trackers)
            .map_err(|e| pyo3::exceptions::PyRuntimeError::new_err(e.to_string()))?;
        Ok(bound.unbind())
    }

    /// Get the plan containing a specific tracker ID
    ///
    /// Returns: Plan
    pub fn get_plan_by_tracker_id(
        &self,
        tracker_id: &str,
        date: Bound<'_, PyDate>,
    ) -> PyResult<PyPlan> {
        let naive_date = date_py_to_rust(date)?;
        let plan = self
            .manager
            .get_plan_by_tracker_id(tracker_id, naive_date)
            .map_err(|e| pyo3::exceptions::PyRuntimeError::new_err(e.to_string()))?;

        Ok(PyPlan { inner: plan })
    }

    /// Get the local plan for a given date
    ///
    /// Returns: Plan (creates empty one if it doesn't exist)
    pub fn local_plan(&self, date: Bound<'_, PyDate>) -> PyResult<PyPlan> {
        let naive_date = date_py_to_rust(date)?;
        let plan = self
            .manager
            .local_plan(naive_date)
            .map_err(|e| pyo3::exceptions::PyRuntimeError::new_err(e.to_string()))?;

        Ok(PyPlan { inner: plan })
    }

    /// Write a plan to storage
    pub fn write_plan(&self, plan: &PyPlan) -> PyResult<()> {
        self.manager
            .write_plan(&plan.inner)
            .map_err(|e| pyo3::exceptions::PyRuntimeError::new_err(e.to_string()))
    }

    /// Clear the plan cache
    pub fn clear_cache(&self) -> PyResult<()> {
        self.manager.clear_cache();
        Ok(())
    }

    /// Get plan remote plugin instances (delegates to workspace.plugins.plan_remotes())
    pub fn remotes(&self, _py: Python<'_>) -> PyResult<Vec<Py<PyAny>>> {
        let workspace = self.workspace.as_ref()
            .ok_or_else(|| pyo3::exceptions::PyRuntimeError::new_err(
                "PlanManager has no workspace reference. This should not happen."
            ))?;

        let plugin_manager_arc = workspace.plugins();
        let mut plugin_manager = plugin_manager_arc.lock().unwrap();
        plugin_manager.plan_remotes()
            .map_err(|e| pyo3::exceptions::PyRuntimeError::new_err(e.to_string()))
    }
}

pub fn register(m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_class::<PyPlanManager>()?;
    Ok(())
}
