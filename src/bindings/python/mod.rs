use pyo3::prelude::*;
use pyo3::PyResult;

pub mod type_mapping;
pub mod storage;
pub mod workspace;

pub mod models;
pub mod managers;

pub fn register(m: &Bound<'_, PyModule>) -> PyResult<()> {
    let models_mod = PyModule::new(m.py(), "models")?;
    models::config::register(&models_mod)?;
    models::intent::register(&models_mod)?;
    models::session::register(&models_mod)?;
    models::log::register(&models_mod)?;
    models::plan::register(&models_mod)?;
    models::timesheet::register(&models_mod)?;
    models::toy::register(&models_mod)?;
    m.add_submodule(&models_mod)?;

    let managers_mod = PyModule::new(m.py(), "managers")?;
    managers::identity_manager::register(&managers_mod)?;
    managers::log_manager::register(&managers_mod)?;
    managers::plan_manager::register(&managers_mod)?;
    managers::timesheet_manager::register(&managers_mod)?;
    m.add_submodule(&managers_mod)?;

    workspace::register(m)?;

    Ok(())
}
