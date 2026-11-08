//! MergeSrf geometry and command state, staged before document mutation.
use super::*;
mod geometry;
mod options;
#[cfg(not(feature = "native-smlib"))]
mod portable;
mod prepare;
pub use geometry::merge;
pub use options::{Options, parse};
pub use prepare::{Prepared, prepare};

pub const USAGE: &str = "MergeSrf [Smooth=Yes|No] [Tolerance=value] [Roundness=value]";

pub(super) struct MergeSurfaceCommand(pub std::sync::Arc<remembered::Remembered<Options>>);
impl Command for MergeSurfaceCommand {
    fn name(&self) -> &'static str {
        "MergeSrf"
    }
    fn run(&self, document: &mut Document, args: &[&str]) -> Result<String, CommandError> {
        self.execute(document, args, false)
    }
    fn run_postselected(
        &self,
        document: &mut Document,
        args: &[&str],
        _: CommandContext,
    ) -> Result<String, CommandError> {
        self.execute(document, args, true)
    }
    fn object_selection_prompt(
        &self,
        args: &[&str],
    ) -> Result<Option<ObjectSelectionPrompt>, CommandError> {
        let options = parse(
            args,
            self.0.get(),
            &viboceros_geometry::LengthUnitSystem::Millimeters,
        )?;
        Ok(Some(ObjectSelectionPrompt {
            command: "MergeSrf",
            filter: ObjectSelectionFilter::Surfaces,
            workflow: ObjectSelectionWorkflow::OptionsDuringSelection,
            options: vec![BooleanSelectionOption {
                name: "Smooth",
                value: options.smooth,
                aliases: &[],
            }],
            menus: vec![],
            choices: vec![],
        }))
    }
    fn object_selection_complete(
        &self,
        document: &Document,
        _: &[&str],
    ) -> Result<bool, CommandError> {
        Ok(document
            .selected_objects()
            .filter(|o| ObjectSelectionFilter::Surfaces.accepts_object(o))
            .count()
            == 2)
    }
    fn accept_object_selection_options(&self, args: &[&str]) -> Result<(), CommandError> {
        self.0.set(parse(
            args,
            self.0.get(),
            &viboceros_geometry::LengthUnitSystem::Millimeters,
        )?);
        Ok(())
    }
    fn cleanup_failed_selection(
        &self,
        document: &mut Document,
        _: &CommandError,
        postselected: bool,
    ) {
        if postselected {
            document.clear_selection();
        }
    }
}
impl MergeSurfaceCommand {
    fn execute(
        &self,
        document: &mut Document,
        args: &[&str],
        postselected: bool,
    ) -> Result<String, CommandError> {
        let options = parse(
            args,
            self.0.get().for_document(document.tolerance())?,
            document.units(),
        )?;
        options.effective_tolerance(document.tolerance())?;
        self.0.set(options);
        prepare(document, options, postselected)?.apply(document)
    }
}
