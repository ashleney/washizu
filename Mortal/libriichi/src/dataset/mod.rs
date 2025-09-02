//! Sample extractions.
pub mod gameplay;
pub mod grp;
pub mod invisible;
use crate::py_helper::add_submodule;
pub use gameplay::{Gameplay, GameplayLoader};
pub use grp::Grp;
pub use invisible::Invisible;
use pyo3::prelude::*;
pub fn register_module(
    py: Python<'_>,
    prefix: &str,
    super_mod: &Bound<'_, PyModule>,
) -> PyResult<()> {
    let m = PyModule::new(py, "dataset")?;
    m.add_class::<Gameplay>()?;
    m.add_class::<GameplayLoader>()?;
    m.add_class::<Grp>()?;
    add_submodule(py, prefix, super_mod, &m)
}
