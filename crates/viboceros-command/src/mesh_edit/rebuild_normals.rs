//! Rebuild stored mesh normals with native compaction and replacement history.
use crate::*;
pub(crate) struct RebuildMeshNormalsCommand;
#[cfg(test)]
mod tests;

impl Command for RebuildMeshNormalsCommand {
    fn name(&self) -> &'static str {
        "RebuildMeshNormals"
    }
    fn object_selection_prompt(
        &self,
        args: &[&str],
    ) -> Result<Option<ObjectSelectionPrompt>, CommandError> {
        require_consumed(args, 0, self.name())?;
        Ok(Some(ObjectSelectionPrompt {
            command: self.name(),
            filter: ObjectSelectionFilter::Mesh,
            workflow: ObjectSelectionWorkflow::ConfirmAfterSelection,
            options: vec![],
            menus: vec![],
            choices: vec![],
        }))
    }
    fn run(&self, doc: &mut Document, args: &[&str]) -> Result<String, CommandError> {
        self.run_selected(doc, args, false)
    }
    fn run_postselected(
        &self,
        doc: &mut Document,
        args: &[&str],
        _: CommandContext,
    ) -> Result<String, CommandError> {
        self.run_selected(doc, args, true)
    }
}
impl RebuildMeshNormalsCommand {
    fn run_selected(
        &self,
        doc: &mut Document,
        args: &[&str],
        postselected: bool,
    ) -> Result<String, CommandError> {
        require_consumed(args, 0, self.name())?;
        if doc.selected_object_count() == 0 {
            return Err(CommandError::NoObjectsSelected);
        }
        let replacements = doc
            .selected_objects()
            .filter_map(|object| {
                let Geometry::Mesh(mesh) = object.geometry() else {
                    return None;
                };
                Some(mesh.rebuilt_normals().map(|mesh| (object.id(), mesh)))
            })
            .collect::<Result<Vec<_>, _>>()?;
        if replacements.is_empty() {
            return Err(CommandError::UnsupportedRebuildMeshNormalsGeometry);
        }
        let count = replacements.len();
        super::replacement::replace_meshes(doc, replacements, postselected)?;
        Ok(format!("Rebuilt normals on {count} mesh(es)"))
    }
}
