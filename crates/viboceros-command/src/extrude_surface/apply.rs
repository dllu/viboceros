//! Stage complete surface extrusion products and remainders before editing.
use super::*;

/// Stage per-face products and source remainders before admitting one command batch.
pub(crate) fn apply_face_extrusion_products(
    doc: &mut Document,
    faces: &SurfaceFaces,
    delete_input: bool,
    mut produce: impl FnMut(&Brep, Tolerance) -> Result<Vec<Geometry>, CommandError>,
) -> Result<usize, CommandError> {
    let refs = surface_extrusion_targets(doc, faces)?;
    let count = refs.len();
    let tolerance = doc.tolerance();
    let mut staged = Vec::new();
    for &(id, face) in &refs {
        let brep = capture_source_face(doc, id, face)?;
        staged.extend(produce(&brep, tolerance)?);
    }
    let outputs = apply_staged_products(doc, staged, refs.into_iter().filter(|_| delete_input))?;
    doc.select_command_results(outputs)?;
    Ok(count)
}

pub(crate) fn capture_source_face(
    doc: &Document,
    id: ObjectId,
    face: usize,
) -> Result<Brep, CommandError> {
    let tolerance = doc.tolerance();
    Ok(match doc.object(id).unwrap().geometry() {
        Geometry::Brep(b) => b.duplicate_faces(&[face], tolerance)?,
        Geometry::NurbsSurface(s) => Brep::try_surface_face(s.clone(), tolerance)?,
        _ => unreachable!(),
    })
}

pub(crate) fn apply_staged_products(
    doc: &mut Document,
    staged: Vec<Geometry>,
    deleted_faces: impl Iterator<Item = (ObjectId, usize)>,
) -> Result<Vec<ObjectId>, CommandError> {
    let tolerance = doc.tolerance();
    let mut grouped = BTreeMap::<ObjectId, BTreeSet<usize>>::new();
    for (id, face) in deleted_faces {
        grouped.entry(id).or_default().insert(face);
    }
    let mut remainders = Vec::new();
    for (id, used) in grouped {
        let remainder = match doc.object(id).unwrap().geometry() {
            Geometry::Brep(b) => {
                let keep = (0..b.faces().len())
                    .filter(|i| !used.contains(i))
                    .collect::<Vec<_>>();
                if keep.is_empty() {
                    None
                } else {
                    Some(Geometry::Brep(b.sub_brep(&keep, tolerance)?))
                }
            }
            Geometry::NurbsSurface(_) => None,
            _ => unreachable!(),
        };
        remainders.push((id, remainder));
    }
    let mut outputs = Vec::with_capacity(staged.len());
    for geometry in staged {
        outputs.push(doc.add_geometry(geometry)?);
    }
    for (id, remainder) in remainders {
        if let Some(g) = remainder {
            doc.replace_object_geometries([(id, g)])?;
        } else {
            doc.delete_object(id)?;
        }
    }
    Ok(outputs)
}
