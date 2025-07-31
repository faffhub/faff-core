use pyo3::prelude::*;
use pyo3::types::{PyDateTime, PyDateAccess};
use crate::models::Toy as RustToy;
use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};

#[pyclass(name = "Toy")]
pub struct PyToy {
    pub inner: RustToy,
}

pub fn register(m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_class::<PyToy>()?;
    Ok(())
}

#[pymethods]
impl PyToy {
   #[new]
    pub fn new(word: String) -> PyResult<Self> {
        Ok(Self {
            inner: RustToy {
                word: word.clone(),
            },
        })
    }

    #[getter]
    fn word(&self) -> String {
        self.inner.word.clone()
    }

    fn hello(&self) -> PyResult<String> {
        self.inner.hello().map_err(|e| PyErr::new::<pyo3::exceptions::PyIOError, _>(e.to_string()))
    }

    fn __eq__(&self, other: PyRef<PyToy>) -> PyResult<bool> {
        println!("I'm __eq__ing!");
        Ok(self.inner == other.inner)
    }

    fn __ne__(&self, other: PyRef<PyToy>) -> PyResult<bool> {
        self.__eq__(other).map(|eq| !eq)
    }



    //fn do_a_datetime<'py>(&self, datetime: Bound<'py, PyDateTime>) {
    //    // let dt: Result<Bound<'_, PyDateTime>, PyErr> =
    //   // 
    //    match datetime.extract::<Bound<PyDateTime>>() {
    //        Ok(dt) => {
    //            println!("{}", dt.get_year())
    //        }
    //        Err(err) => {

    //        }
    //    }
    //    let dt: Bound<PyDateTime> = datetime.extract()?;
    //    println!("{}", dt.get_year())
    //    // println!("{}", dt.get_year().to_string());
 
    //}

    /// Python `hash()`
    fn __hash__(&self) -> PyResult<usize> {
        println!("I'm hashing!");
        let mut hasher = DefaultHasher::new();
        self.inner.hash(&mut hasher);
        Ok(hasher.finish() as usize)
    }

    fn toy(&self, word: String) -> PyToy {
        PyToy{inner: self.inner.toy(word)}
    }

}