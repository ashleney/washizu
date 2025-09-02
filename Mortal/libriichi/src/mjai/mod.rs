pub mod bot;
pub mod event;
pub use event::{Event, EventExt, EventWithCanAct, Metadata, OutOfBoundError};
use crate::py_helper::add_submodule;
use bot::Bot;
use pyo3::prelude::*;
pub fn register_module(
    py: Python<'_>,
    prefix: &str,
    super_mod: &Bound<'_, PyModule>,
) -> PyResult<()> {
    let m = PyModule::new(py, "mjai")?;
    m.add_class::<Bot>()?;
    add_submodule(py, prefix, super_mod, &m)
}
