//! Expands face picks to original non-tangent boundary edges.
use super::*;

pub fn edge_blend_face_edges(
    brep: &Brep,
    faces: impl IntoIterator<Item = usize>,
    tolerance: Tolerance,
) -> Result<Vec<usize>, CommandError> {
    if !brep.is_solid() {
        return Err(CommandError::Usage("Pick faces on a closed polysurface"));
    }
    let shared = brep.edges_shared_by_distinct_faces();
    let mut edges = BTreeSet::new();
    for index in faces {
        let face = brep
            .faces()
            .get(index)
            .ok_or(CommandError::Usage("Face selection is out of range"))?;
        edges.extend(
            face.loops()
                .iter()
                .flat_map(|boundary| boundary.trims())
                .filter_map(|trim| trim.edge())
                .filter(|&edge| shared[edge]),
        );
    }
    if edges.is_empty() {
        return Ok(Vec::new());
    }
    let edges = edges.into_iter().collect::<Vec<_>>();
    let (local, _) = local_blend_source(brep, tolerance)?;
    let solid = viboceros_smlib::Solid::from_brep(&local, tolerance)?;
    let tangent = solid.tangent_source_edges(&edges, tolerance.angular())?;
    Ok(edges
        .into_iter()
        .zip(tangent)
        .filter_map(|(edge, tangent)| (!tangent).then_some(edge))
        .collect())
}

#[cfg(test)]
mod tests {
    use super::*;
    use viboceros_geometry::TriangleMesh;

    #[test]
    fn face_edges_omit_coplanar_divisions_and_deduplicate_boundary_picks() {
        let tolerance = Tolerance::DEFAULT;
        let vertices = [
            [0., 0., 0.],
            [10., 0., 0.],
            [10., 10., 0.],
            [0., 10., 0.],
            [0., 0., 10.],
            [10., 0., 10.],
            [10., 10., 10.],
            [0., 10., 10.],
        ]
        .map(|point| Point3::try_from(point).unwrap())
        .to_vec();
        let triangles = [
            [0, 3, 2, 1],
            [4, 5, 6, 7],
            [0, 1, 5, 4],
            [1, 2, 6, 5],
            [2, 3, 7, 6],
            [3, 0, 4, 7],
        ]
        .into_iter()
        .flat_map(|[a, b, c, d]| [[a, b, c], [a, c, d]])
        .collect();
        let mesh = TriangleMesh::try_new(vertices, triangles, tolerance).unwrap();
        let brep = Brep::try_from_mesh(&mesh, true, tolerance).unwrap();
        let first = edge_blend_face_edges(&brep, [2], tolerance).unwrap();
        assert_eq!(first.len(), 2);
        let top = edge_blend_face_edges(&brep, [2, 3, 2], tolerance).unwrap();
        assert_eq!(top.len(), 4);
        assert!(first.iter().all(|edge| top.contains(edge)));
        for index in top {
            let [a, b] = brep.edges()[index]
                .vertices()
                .map(|i| brep.vertices()[i].point());
            assert_eq!(a.z(), 10.);
            assert_eq!(b.z(), 10.);
            assert!(a.x() == b.x() || a.y() == b.y());
        }
    }

    #[test]
    fn face_edges_omit_the_tangent_rails_of_a_rounded_box() {
        let tolerance = Tolerance::DEFAULT;
        let frame = CommandContext::default().construction_plane;
        let brep = Brep::try_box(frame, [[0., 10.]; 3], tolerance).unwrap();
        let rounded = blend(
            &brep,
            &[0],
            1.,
            EdgeBlendKind::Fillet(FilletCorner::Automatic),
            tolerance,
        )
        .unwrap();
        let face = rounded
            .faces()
            .iter()
            .position(|face| face.surface().plane(tolerance).unwrap().is_none())
            .expect("expected the curved fillet face");
        let all_edges = rounded.faces()[face].loops()[0].trims().len();
        assert_eq!(all_edges, 4);
        let sharp = edge_blend_face_edges(&rounded, [face], tolerance).unwrap();
        assert_eq!(sharp.len(), 2);
        let [a, b] = brep.edges()[0]
            .vertices()
            .map(|i| brep.vertices()[i].point().to_array());
        let axis = (0..3).find(|&axis| a[axis] != b[axis]).unwrap();
        for end in [a[axis], b[axis]] {
            assert!(sharp.iter().any(|&index| {
                rounded.edges()[index]
                    .curve()
                    .control_points()
                    .iter()
                    .all(|point| tolerance.approx_eq(point.point().to_array()[axis], end))
            }));
        }
    }
}
