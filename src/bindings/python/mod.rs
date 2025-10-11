use pyo3::prelude::*;
use pyo3::PyResult;

pub mod type_mapping;

pub mod intent;
pub mod session;
pub mod log;
pub mod plan;
pub mod toy;

pub use intent::PyIntent;
pub use session::PySession;
pub use log::PyLog;
pub use plan::PyPlan;
pub use toy::PyToy;

pub fn register(m: &Bound<'_, PyModule>) -> PyResult<()> {
    let models_mod = PyModule::new(m.py(), "models")?;
    intent::register(&models_mod)?;
    session::register(&models_mod)?;
    log::register(&models_mod)?;
    plan::register(&models_mod)?;
    toy::register(&models_mod)?;
    m.add_submodule(&models_mod)?;
    Ok(())
}