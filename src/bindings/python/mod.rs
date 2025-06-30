use pyo3::prelude::*;
use pyo3::PyResult;

pub mod intent;

pub fn register(m: &Bound<'_, PyModule>) -> PyResult<()> {
    let models_mod = PyModule::new(m.py(), "models")?;
    intent::register(&models_mod)?;
    m.add_submodule(&models_mod)?;
    Ok(())
}