//! Ribbon command staging keeps source state and geometry failures atomic.
use super::*;
mod geometry;
mod options;
pub use geometry::Source;
pub use options::{Input, Options, USAGE, length, parse};
#[cfg(test)]
mod tests;

pub(super) struct RibbonCommand(pub std::sync::Arc<remembered::Remembered<Options>>);

impl Command for RibbonCommand {
    fn name(&self) -> &'static str {
        "Ribbon"
    }
    fn run(&self, doc: &mut Document, args: &[&str]) -> Result<String, CommandError> {
        self.run_in_context(doc, args, CommandContext::default())
    }
    fn run_in_context(
        &self,
        doc: &mut Document,
        args: &[&str],
        context: CommandContext,
    ) -> Result<String, CommandError> {
        self.execute(doc, args, context, false)
    }
    fn run_postselected(
        &self,
        doc: &mut Document,
        args: &[&str],
        context: CommandContext,
    ) -> Result<String, CommandError> {
        self.execute(doc, args, context, true)
    }
    fn object_selection_prompt(
        &self,
        args: &[&str],
    ) -> Result<Option<ObjectSelectionPrompt>, CommandError> {
        parse(
            args,
            self.0.get(),
            &viboceros_geometry::LengthUnitSystem::Millimeters,
            CommandContext::default(),
        )?;
        Ok(Some(ObjectSelectionPrompt {
            command: "Ribbon",
            filter: ObjectSelectionFilter::Curves,
            workflow: ObjectSelectionWorkflow::PointInputAfterSelection,
            menus: vec![],
            choices: vec![],
            options: vec![],
        }))
    }
    fn object_selection_complete(
        &self,
        doc: &Document,
        _args: &[&str],
    ) -> Result<bool, CommandError> {
        Ok(doc
            .selected_objects()
            .filter(|o| o.geometry().curve_ref().is_some())
            .count()
            == 1)
    }
}
impl RibbonCommand {
    fn execute(
        &self,
        doc: &mut Document,
        args: &[&str],
        context: CommandContext,
        postselected: bool,
    ) -> Result<String, CommandError> {
        let input = parse(args, self.0.get(), doc.units(), context)?;
        let side = input.side.ok_or(CommandError::Usage(USAGE))?;
        let mut sources = doc
            .selected_objects()
            .filter(|o| o.geometry().curve_ref().is_some());
        let source = sources
            .next()
            .ok_or(CommandError::RibbonSourceUnavailable)?;
        if sources.next().is_some() {
            return Err(CommandError::RibbonSourceUnavailable);
        }
        let solver = Source::capture(
            source.geometry().curve_ref().unwrap(),
            input.options.in_cplane,
            context,
            doc.tolerance(),
        )?;
        let distance = solver.signed_distance(input.options, side, input.through_point)?;
        let outputs = solver.solve(input.options, distance)?;
        let attributes = if input.options.input_layer {
            source.attributes().clone()
        } else {
            ObjectAttributes::on_layer(doc.current_layer_id())
        };
        drop(sources);
        if postselected {
            doc.release_command_selection_on_history_replay(
                doc.selected_object_ids().collect::<Vec<_>>(),
            )?;
        }
        let count = outputs.len();
        for brep in outputs {
            doc.add_geometry_with_attributes(Geometry::Brep(brep), attributes.clone())?;
        }
        if postselected {
            doc.clear_selection();
        }
        let mut remembered = input.options.remembered();
        remembered.distance = distance.abs();
        self.0.set(remembered);
        Ok(format!("Created {count} ribbon surface(s)"))
    }
}
