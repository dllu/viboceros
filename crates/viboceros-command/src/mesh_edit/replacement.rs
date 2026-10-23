//! Replacement lifecycle observed for TriangulateMesh and RebuildMeshNormals.
use crate::*;

pub(super) fn replace_meshes(
    document: &mut Document,
    replacements: impl IntoIterator<Item = (ObjectId, TriangleMesh)>,
    postselected: bool,
) -> Result<(), CommandError> {
    let replacements = replacements.into_iter().collect::<Vec<_>>();
    let ids = replacements.iter().map(|(id, _)| *id).collect::<Vec<_>>();
    let released = document
        .selected_objects()
        .filter(|object| postselected || !matches!(object.geometry(), Geometry::Mesh(_)))
        .map(|object| object.id())
        .collect::<Vec<_>>();
    document.release_command_selection_on_history_replay(released)?;
    if postselected {
        document.clear_selection();
    }
    document.replace_object_geometries_with_history(
        replacements
            .into_iter()
            .map(|(id, mesh)| (id, Geometry::Mesh(mesh))),
        viboceros_document::ReplacementHistory::EveryReplacement,
    )?;
    document.move_objects_to_end_in_order(ids.iter().copied())?;
    if !postselected {
        document.release_transform_selection_on_history_replay(ids)?;
    }
    Ok(())
}
