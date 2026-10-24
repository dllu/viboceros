//! MeshSplit partition policy over shared cutter preparation and source faces.
use super::*;
use std::borrow::Cow;
pub(super) fn compute(
    doc: &Document,
    first: &[ObjectId],
    args: &Arguments,
    context: CommandContext,
) -> Result<Vec<(ObjectId, TriangleMesh)>, CommandError> {
    let tolerance = args.shared.geometry_tolerance(doc.tolerance());
    let targets = first
        .iter()
        .map(|&id| {
            let object = doc.object(id).ok_or(DocumentError::ObjectNotFound(id))?;
            crate::mesh_partition::source_mesh(object.geometry(), doc.tolerance())
        })
        .collect::<Result<Vec<_>, _>>()?;
    let mut output = Vec::new();
    let mut work = 0;
    for (i, target) in targets.iter().enumerate() {
        let cutters = crate::mesh_partition::cutters(
            doc,
            first[i],
            target.bounds(),
            &args.second,
            &args.planes,
            context,
            tolerance,
        )?;
        let mut parts = viboceros_smlib::mesh_split_with_face_sources(
            target,
            &cutters.iter().map(Cow::as_ref).collect::<Vec<_>>(),
            args.shared.coplanar != "Drop",
            tolerance,
        )?;
        if args.settings.discard && parts.len() < 2 {
            continue;
        }
        parts.sort_by(|a, b| {
            b.mesh
                .bounds()
                .min()
                .to_array()
                .partial_cmp(&a.mesh.bounds().min().to_array())
                .unwrap()
                .then_with(|| {
                    a.mesh
                        .bounds()
                        .max()
                        .to_array()
                        .partial_cmp(&b.mesh.bounds().max().to_array())
                        .unwrap()
                })
        });
        let appearance = crate::mesh_partition::Appearance::new(target)?;
        for part in parts {
            let mesh = appearance.restore(part.mesh, part.source_faces, args.settings.ngons)?;
            work += mesh.vertices().len() + mesh.faces().len();
            if work > MAX_SPAN_OUTPUT_OBJECTS {
                return Err(too_many_span_outputs("mesh split"));
            }
            output.push((first[i], mesh));
        }
    }
    Ok(output)
}
