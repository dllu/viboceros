//! In-place redundant boundary cleanup with one document transaction.
use super::*;

#[cfg(test)]
mod tests;

pub(super) struct MergeAllEdgesCommand;

impl Command for MergeAllEdgesCommand {
    fn name(&self) -> &'static str {
        "MergeAllEdges"
    }

    fn object_selection_prompt(
        &self,
        arguments: &[&str],
    ) -> Result<Option<ObjectSelectionPrompt>, CommandError> {
        require_consumed(arguments, 0, "MergeAllEdges")?;
        Ok(Some(ObjectSelectionPrompt {
            command: self.name(),
            filter: ObjectSelectionFilter::SurfaceComponents,
            options: vec![],
            menus: vec![],
            choices: vec![],
            workflow: ObjectSelectionWorkflow::OptionsDuringSelection,
        }))
    }

    fn run(&self, document: &mut Document, arguments: &[&str]) -> Result<String, CommandError> {
        merge(document, arguments, false)
    }

    fn run_postselected(
        &self,
        document: &mut Document,
        arguments: &[&str],
        _context: CommandContext,
    ) -> Result<String, CommandError> {
        merge(document, arguments, true)
    }

    fn cleanup_failed_selection(
        &self,
        document: &mut Document,
        error: &CommandError,
        _postselected: bool,
    ) {
        if matches!(error, CommandError::UnsupportedMergeAllEdgesGeometry) {
            document.clear_selection();
        }
    }
}

fn merge(
    document: &mut Document,
    arguments: &[&str],
    postselected: bool,
) -> Result<String, CommandError> {
    require_consumed(arguments, 0, "MergeAllEdges")?;
    if document.selected_object_count() == 0 {
        return Err(CommandError::NoObjectsSelected);
    }
    let tolerance = document.tolerance();
    let mut eligible = 0;
    let mut removed = 0;
    let mut replacements = Vec::new();
    for object in document.selected_objects() {
        let converted;
        let brep = match object.geometry() {
            Geometry::Brep(brep) => brep,
            Geometry::NurbsSurface(surface) => {
                converted = Brep::try_surface_face(surface.clone(), tolerance)?;
                &converted
            }
            _ => continue,
        };
        eligible += 1;
        // Public command probes on planar trims isolate this clamp from
        // Rhino's separate replacement-time splitting of kinky surfaces.
        let angle = tolerance
            .angular()
            .clamp(0.1_f64.to_radians(), 1_f64.to_radians());
        let merged = brep.try_merge_all_edges(angle, tolerance)?;
        let count = brep.edges().len() - merged.edges().len();
        if count > 0 {
            removed += count;
            replacements.push((object.id(), Geometry::Brep(merged)));
        }
    }
    if eligible == 0 {
        return Err(CommandError::UnsupportedMergeAllEdgesGeometry);
    }
    // Command-first picks are transient: neither Undo nor Redo restores
    // them. Release before recording replacements; registry rollback still
    // restores the original selection if the commit fails.
    if postselected {
        document.clear_selection();
    }
    let objects = document.replace_object_geometries(replacements)?;
    Ok(format!(
        "Merged {removed} redundant edge(s) in {objects} object(s)"
    ))
}
