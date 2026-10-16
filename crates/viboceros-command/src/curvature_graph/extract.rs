//! Extraction shares the accepted graph settings and immutable curve samples.
use super::*;
#[cfg(test)]
mod tests;
pub(crate) struct ExtractCommand(pub Arc<Session>);
pub const USAGE: &str = "ExtractCurvatureGraph";
impl Session {
    pub(crate) fn curve_sources(&self, doc: &Document) -> BTreeSet<ObjectId> {
        let state = self.0.lock().expect("curvature graph state");
        if !state.enabled {
            return BTreeSet::new();
        }
        state
            .sources
            .iter()
            .filter(|id| {
                doc.object(**id)
                    .is_some_and(|o| o.geometry().curve_ref().is_some())
            })
            .copied()
            .collect()
    }
    fn selected_curve_view(&self, doc: &Document) -> Result<Option<View>, CommandError> {
        let mut state = self.0.lock().expect("curvature graph state").clone();
        if !state.enabled {
            return Ok(None);
        }
        state.sources.retain(|id| {
            doc.is_selected(*id)
                && doc
                    .object(*id)
                    .is_some_and(|o| o.geometry().curve_ref().is_some())
        });
        if state.sources.is_empty() {
            return Ok(None);
        }
        Ok(Some(Self::resolve(&mut state, doc)?))
    }
}
impl ExtractCommand {
    fn extract(
        &self,
        doc: &mut Document,
        args: &[&str],
        postselected: bool,
    ) -> Result<String, CommandError> {
        require_consumed(args, 0, USAGE)?;
        let view = self
            .0
            .selected_curve_view(doc)?
            .ok_or(CommandError::OperationDeclined)?;
        let mut output = Vec::new();
        for graph in view.graphs.iter().filter(|g| g.kind == Kind::Curve) {
            output.extend(
                graph
                    .comb
                    .envelope_polylines(view.factor, doc.tolerance())?
                    .into_iter()
                    .map(Geometry::Polyline),
            );
        }
        let count = output.len();
        for geometry in output {
            doc.add_geometry(geometry)?;
        }
        if postselected {
            doc.clear_selection();
        }
        Ok(format!("Extracted {count} curvature graph outline(s)"))
    }
}
impl Command for ExtractCommand {
    fn name(&self) -> &'static str {
        USAGE
    }
    fn object_selection_prompt(
        &self,
        args: &[&str],
    ) -> Result<Option<ObjectSelectionPrompt>, CommandError> {
        require_consumed(args, 0, USAGE)?;
        Ok(Some(ObjectSelectionPrompt {
            command: self.name(),
            filter: ObjectSelectionFilter::Curves,
            workflow: ObjectSelectionWorkflow::OptionsDuringSelection,
            options: vec![],
            menus: vec![],
            choices: vec![],
        }))
    }
    fn cancel_empty_object_selection(&self) -> bool {
        true
    }
    fn cleanup_failed_selection(
        &self,
        doc: &mut Document,
        error: &CommandError,
        _postselected: bool,
    ) {
        if matches!(error, CommandError::OperationDeclined) {
            doc.clear_selection();
        }
    }
    fn run(&self, doc: &mut Document, args: &[&str]) -> Result<String, CommandError> {
        self.extract(doc, args, false)
    }
    fn run_postselected(
        &self,
        doc: &mut Document,
        args: &[&str],
        _context: CommandContext,
    ) -> Result<String, CommandError> {
        self.extract(doc, args, true)
    }
}
