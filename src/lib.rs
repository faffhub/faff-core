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

    bindings::python::register(&m)?;
    Ok(())
}