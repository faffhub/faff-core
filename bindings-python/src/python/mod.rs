use pyo3::prelude::*;
use pyo3::PyResult;

pub mod events;
pub mod managers;
pub mod query;
pub mod storage;
pub mod workspace;

pub fn register(m: &Bound<'_, PyModule>) -> PyResult<()> {
    let models_mod = PyModule::new(m.py(), "models")?;
    faff_core::plugins::models::config::register(&models_mod)?;
    faff_core::plugins::models::intent::register(&models_mod)?;
    faff_core::plugins::models::session::register(&models_mod)?;
    faff_core::plugins::models::log::register(&models_mod)?;
    faff_core::plugins::models::plan::register(&models_mod)?;
    faff_core::plugins::models::timesheet::register(&models_mod)?;
    faff_core::plugins::models::toy::register(&models_mod)?;
    m.add_submodule(&models_mod)?;

    let managers_mod = PyModule::new(m.py(), "managers")?;
    managers::identity_manager::register(&managers_mod)?;
    managers::log_manager::register(&managers_mod)?;
    managers::plan_manager::register(&managers_mod)?;
    managers::plugin_manager::register(&managers_mod)?;
    managers::timesheet_manager::register(&managers_mod)?;
    m.add_submodule(&managers_mod)?;

    workspace::register(m)?;
    storage::register(m)?;
    query::register(m)?;
    events::register_module(m)?;

    Ok(())
}
