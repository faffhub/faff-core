mod bindings;
mod models;

use pyo3::prelude::*;
use pyo3::wrap_pyfunction;

#[pyfunction]
fn hello_world() -> PyResult<String> {
    Ok("Hello from Rust!".to_string())
}

#[pymodule]
fn faff_core(_py: Python, m: Bound<'_, PyModule>) -> PyResult<()> {
    m.add_wrapped(wrap_pyfunction!(hello_world))?;

    let models_mod = PyModule::new(_py, "models")?;
    bindings::register(&models_mod)?;
    m.add_submodule(&models_mod)?;
    Ok(())
}
 