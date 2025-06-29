use pyo3::prelude::*;

mod core;
mod models;

#[pyfunction]
fn hello_world() -> PyResult<String> {
    Ok("Hello from Rust!".to_string())
}

#[pymodule]
fn faff_core(_py: Python, m: Bound<'_, PyModule>) -> PyResult<()> {
    m.add_wrapped(wrap_pyfunction!(hello_world))?;

    let models_mod = PyModule::new(_py, "models")?;
    models::register(&models_mod)?;
    m.add_submodule(&models_mod)?;
    Ok(())
}
