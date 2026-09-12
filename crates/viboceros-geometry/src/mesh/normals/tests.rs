use super::*;
use crate::{MeshEdgeFilter, Point3, Tolerance};

#[test]
fn polygon_and_facet_normal_indices_are_distinct_and_checked() {
    let vertices = [[0., 0., 0.], [2., 0., 0.], [2., 1., 1.], [0., 1., 0.]]
        .map(|[x, y, z]| Point3::try_new(x, y, z).unwrap())
        .to_vec();
    let mesh = TriangleMesh::try_new_faces(
        vertices,
        vec![MeshFace::Quad([0, 1, 2, 3]), MeshFace::Triangle([3, 2, 0])],
        Tolerance::DEFAULT,
    )
    .unwrap();
    let polygons = mesh.polygon_face_normals().unwrap();
    assert_eq!(polygons.len(), 2);
    assert_eq!(mesh.triangles().len(), 3);
    for (index, expected) in [[0., -1., 1.], [-1., 0., 2.], [1., 0., -2.]]
        .into_iter()
        .enumerate()
    {
        let actual = mesh.face_normal(index).unwrap().as_vector();
        let expected =
            UnitVector3::try_new(expected[0], expected[1], expected[2], Tolerance::DEFAULT)
                .unwrap()
                .as_vector();
        for (actual, expected) in [actual.x(), actual.y(), actual.z()].into_iter().zip([
            expected.x(),
            expected.y(),
            expected.z(),
        ]) {
            assert!((actual - expected).abs() <= 4.0 * f64::EPSILON);
        }
    }
    assert_eq!(polygons[1], mesh.face_normal(2).unwrap());
    assert_ne!(polygons[0], mesh.face_normal(0).unwrap());
    for index in [3, usize::MAX] {
        assert_eq!(
            mesh.face_normal(index),
            Err(GeometryError::TriangleIndexOutOfRange { triangle: index })
        );
    }
}

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
