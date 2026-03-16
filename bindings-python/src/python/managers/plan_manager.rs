use pyo3::prelude::*;
use pyo3::types::{PyDate, PyDict, PyList};
use std::sync::Arc;

use crate::python::runtime::runtime;
use crate::python::storage::PyStorage;
use faff_core::managers::plan_manager::PlanManager as RustPlanManager;
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
        let plans = runtime()
            .block_on(self.manager.get_plans(naive_date))
            .map_err(|e| pyo3::exceptions::PyRuntimeError::new_err(e.to_string()))?;

        let dict = PyDict::new(py);
        for (source, plan) in plans {
            let py_plan = PyPlan { inner: plan };
            dict.set_item(source, py_plan)?;
        }

        Ok(dict.into())
    }

    /// Get all roles from plans valid for a given date
    ///
    /// Returns: list[str]
    pub fn get_roles(&self, py: Python, date: Bound<'_, PyDate>) -> PyResult<Py<PyAny>> {
        let naive_date = date_py_to_rust(date)?;
        let roles = runtime()
            .block_on(self.manager.get_roles(naive_date))
            .map_err(|e| pyo3::exceptions::PyRuntimeError::new_err(e.to_string()))?;

        let list = PyList::empty(py);
        for role in roles {
            list.append(role)?;
        }

        Ok(list.into())
    }

    /// Get all impacts from plans valid for a given date
    ///
    /// Returns: list[str]
    pub fn get_impacts(&self, py: Python, date: Bound<'_, PyDate>) -> PyResult<Py<PyAny>> {
        let naive_date = date_py_to_rust(date)?;
        let impacts = runtime()
            .block_on(self.manager.get_impacts(naive_date))
            .map_err(|e| pyo3::exceptions::PyRuntimeError::new_err(e.to_string()))?;

        let list = PyList::empty(py);
        for impact in impacts {
            list.append(impact)?;
        }

        Ok(list.into())
    }

    /// Get all modes from plans valid for a given date
    ///
    /// Returns: list[str]
    pub fn get_modes(&self, py: Python, date: Bound<'_, PyDate>) -> PyResult<Py<PyAny>> {
        let naive_date = date_py_to_rust(date)?;
        let modes = runtime()
            .block_on(self.manager.get_modes(naive_date))
            .map_err(|e| pyo3::exceptions::PyRuntimeError::new_err(e.to_string()))?;

        let list = PyList::empty(py);
        for mode in modes {
            list.append(mode)?;
        }

        Ok(list.into())
    }

    /// Get all subjects from plans valid for a given date
    ///
    /// Returns: list[str]
    pub fn get_subjects(&self, py: Python, date: Bound<'_, PyDate>) -> PyResult<Py<PyAny>> {
        let naive_date = date_py_to_rust(date)?;
        let subjects = runtime()
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
        let trackers = runtime()
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
        let plan = runtime()
            .block_on(self.manager.get_plan_by_tracker_id(tracker_id, naive_date))
            .map_err(|e| pyo3::exceptions::PyRuntimeError::new_err(e.to_string()))?;

        Ok(plan.map(|inner| PyPlan { inner }))
    }

    /// Get the local plan for a given date (returns None if it doesn't exist)
    ///
    /// Returns: Plan or None
    pub fn get_local_plan(&self, date: Bound<'_, PyDate>) -> PyResult<Option<PyPlan>> {
        let naive_date = date_py_to_rust(date)?;
        let plan = runtime()
            .block_on(self.manager.get_local_plan(naive_date))
            .map_err(|e| pyo3::exceptions::PyRuntimeError::new_err(e.to_string()))?;

        Ok(plan.map(|inner| PyPlan { inner }))
    }

    /// Get the local plan for a given date (creates empty one if it doesn't exist)
    ///
    /// Returns: Plan
    pub fn get_local_plan_or_create(&self, date: Bound<'_, PyDate>) -> PyResult<PyPlan> {
        let naive_date = date_py_to_rust(date)?;
        let plan = runtime()
            .block_on(self.manager.get_local_plan_or_create(naive_date))
            .map_err(|e| pyo3::exceptions::PyRuntimeError::new_err(e.to_string()))?;

        Ok(PyPlan { inner: plan })
    }

    /// Write a plan to storage
    pub fn write_plan(&self, plan: &PyPlan) -> PyResult<()> {
        runtime()
            .block_on(self.manager.write_plan(&plan.inner))
            .map_err(|e| pyo3::exceptions::PyRuntimeError::new_err(e.to_string()))
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

        runtime()
            .block_on(self.manager.remotes(workspace.plugins()))
            .map_err(|e| pyo3::exceptions::PyRuntimeError::new_err(e.to_string()))
    }

    /// Replace a field value across all plans
    ///
    /// Returns number of plans updated
    pub fn replace_field_in_all_plans(
        &self,
        field: &str,
        old_value: &str,
        new_value: &str,
    ) -> PyResult<usize> {
        runtime()
            .block_on(
                self.manager
                    .replace_field_in_all_plans(field, old_value, new_value),
            )
            .map_err(|e| pyo3::exceptions::PyRuntimeError::new_err(e.to_string()))
    }

    /// Get session hints from plans valid for a given date
    ///
    /// Returns: list[dict] with keys: title, role, subject, impact, mode (Optional[str]),
    /// trackers (list[str])
    pub fn get_session_hints(
        &self,
        py: Python,
        date: Bound<'_, PyDate>,
    ) -> PyResult<Py<PyAny>> {
        let naive_date = date_py_to_rust(date)?;
        let hints = runtime()
            .block_on(self.manager.get_session_hints(naive_date))
            .map_err(|e| pyo3::exceptions::PyRuntimeError::new_err(e.to_string()))?;

        let list = PyList::empty(py);
        for hint in hints {
            let dict = PyDict::new(py);
            dict.set_item("title", &hint.title)?;
            dict.set_item("role", hint.role.as_deref().into_pyobject(py)?)?;
            dict.set_item("subject", hint.subject.as_deref().into_pyobject(py)?)?;
            dict.set_item("impact", hint.impact.as_deref().into_pyobject(py)?)?;
            dict.set_item("mode", hint.mode.as_deref().into_pyobject(py)?)?;
            dict.set_item("trackers", PyList::new(py, &hint.trackers)?)?;
            list.append(dict)?;
        }
        Ok(list.into())
    }

    /// Get tracker mappings for plans valid for a given date
    ///
    /// Returns: list[dict] with keys: tracker_id, tracker_name, hint_title, role, subject, impact, mode
    /// Fields role/subject/impact/mode are None when not constrained by the mapping.
    pub fn get_tracker_mappings(
        &self,
        py: Python,
        date: Bound<'_, PyDate>,
    ) -> PyResult<Py<PyAny>> {
        let naive_date = date_py_to_rust(date)?;
        let mappings = runtime()
            .block_on(self.manager.get_tracker_mappings(naive_date))
            .map_err(|e| pyo3::exceptions::PyRuntimeError::new_err(e.to_string()))?;

        let list = PyList::empty(py);
        for mapping in mappings {
            let dict = PyDict::new(py);
            dict.set_item("tracker_id", &mapping.tracker_id)?;
            dict.set_item("tracker_name", &mapping.tracker_name)?;
            dict.set_item("hint_title", &mapping.hint_title)?;
            dict.set_item("role", mapping.role.as_deref().into_pyobject(py)?)?;
            dict.set_item("subject", mapping.subject.as_deref().into_pyobject(py)?)?;
            dict.set_item("impact", mapping.impact.as_deref().into_pyobject(py)?)?;
            dict.set_item("mode", mapping.mode.as_deref().into_pyobject(py)?)?;
            list.append(dict)?;
        }

        Ok(list.into())
    }

    /// Get all plan-derived data needed at session start time in a single call.
    ///
    /// Returns a dict with keys: roles, impacts, modes, subjects, trackers,
    /// hints (list[dict]), tracker_mappings (list[dict])
    pub fn get_start_data(&self, py: Python, date: Bound<'_, PyDate>) -> PyResult<Py<PyAny>> {
        let naive_date = date_py_to_rust(date)?;
        let data = runtime()
            .block_on(self.manager.get_start_data(naive_date))
            .map_err(|e| pyo3::exceptions::PyRuntimeError::new_err(e.to_string()))?;

        let dict = PyDict::new(py);

        // Simple lists
        dict.set_item("roles", data.roles)?;
        dict.set_item("impacts", data.impacts)?;
        dict.set_item("modes", data.modes)?;
        dict.set_item("subjects", data.subjects)?;

        // Trackers dict
        let bound = pythonize::pythonize(py, &data.trackers)
            .map_err(|e| pyo3::exceptions::PyRuntimeError::new_err(e.to_string()))?;
        dict.set_item("trackers", bound)?;

        // Hints
        let hints_list = PyList::empty(py);
        for hint in data.hints {
            let h = PyDict::new(py);
            h.set_item("title", &hint.title)?;
            h.set_item("role", hint.role.as_deref().into_pyobject(py)?)?;
            h.set_item("subject", hint.subject.as_deref().into_pyobject(py)?)?;
            h.set_item("impact", hint.impact.as_deref().into_pyobject(py)?)?;
            h.set_item("mode", hint.mode.as_deref().into_pyobject(py)?)?;
            h.set_item("trackers", PyList::new(py, &hint.trackers)?)?;
            hints_list.append(h)?;
        }
        dict.set_item("hints", hints_list)?;

        // Tracker mappings
        let mappings_list = PyList::empty(py);
        for mapping in data.tracker_mappings {
            let m = PyDict::new(py);
            m.set_item("tracker_id", &mapping.tracker_id)?;
            m.set_item("tracker_name", &mapping.tracker_name)?;
            m.set_item("hint_title", &mapping.hint_title)?;
            m.set_item("role", mapping.role.as_deref().into_pyobject(py)?)?;
            m.set_item("subject", mapping.subject.as_deref().into_pyobject(py)?)?;
            m.set_item("impact", mapping.impact.as_deref().into_pyobject(py)?)?;
            m.set_item("mode", mapping.mode.as_deref().into_pyobject(py)?)?;
            mappings_list.append(m)?;
        }
        dict.set_item("tracker_mappings", mappings_list)?;

        Ok(dict.into())
    }

    /// Get usage statistics for a field across all plans
    ///
    /// Returns dict of field value -> count
    pub fn get_field_usage_stats(&self, field: &str, py: Python<'_>) -> PyResult<Py<PyDict>> {
        let stats = runtime()
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
