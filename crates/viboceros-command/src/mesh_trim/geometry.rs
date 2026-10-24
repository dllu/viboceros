use super::*;
use std::borrow::Cow;

pub(super) fn partition(
    doc: &Document,
    target: ObjectId,
    cutters: &[ObjectId],
    planes: &[String],
    point: Point3,
    context: CommandContext,
) -> Result<(Vec<viboceros_smlib::MeshSplitPart>, usize), CommandError> {
    let object = doc
        .object(target)
        .ok_or(DocumentError::ObjectNotFound(target))?;
    let Geometry::Mesh(mesh) = object.geometry() else {
        return Err(CommandError::Usage("a mesh to trim"));
    };
    let cutters = crate::mesh_partition::cutters(
        doc,
        target,
        mesh.bounds(),
        cutters,
        planes,
        context,
        doc.tolerance(),
    )?;
    let parts = viboceros_smlib::mesh_split_with_face_sources(
        mesh,
        &cutters.iter().map(Cow::as_ref).collect::<Vec<_>>(),
        true,
        doc.tolerance(),
    )?;
    let mut closest = None;
    for (i, part) in parts.iter().enumerate() {
        for face in 0..part.mesh.faces().len() {
            let candidate = part.mesh.closest_point_on_face(face, point)?;
            if closest
                .is_none_or(|(_, previous)| point.compare_distances(candidate, previous).is_lt())
            {
                closest = Some((i, candidate));
            }
        }
    }
    let selected = closest
        .ok_or(CommandError::Usage("a mesh region to trim"))?
        .0;
    Ok((parts, selected))
}
