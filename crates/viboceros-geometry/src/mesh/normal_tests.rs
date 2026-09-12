use super::*;

#[test]
fn warped_quad_normals_use_diagonals_and_preserve_cyclic_winding() {
    for scale in [1.0, 1e200, 1e-200] {
        let vertices = [[0., 0., 0.], [2., 0., 0.], [2., 1., 1.], [0., 1., 0.]]
            .map(|[x, y, z]| Point3::try_new(x * scale, y * scale, z * scale).unwrap())
            .to_vec();
        for reverse in [false, true] {
            for offset in 0..4 {
                let mut indices = [0, 1, 2, 3];
                indices.rotate_left(offset);
                if reverse {
                    indices.reverse();
                }
                let mesh = TriangleMesh::try_new_faces(
                    vertices.clone(),
                    vec![MeshFace::Quad(indices)],
                    Tolerance::MESH_VALIDATION,
                )
                .unwrap();
                let normal = mesh.polygon_face_normals().unwrap()[0].as_vector();
                // AC x BD = (2,1,1) x (-2,1,0) = (-1,-2,4).
                // Unequal triangle areas make averaging unit triangle normals wrong.
                let sign = if reverse { -1.0 } else { 1.0 };
                for (actual, component) in [normal.x(), normal.y(), normal.z()]
                    .into_iter()
                    .zip([-1., -2., 4.])
                {
                    let expected = sign * component / 21.0f64.sqrt();
                    assert!(
                        (actual - expected).abs() <= 4.0 * f64::EPSILON,
                        "scale={scale}, offset={offset}, reverse={reverse}: {actual} != {expected}"
                    );
                }
            }
        }
    }
}

#[test]
fn polygon_normals_preserve_direction_across_extreme_mesh_scales() {
    for scale in [1.0, 1e200, 1e-200] {
        for reverse in [false, true] {
            for quad in [false, true] {
                // z = x + y: its oriented unit normal is (-1, -1, 1)/sqrt(3).
                let vertices = [[0., 0., 0.], [1., 0., 1.], [1., 1., 2.], [0., 1., 1.]]
                    .map(|[x, y, z]| Point3::try_new(x * scale, y * scale, z * scale).unwrap())
                    .to_vec();
                let face = match (quad, reverse) {
                    (false, false) => MeshFace::Triangle([0, 1, 2]),
                    (false, true) => MeshFace::Triangle([2, 1, 0]),
                    (true, false) => MeshFace::Quad([0, 1, 2, 3]),
                    (true, true) => MeshFace::Quad([3, 2, 1, 0]),
                };
                let mesh =
                    TriangleMesh::try_new_faces(vertices, vec![face], Tolerance::MESH_VALIDATION)
                        .unwrap();
                let normals = mesh.polygon_face_normals().unwrap();
                let normal = normals[0].as_vector();
                let direction = if reverse { -1.0 } else { 1.0 } / 3.0f64.sqrt();
                for (actual, expected) in [normal.x(), normal.y(), normal.z()]
                    .into_iter()
                    .zip([-direction, -direction, direction])
                {
                    assert!(
                        (actual - expected).abs() <= 4.0 * f64::EPSILON,
                        "scale={scale}, quad={quad}, reverse={reverse}: {actual} != {expected}"
                    );
                }
            }
        }
    }
}

#[test]
fn right_angle_edge_filter_is_invariant_under_extreme_uniform_scaling() {
    for scale in [1.0, 1e200, 1e-200] {
        let vertices = [[0., 0., 0.], [1., 0., 0.], [0., 1., 0.], [0., 0., 1.]]
            .map(|[x, y, z]| Point3::try_new(x * scale, y * scale, z * scale).unwrap())
            .to_vec();
        let mesh = TriangleMesh::try_new(
            vertices,
            vec![[0, 1, 2], [1, 0, 3]],
            Tolerance::MESH_VALIDATION,
        )
        .unwrap();
        let edges = mesh
            .filtered_edge_lines(
                MeshEdgeFilter::FaceAngle {
                    greater_than_radians: std::f64::consts::FRAC_PI_4,
                    less_than_radians: 3.0 * std::f64::consts::FRAC_PI_4,
                },
                Tolerance::MESH_VALIDATION,
            )
            .unwrap();
        assert_eq!(edges.len(), 1, "scale={scale}");
        assert_eq!(edges[0].start(), mesh.vertices()[0]);
        assert_eq!(edges[0].end(), mesh.vertices()[1]);
    }
}
