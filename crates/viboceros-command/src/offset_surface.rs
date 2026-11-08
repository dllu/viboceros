//! Exact normal offsets of planar and canonical analytic surfaces.

use super::*;
mod analytic;
mod geometry;
#[cfg(feature = "native-smlib")]
mod native;
mod options;
mod prepare;
mod through_point;
mod zero;
pub use options::{Input, Options, distance, parse};
pub use prepare::{Prepared, prepare};
pub use through_point::through_distance;
#[cfg(any(not(feature = "native-smlib"), test))]
mod planar;
#[cfg(test)]
mod tests;
use analytic::*;
#[cfg(any(not(feature = "native-smlib"), test))]
use planar::*;

const USAGE: &str = "OffsetSrf [distance|Distance=value] [Corner=Sharp] [ThroughPoint=x,y,z] [Solid=Yes|No] [BothSides=Yes|No] [DeleteInput=Yes|No] [FlipAll]";

pub(super) struct OffsetSurfaceCommand(pub std::sync::Arc<remembered::Remembered<Options>>);

impl Command for OffsetSurfaceCommand {
    fn name(&self) -> &'static str {
        "OffsetSrf"
    }
    fn run(&self, document: &mut Document, arguments: &[&str]) -> Result<String, CommandError> {
        self.execute(document, arguments, false)
    }
    fn run_postselected(
        &self,
        document: &mut Document,
        arguments: &[&str],
        _context: CommandContext,
    ) -> Result<String, CommandError> {
        self.execute(document, arguments, true)
    }

    fn object_selection_prompt(
        &self,
        arguments: &[&str],
    ) -> Result<Option<ObjectSelectionPrompt>, CommandError> {
        parse(
            arguments,
            self.0.get(),
            &viboceros_geometry::LengthUnitSystem::Millimeters,
        )?;
        Ok(Some(ObjectSelectionPrompt {
            command: self.name(),
            filter: if cfg!(feature = "native-smlib") {
                ObjectSelectionFilter::SurfaceComponents
            } else {
                ObjectSelectionFilter::Surfaces
            },
            workflow: ObjectSelectionWorkflow::ConfirmAfterSelection,
            menus: vec![],
            choices: vec![],
            options: vec![],
        }))
    }
}

impl OffsetSurfaceCommand {
    fn execute(
        &self,
        document: &mut Document,
        arguments: &[&str],
        postselected: bool,
    ) -> Result<String, CommandError> {
        let input = parse(arguments, self.0.get(), document.units())?;
        let mut options = input.options;
        if let Some(point) = input.through_point {
            options.distance =
                through_distance(document, point, options.flip_all, &BTreeSet::new())?;
        }
        self.0.set(options.remembered());
        let prepared = prepare(document, options, &BTreeSet::new())?;
        prepared.apply(document, postselected)
    }
}
