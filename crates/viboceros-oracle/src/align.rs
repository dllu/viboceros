//! Actual Align command, with shared source lifecycle and rigid output samples.
use super::*;
use crate::object_layout::{self, ObjectLayoutFixture};
use viboceros_command::{AlignmentOptions, CommandContext};

#[cfg(test)]
mod tests;

#[derive(Clone, Debug, Deserialize, PartialEq)]
pub struct AlignFixture {
    #[serde(flatten)]
    pub layout: ObjectLayoutFixture,
    pub mode: String,
    pub align_to: Option<String>,
    pub target: Option<[f64; 3]>,
}

pub(super) fn run(f: &AlignFixture, tolerance: Tolerance) -> Result<(Value, u64), ProbeError> {
    let coordinates = f.align_to.as_deref().unwrap_or("CPlane");
    let options = AlignmentOptions::default().parse(&["Mode", &f.mode, "AlignTo", coordinates])?;
    let mut command = options.command_line();
    if let Some(p) = f.target {
        let p = Point3::try_from(p)?;
        command.push_str(&format!(" {},{},{}", p.x(), p.y(), p.z()));
    }
    object_layout::run(&f.layout, tolerance, 1, |document, construction_plane| {
        let registry = CommandRegistry::with_builtins();
        let context = CommandContext { construction_plane };
        if f.layout.preselect {
            registry.execute_in_context(document, &command, context)?;
        } else {
            registry.execute_postselected(document, &command, context)?;
        }
        Ok(true)
    })
}
