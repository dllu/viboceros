//! Native positive-thickness walls, localized and staged before admission.
use super::*;
#[cfg(not(feature = "native-smlib"))]
pub(super) fn stage_positive_shell(
    _doc: &Document,
    _sources: &BTreeMap<ObjectId, Vec<usize>>,
    _thickness: f64,
) -> Result<Vec<(ObjectId, Geometry)>, CommandError> {
    Err(CommandError::ShellRequiresNativeKernel)
}
#[cfg(feature = "native-smlib")]
pub(super) fn stage_positive_shell(
    doc: &Document,
    sources: &BTreeMap<ObjectId, Vec<usize>>,
    thickness: f64,
) -> Result<Vec<(ObjectId, Geometry)>, CommandError> {
    let mut staged = Vec::new();
    let tolerance = doc.tolerance();
    for (id, indices) in sources {
        let Geometry::Brep(b) = doc.object(*id).unwrap().geometry() else {
            unreachable!()
        };
        let origin = b
            .vertices()
            .first()
            .map(|v| v.point())
            .unwrap_or_else(|| b.bounds().min());
        let zero = Point3::try_new(0., 0., 0.)?;
        let local = b.transformed(
            AffineTransform3::from_translation(origin.vector_to(zero)?),
            tolerance,
        )?;
        let result = viboceros_smlib::Shape::from_brep(&local, tolerance)?
            .shelled_faces(indices, thickness)?
            .to_brep_approximated_with_closed_uv_joints(tolerance, tolerance.absolute().min(1e-8))?
            .transformed(
                AffineTransform3::from_translation(zero.vector_to(origin)?),
                tolerance,
            )?;
        if !result.is_solid() {
            return Err(CommandError::Usage(
                "Shell did not produce a closed wall volume",
            ));
        }
        let components = result.edge_connected_face_components();
        if components.len() == 1 {
            staged.push((*id, Geometry::Brep(result)));
        } else {
            // Keep inward cavity boundaries with their enclosing material.
            let (_, regions) = viboceros_smlib::Solid::from_brep_with_regions(&result, tolerance)?;
            for region in regions {
                let faces = region
                    .iter()
                    .flat_map(|&i| components[i].iter().copied())
                    .collect::<Vec<_>>();
                staged.push((*id, Geometry::Brep(result.sub_brep(&faces, tolerance)?)));
            }
        }
    }
    Ok(staged)
}
