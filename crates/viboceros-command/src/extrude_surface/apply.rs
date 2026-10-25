//! Stage complete surface extrusion products and remainders before editing.
use super::*;

pub(crate) fn apply_face_extrusions(
    doc: &mut Document,
    faces: &SurfaceFaces,
    delete_input: bool,
    mut produce: impl FnMut(&Brep, Tolerance) -> Result<Geometry, CommandError>,
) -> Result<usize, CommandError> {
    apply_face_extrusion_products(doc, faces, delete_input, |brep, tolerance| {
        Ok(vec![produce(brep, tolerance)?])
    })
}

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
    let mut grouped = BTreeMap::<ObjectId, BTreeSet<usize>>::new();
    let mut staged = Vec::new();
    for (id, face) in refs {
        let source = doc.object(id).unwrap().geometry();
        let brep = match source {
            Geometry::Brep(b) => b.duplicate_faces(&[face], tolerance)?,
            Geometry::NurbsSurface(s) => Brep::try_surface_face(s.clone(), tolerance)?,
            _ => unreachable!(),
        };
        staged.extend(produce(&brep, tolerance)?);
        grouped.entry(id).or_default().insert(face);
    }
    let mut remainders = Vec::new();
    if delete_input {
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
    doc.select_command_results(outputs)?;
    Ok(count)
}
