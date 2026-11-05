//! One exact limit face-center sample per source face, with bounded annotation.
use super::*;
use viboceros_geometry::SubD;

pub(super) fn collect(id: ObjectId, subd: &SubD) -> Vec<Marker> {
    let Ok(display) = subd.tessellate_with_normals(1) else {
        return Vec::new();
    };
    // At density one, all child quads of a control face share its face point.
    // Intersect their four indices to locate that exact limit sample. This is
    // linear in the display size, including arbitrary-sided control faces.
    let mut centers = vec![None::<Vec<(u32, Point3, Vector3)>>; subd.faces().len()];
    for ((face, source), normals) in display
        .mesh
        .faces()
        .iter()
        .zip(display.face_sources)
        .zip(display.face_corner_normals)
    {
        let indices = face.indices();
        let Some(candidates) = centers.get_mut(source) else {
            continue;
        };
        if let Some(candidates) = candidates {
            candidates.retain(|(i, _, _)| indices.contains(i));
        } else {
            *candidates = Some(
                indices
                    .iter()
                    .zip(normals)
                    .map(|(&i, n)| (i, display.mesh.vertices()[i as usize], n))
                    .collect(),
            );
        }
    }
    centers
        .into_iter()
        .step_by(subd.faces().len().div_ceil(256).max(1))
        .filter_map(|candidates| {
            let candidates = candidates?;
            let [(_, point, normal)] = candidates.as_slice() else {
                return None;
            };
            Some(Marker {
                object: id,
                point: *point,
                axes: [
                    None,
                    None,
                    Some(normal.normalized_nonzero().ok()?.as_vector()),
                ],
                surface: false,
            })
        })
        .collect()
}
