//! Ribbon command staging keeps source state and geometry failures atomic.
use super::*;
mod geometry;
mod options;
pub use geometry::Source;
pub use options::{Input, Options, USAGE, length, parse};
#[cfg(test)]
mod tests;
use viboceros_geometry::CurveRef;

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
        if input.side.is_none() {
            return Err(CommandError::Usage(USAGE));
        }
        let mut sources = doc
            .selected_objects()
            .filter(|o| o.geometry().curve_ref().is_some());
        let source = sources
            .next()
            .ok_or(CommandError::RibbonSourceUnavailable)?;
        if sources.next().is_some() {
            return Err(CommandError::RibbonSourceUnavailable);
        }
        let (outputs, remembered) = build(
            source.geometry().curve_ref().unwrap(),
            input,
            context,
            doc.tolerance(),
        )?;
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
        let message = add_outputs(doc, outputs, attributes)?;
        if postselected {
            doc.clear_selection();
        }
        self.0.set(remembered);
        Ok(message)
    }
}

impl CommandRegistry {
    /// Accept a temporary Ribbon source from the shared SubCrv picker.
    pub fn commit_ribbon_portion(
        &self,
        document: &mut Document,
        reference: &curve_reference::CapturedCurve,
        portion: CurveRef<'_>,
        input: Input,
        context: CommandContext,
    ) -> Result<String, CommandError> {
        if !reference.current(document) {
            return Err(CommandError::RibbonSourceUnavailable);
        }
        let (outputs, remembered) = build(portion, input, context, document.tolerance())?;
        // A temporary curve has no input layer or object attributes.
        let attributes = ObjectAttributes::on_layer(document.current_layer_id());
        let message = run_command_transaction(document, "Ribbon", |document| {
            add_outputs(document, outputs, attributes)
        })?;
        self.ribbon_options.set(remembered);
        Ok(message)
    }
}

fn build(
    curve: CurveRef<'_>,
    input: Input,
    context: CommandContext,
    tolerance: Tolerance,
) -> Result<(Vec<Brep>, Options), CommandError> {
    let side = input.side.ok_or(CommandError::Usage(USAGE))?;
    let options = input.options.validate()?;
    let source = Source::capture(curve, options.in_cplane, context, tolerance)?;
    let distance = source.signed_distance(options, side, input.through_point)?;
    let outputs = source.solve(options, distance)?;
    let mut remembered = options.remembered();
    remembered.distance = distance.abs();
    Ok((outputs, remembered))
}

fn add_outputs(
    document: &mut Document,
    outputs: Vec<Brep>,
    attributes: ObjectAttributes,
) -> Result<String, CommandError> {
    let count = outputs.len();
    for brep in outputs {
        document.add_geometry_with_attributes(Geometry::Brep(brep), attributes.clone())?;
    }
    Ok(format!("Created {count} ribbon surface(s)"))
}
