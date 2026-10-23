//! Triangle pairing used only within one newly inserted polygon region.
use super::*;

#[derive(Clone, Copy)]
struct TriangleEdgeUse {
    face: usize,
    start: u32,
    end: u32,
    opposite: u32,
}

#[derive(Clone, Copy)]
struct Candidate {
    first: usize,
    second: usize,
    quad: [u32; 4],
    diagonal_ratio: Real,
    normal_angle: Real,
    removed_quality: Real,
}

/// Pair a newly triangulated region; source n-gons are rebuilt after tiling.
/// New quads must remain valid when a source boundary changes their origin.
pub(super) fn merge(
    mesh: TriangleMesh,
    tolerance: Tolerance,
) -> Result<(TriangleMesh, usize), GeometryError> {
    debug_assert!(mesh.ngons.is_empty());
    if mesh.faces.iter().filter(|face| face.is_triangle()).count() < 2 {
        return Ok((mesh, 0));
    }

    let mut edges = BTreeMap::<[u32; 2], Vec<TriangleEdgeUse>>::new();
    for (face, &polygon) in mesh.faces.iter().enumerate() {
        let MeshFace::Triangle([a, b, c]) = polygon else {
            continue;
        };
        for (start, end, opposite) in [(a, b, c), (b, c, a), (c, a, b)] {
            let key = [start.min(end), start.max(end)];
            edges.entry(key).or_default().push(TriangleEdgeUse {
                face,
                start,
                end,
                opposite,
            });
        }
    }
    let normals = mesh.polygon_face_normals()?;
    let triangle_quality = (0..mesh.face_count())
        .map(|i| mesh.face_aspect_ratio(i).map(|ratio| 1. / ratio))
        .collect::<Result<Vec<_>, _>>()?;
    let mut candidates = Vec::new();
    for uses in edges.values() {
        let [first, second] = uses.as_slice() else {
            continue;
        };
        if first.start != second.end || first.end != second.start {
            continue;
        }
        let corners = [first.start, first.end, first.opposite, second.opposite];
        if corners
            .iter()
            .enumerate()
            .any(|(i, v)| corners[..i].contains(v))
        {
            continue;
        }
        let [u, v, w, x] = corners;
        let normal_angle = normals[first.face]
            .as_vector()
            .angle_to(normals[second.face].as_vector())?;
        let (Ok(shared), Ok(other)) = (
            mesh.vertices[u as usize].distance_to(mesh.vertices[v as usize]),
            mesh.vertices[w as usize].distance_to(mesh.vertices[x as usize]),
        ) else {
            continue;
        };
        let short = shared.min(other);
        let long = shared.max(other);
        let diagonal_ratio = long / short;
        if !diagonal_ratio.is_finite() {
            continue;
        }
        let quad = [u, x, v, w];
        {
            let Some(projected) =
                project_mesh_hole_boundary(&quad.map(|i| mesh.vertices[i as usize]))?
            else {
                continue;
            };
            if (0..4).any(|i| {
                mesh_hole_cross(projected[i], projected[(i + 1) % 4], projected[(i + 2) % 4])
                    <= 64. * Real::EPSILON
            }) {
                continue;
            }
        }
        // A valid diagonal split does not imply a convex boundary. Both
        // triangles across the other diagonal must retain orientation.
        let (Ok(left), Ok(right)) = (
            mesh.normal_for_face(MeshFace::Triangle([x, v, w])),
            mesh.normal_for_face(MeshFace::Triangle([x, w, u])),
        ) else {
            continue;
        };
        if left.as_vector().dot(normals[first.face].as_vector())? <= 0.0
            || right.as_vector().dot(normals[first.face].as_vector())? <= 0.0
        {
            continue;
        }
        candidates.push(Candidate {
            first: first.face.min(second.face),
            second: first.face.max(second.face),
            quad,
            diagonal_ratio,
            normal_angle,
            removed_quality: triangle_quality[first.face] + triangle_quality[second.face],
        });
    }
    let planarity = |c: &Candidate| {
        if c.normal_angle > 64. * Real::EPSILON {
            c.normal_angle
        } else {
            0.
        }
    };
    candidates.sort_by(|a, b| {
        planarity(a)
            .total_cmp(&planarity(b))
            .then_with(|| a.removed_quality.total_cmp(&b.removed_quality))
            .then_with(|| a.diagonal_ratio.total_cmp(&b.diagonal_ratio))
            .then_with(|| a.normal_angle.total_cmp(&b.normal_angle))
            .then_with(|| (a.first, a.second).cmp(&(b.first, b.second)))
    });
    let mut replacement = vec![None; mesh.faces.len()];
    let mut removed = vec![false; mesh.faces.len()];
    let mut merged = 0;
    for candidate in candidates {
        if replacement[candidate.first].is_some()
            || replacement[candidate.second].is_some()
            || removed[candidate.first]
            || removed[candidate.second]
        {
            continue;
        }
        replacement[candidate.first] = Some(MeshFace::Quad(candidate.quad));
        removed[candidate.second] = true;
        merged += 1;
    }
    if merged == 0 {
        return Ok((mesh, 0));
    }
    let mut faces = Vec::with_capacity(mesh.faces.len() - merged);
    for (index, &face) in mesh.faces.iter().enumerate() {
        if removed[index] {
            continue;
        }
        faces.push(replacement[index].unwrap_or(face));
    }
    let mut result = TriangleMesh::try_new_faces(mesh.vertices, faces, tolerance)?
        .try_with_vertex_colors(mesh.vertex_colors)?;
    result.vertex_channels = mesh.vertex_channels;
    Ok((result, merged))
}

#[cfg(test)]
mod tests {
    use super::*;
    fn point(x: Real, y: Real, z: Real) -> Point3 {
        Point3::try_new(x, y, z).unwrap()
    }

    #[test]
    fn insertion_pairing_keeps_collinear_corners_as_separate_triangles() {
        let mesh = TriangleMesh::try_new(
            vec![
                point(0., 0.818517629189515, 0.10913568389193534),
                point(0., 0., 0.),
                point(4., 0., 0.),
                point(0., 2.2945783401646214, 0.30594377868861616),
            ],
            vec![[0, 1, 2], [0, 2, 3]],
            Tolerance::MESH_VALIDATION,
        )
        .unwrap();
        let (result, merged) = merge(mesh.clone(), Tolerance::MESH_VALIDATION).unwrap();
        assert_eq!(merged, 0);
        assert_eq!(result, mesh);
    }
}
