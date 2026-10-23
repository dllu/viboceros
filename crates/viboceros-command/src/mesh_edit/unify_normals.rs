//! Whole-mesh winding repair.
use crate::*;

pub(crate) struct UnifyMeshNormalsCommand;

impl Command for UnifyMeshNormalsCommand {
    fn name(&self) -> &'static str {
        "UnifyMeshNormals"
    }

    fn run(&self, document: &mut Document, arguments: &[&str]) -> Result<String, CommandError> {
        self.run_selected(document, arguments, false)
    }

    fn run_postselected(
        &self,
        doc: &mut Document,
        args: &[&str],
        _: CommandContext,
    ) -> Result<String, CommandError> {
        self.run_selected(doc, args, true)
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
}

impl UnifyMeshNormalsCommand {
    fn run_selected(
        &self,
        document: &mut Document,
        arguments: &[&str],
        postselected: bool,
    ) -> Result<String, CommandError> {
        require_consumed(arguments, 0, "UnifyMeshNormals")?;
        let selected = document
            .selected_objects()
            .map(|object| (object.id(), object.geometry()))
            .collect::<Vec<_>>();
        if selected.is_empty() {
            return Err(CommandError::NoObjectsSelected);
        }

        let mut mesh_count = 0;
        let mut flipped_face_count = 0;
        let mut replacements = Vec::new();
        for (id, geometry) in selected {
            let Geometry::Mesh(mesh) = geometry else {
                continue;
            };
            mesh_count += 1;
            let (unified, flipped) = mesh.unified_normals_for_command()?;
            flipped_face_count += flipped;
            if flipped > 0 {
                replacements.push((id, unified));
            }
        }
        if mesh_count == 0 {
            return Err(CommandError::UnsupportedUnifyMeshNormalsGeometry);
        }
        let changed_mesh_count = replacements.len();
        super::replacement::replace_changed_meshes(document, replacements, postselected)?;
        Ok(format!(
            "Unified {mesh_count} mesh(es): flipped {flipped_face_count} face(s) in {changed_mesh_count} mesh(es); {} already consistent",
            mesh_count - changed_mesh_count
        ))
    }
}
