use pyo3::prelude::*;
use pyo3::{create_exception, exceptions::PyException};

// Create custom exception for uninitialized ledger
create_exception!(
    faff_core,
    UninitializedLedgerError,
    PyException,
    "Raised when attempting to use a faff ledger that has not been initialized."
);

pub fn register(m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add(
        "UninitializedLedgerError",
        m.py().get_type::<UninitializedLedgerError>(),
    )?;
    Ok(())
}
