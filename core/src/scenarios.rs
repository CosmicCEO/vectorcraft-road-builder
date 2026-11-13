//! The crossing treatments that are not a plain paved box: median opening, mid-block crossing, railroad, cloverleaf.
//! (Filled in scenario by scenario.)

use crate::crossing::{Arm, Crossing, CrossingSpec};
use crate::output::Out;

pub(crate) fn emit(_out: &mut Out, _base: &str, cs: &CrossingSpec, _x: &Crossing, _arms: &[Arm]) -> Result<Vec<String>, String> {
    Err(format!("{} is not built yet", cs.treatment.name()))
}
