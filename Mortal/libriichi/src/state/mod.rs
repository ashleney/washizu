pub mod action;
pub mod agent_helper;
pub mod getter;
pub mod item;
pub mod obs_repr;
pub mod player_state;
pub mod sp_tables;
pub mod update;
#[cfg(test)]
pub mod test;
use crate::py_helper::add_submodule;
pub use action::ActionCandidate;
pub use player_state::PlayerState;
pub use sp_tables::SinglePlayerTables;
use pyo3::prelude::*;
pub fn register_module(
    py: Python<'_>,
    prefix: &str,
    super_mod: &Bound<'_, PyModule>,
) -> PyResult<()> {
    let m = PyModule::new(py, "state")?;
    m.add_class::<ActionCandidate>()?;
    m.add_class::<PlayerState>()?;
    add_submodule(py, prefix, super_mod, &m)
}
