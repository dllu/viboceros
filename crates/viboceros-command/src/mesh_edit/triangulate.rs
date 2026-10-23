//! Whole-mesh triangulation with staged replacements.
use crate::*;

pub(crate) struct TriangulateMeshCommand;

impl Command for TriangulateMeshCommand {
    fn name(&self) -> &'static str {
        "TriangulateMesh"
    }

    fn object_selection_prompt(
        &self,
        arguments: &[&str],
    ) -> Result<Option<ObjectSelectionPrompt>, CommandError> {
        require_consumed(arguments, 0, "TriangulateMesh")?;
        Ok(Some(ObjectSelectionPrompt {
            command: "TriangulateMesh",
            filter: ObjectSelectionFilter::Mesh,
            workflow: ObjectSelectionWorkflow::ConfirmAfterSelection,
            options: vec![],
            menus: vec![],
            choices: vec![],
        }))
    }

    fn run(&self, document: &mut Document, arguments: &[&str]) -> Result<String, CommandError> {
        self.run_selected(document, arguments, false)
    }

    fn run_postselected(
        &self,
        document: &mut Document,
        arguments: &[&str],
        _: CommandContext,
    ) -> Result<String, CommandError> {
        self.run_selected(document, arguments, true)
    }
}

impl TriangulateMeshCommand {
    fn run_selected(
        &self,
        document: &mut Document,
        arguments: &[&str],
        postselected: bool,
    ) -> Result<String, CommandError> {
        require_consumed(arguments, 0, "TriangulateMesh")?;
        if document.selected_object_count() == 0 {
            return Err(CommandError::NoObjectsSelected);
        }
        let staged = document
            .selected_objects()
            .filter(|object| matches!(object.geometry(), Geometry::Mesh(_)))
            .map(|object| {
                let Geometry::Mesh(mesh) = object.geometry() else {
                    return Err(CommandError::UnsupportedTriangulateMeshGeometry);
                };
                let (triangulated, quad_count) = mesh.triangulate_quads(document.tolerance())?;
                let changed = quad_count > 0 || !mesh.ngons().is_empty();
                // The command removes all overlays, including triangle-only
                // regions; the SDK operation retains and remaps them.
                let triangulated = triangulated.try_with_ngons(Vec::new())?;
                Ok((object.id(), triangulated, quad_count, changed))
            })
            .collect::<Result<Vec<_>, CommandError>>()?;
        if staged.is_empty() {
            return Err(CommandError::UnsupportedTriangulateMeshGeometry);
        }
        let changed_mesh_count = staged.iter().filter(|(_, _, _, changed)| *changed).count();
        let converted_quad_count = staged
            .iter()
            .map(|(_, _, quad_count, _)| quad_count)
            .sum::<usize>();
        let unchanged = staged.len() - changed_mesh_count;
        super::replacement::replace_meshes(
            document,
            staged.into_iter().map(|(id, mesh, _, _)| (id, mesh)),
            postselected,
        )?;
        Ok(format!(
            "Triangulated {converted_quad_count} quadrilateral face(s) in {changed_mesh_count} mesh(es); {unchanged} mesh(es) unchanged"
        ))
    }
}
