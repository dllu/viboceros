//! View-dependent outlines of aggregate mesh and surface display coverage.
use super::*;
use std::borrow::Cow;
use viboceros_geometry::MeshOutlineProjection;

pub(super) struct MeshOutlineCommand;

impl Command for MeshOutlineCommand {
    fn name(&self) -> &'static str {
        "MeshOutline"
    }

    fn object_selection_prompt(
        &self,
        args: &[&str],
    ) -> Result<Option<ObjectSelectionPrompt>, CommandError> {
        require_consumed(args, 0, self.name())?;
        Ok(Some(ObjectSelectionPrompt {
            command: self.name(),
            filter: ObjectSelectionFilter::EdgeAnalysis,
            workflow: ObjectSelectionWorkflow::ConfirmAfterSelection,
            options: vec![],
            menus: vec![],
            choices: vec![],
        }))
    }

    fn cancel_empty_object_selection(&self) -> bool {
        true
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
        execute(
            doc,
            args,
            MeshOutlineProjection::Parallel(context.construction_plane),
        )
    }

    fn run_with_pick_projection(
        &self,
        doc: &mut Document,
        args: &[&str],
        context: CommandContext,
        _: bool,
        projection: picking::Projection,
    ) -> Result<String, CommandError> {
        execute(
            doc,
            args,
            projection.mesh_outline_projection(context.construction_plane)?,
        )
    }
}

fn execute(
    doc: &mut Document,
    args: &[&str],
    projection: MeshOutlineProjection,
) -> Result<String, CommandError> {
    require_consumed(args, 0, "MeshOutline")?;
    let selected = doc.selected_objects().map(|o| o.id()).collect::<Vec<_>>();
    let mut eligible = Vec::new();
    let mut meshes = Vec::new();
    for &id in &selected {
        let object = doc.object(id).expect("selected object exists");
        let mesh = match object.geometry() {
            Geometry::Mesh(mesh) => Cow::Borrowed(mesh),
            Geometry::NurbsSurface(surface) => Cow::Owned(
                surface.tessellate(interface::SURFACE_DISPLAY_SAMPLES_PER_SPAN, doc.tolerance())?,
            ),
            Geometry::Brep(brep) => Cow::Owned(
                brep.display_mesh(interface::SURFACE_DISPLAY_SAMPLES_PER_SPAN, doc.tolerance())?,
            ),
            _ => continue,
        };
        eligible.push(id);
        meshes.push(mesh);
    }
    if meshes.is_empty() {
        return Err(CommandError::NoObjectsSelected);
    }
    let aggregate = (meshes.len() > 1)
        .then(|| TriangleMesh::try_append(&meshes.iter().map(Cow::as_ref).collect::<Vec<_>>()))
        .transpose()?;
    let mesh = aggregate.as_ref().unwrap_or(meshes[0].as_ref());
    let outlines = mesh.outlines(projection, Tolerance::MESH_VALIDATION)?;
    // Finish all fallible geometry work before changing selection or objects.
    drop(aggregate);
    drop(meshes);
    doc.release_command_selection_on_history_replay(selected)?;
    doc.select_objects_direct(eligible, SelectionMode::Remove)?;
    let count = outlines.len();
    let ids = outlines
        .into_iter()
        .map(|p| doc.add_geometry(Geometry::Polyline(p)))
        .collect::<Result<Vec<_>, _>>()?;
    doc.release_command_selection_on_history_replay(ids.iter().copied())?;
    // Native keeps ignored preselected peers selected during the live command.
    doc.select_objects_direct(ids, SelectionMode::Add)?;
    Ok(format!("Created {count} mesh outline(s)"))
}

#[cfg(test)]
mod tests;
