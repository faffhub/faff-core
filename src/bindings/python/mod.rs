use pyo3::prelude::*;
use pyo3::PyResult;

pub mod type_mapping;

pub mod intent;
pub mod session;
pub mod log;
pub mod plan;
pub mod timesheet;
pub mod storage;
pub mod log_manager;
pub mod toy;

pub use intent::PyIntent;
pub use session::PySession;
pub use log::PyLog;
pub use plan::PyPlan;
pub use timesheet::{PyTimesheet, PyTimesheetMeta, PySubmittableTimesheet};
pub use storage::PyStorage;
pub use toy::PyToy;

pub fn register(m: &Bound<'_, PyModule>) -> PyResult<()> {
    let models_mod = PyModule::new(m.py(), "models")?;
    intent::register(&models_mod)?;
    session::register(&models_mod)?;
    log::register(&models_mod)?;
    plan::register(&models_mod)?;
    timesheet::register(&models_mod)?;
    toy::register(&models_mod)?;
    m.add_submodule(&models_mod)?;

    let managers_mod = PyModule::new(m.py(), "managers")?;
    log_manager::register(&managers_mod)?;
    m.add_submodule(&managers_mod)?;

    Ok(())
}