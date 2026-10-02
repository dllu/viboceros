//! Underlying-surface shrink, staged across all selected objects before editing.
use super::*;
use viboceros_geometry::BrepSurfaceShrinkMode;

#[cfg(test)]
mod tests;

pub(super) struct ShrinkTrimmedCommand(pub(super) BrepSurfaceShrinkMode);

impl Command for ShrinkTrimmedCommand {
    fn name(&self) -> &'static str {
        match self.0 {
            BrepSurfaceShrinkMode::Standard => "ShrinkTrimmedSrf",
            BrepSurfaceShrinkMode::ToEdge => "ShrinkTrimmedSrfToEdge",
        }
    }

    fn object_selection_prompt(
        &self,
        arguments: &[&str],
    ) -> Result<Option<ObjectSelectionPrompt>, CommandError> {
        require_consumed(arguments, 0, self.name())?;
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
        self.shrink(document, arguments, false)
    }

    fn run_postselected(
        &self,
        document: &mut Document,
        arguments: &[&str],
        _context: CommandContext,
    ) -> Result<String, CommandError> {
        self.shrink(document, arguments, true)
    }
}

impl ShrinkTrimmedCommand {
    fn shrink(
        &self,
        document: &mut Document,
        arguments: &[&str],
        postselected: bool,
    ) -> Result<String, CommandError> {
        require_consumed(arguments, 0, self.name())?;
        if document.selected_object_count() == 0 {
            return Err(CommandError::NoObjectsSelected);
        }
        let tolerance = document.tolerance();
        let sources = document.selected_objects().collect::<Vec<_>>();
        let mut staged = Vec::new();
        let mut retained = Vec::new();
        let mut shrunk = 0usize;
        let mut unchanged = 0usize;
        let mut eligible = Vec::new();
        for object in sources {
            let converted;
            let brep = match object.geometry() {
                Geometry::Brep(brep) => brep,
                Geometry::NurbsSurface(surface) => {
                    converted = Brep::try_surface_face_with_native_edge_parameters(
                        surface.clone(),
                        tolerance,
                    )?;
                    &converted
                }
                _ => {
                    retained.push(object.id());
                    continue;
                }
            };
            eligible.push(object.id());
            let result = brep.try_shrunk_surfaces(self.0, tolerance)?;
            let count = result
                .faces()
                .iter()
                .zip(brep.faces())
                .filter(|(a, b)| a.surface() != b.surface())
                .count();
            shrunk += count;
            unchanged += brep.faces().len() - count;
            if count == 0 {
                retained.push(object.id());
            } else {
                staged.push((object.id(), Geometry::Brep(result)));
            }
        }
        if eligible.is_empty() {
            return Err(CommandError::NoObjectsSelected);
        }
        let order = staged.iter().map(|(id, _)| *id).collect::<Vec<_>>();
        if postselected && !staged.is_empty() {
            document.release_command_selection_on_history_replay(eligible)?;
            document.clear_selection();
        }
        document.replace_object_geometries(staged)?;
        document.move_objects_to_end_in_order(order)?;
        if postselected {
            document.select_command_results(retained)?;
        }
        Ok(format!(
            "Shrunk {shrunk} surface(s); {unchanged} already shrunk"
        ))
    }
}
