//! Replacement and selection lifecycles for whole-mesh editing commands.
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

/// Record only winding repairs. Accepted unchanged peers retain their live
/// selection but release it on history replay, including all-no-op commands.
pub(super) fn replace_changed_meshes(
    document: &mut Document,
    replacements: Vec<(ObjectId, TriangleMesh)>,
    postselected: bool,
) -> Result<(), CommandError> {
    let ids = replacements
        .iter()
        .map(|(id, _)| *id)
        .collect::<BTreeSet<_>>();
    let released = document
        .selected_objects()
        .filter(|object| postselected || !ids.contains(&object.id()))
        .map(|object| object.id())
        .collect::<Vec<_>>();
    document.release_command_selection_on_history_replay(released)?;
    if postselected && !replacements.is_empty() {
        document.clear_selection();
    }
    let ordered = replacements.iter().map(|(id, _)| *id).collect::<Vec<_>>();
    document.replace_object_geometries(
        replacements
            .into_iter()
            .map(|(id, mesh)| (id, Geometry::Mesh(mesh))),
    )?;
    document.move_objects_to_end_in_order(ordered)?;
    if !postselected {
        document.release_transform_selection_on_history_replay(ids)?;
    }
    Ok(())
}
