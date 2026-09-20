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
    #[serde(default)]
    pub references: Vec<[f64; 3]>,
    #[serde(default)]
    pub three_point: bool,
}

pub(super) fn run(f: &AlignFixture, tolerance: Tolerance) -> Result<(Value, u64), ProbeError> {
    let coordinates = f.align_to.as_deref().unwrap_or("CPlane");
    let options = AlignmentOptions::default().parse(&["Mode", &f.mode, "AlignTo", coordinates])?;
    let mut command = options.command_line();
    if f.three_point {
        command.push_str(" 3Point");
    }
    if let Some(p) = f.target {
        let p = Point3::try_from(p)?;
        command.push_str(&format!(" {},{},{}", p.x(), p.y(), p.z()));
    }
    for p in &f.references {
        let p = Point3::try_from(*p)?;
        command.push_str(&format!(" {},{},{}", p.x(), p.y(), p.z()));
    }
    // Validate fixture shape before building a document, including inappropriate
    // target/reference fields that otherwise look like ordinary command points.
    if options.projects() && f.target.is_some()
        || options.reference_count() == 0 && !f.references.is_empty()
    {
        return Err(ProbeError::FixtureInvariant(
            "invalid alignment target/references",
        ));
    }
    object_layout::run(&f.layout, tolerance, 1, |document, construction_plane| {
        let registry = CommandRegistry::with_builtins();
        let context = CommandContext { construction_plane };
        let result = if f.layout.preselect {
            registry.execute_in_context(document, &command, context)
        } else {
            registry.execute_postselected(document, &command, context)
        };
        match result {
            Ok(_) => Ok(true),
            Err(CommandError::InsufficientPlaneAlignmentObjects { .. }) => Ok(false),
            Err(error) => Err(error.into()),
        }
    })
}
