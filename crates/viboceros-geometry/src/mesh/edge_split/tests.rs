use super::*;

#[test]
fn interior_splits_preserve_planar_area_and_winding_for_every_side_and_seam() {
    for corners in [3, 4] {
        for upper_rotation in 0..corners {
            for lower_rotation in 0..corners {
                for reverse in [false, true] {
                    // Shared endpoints, one shared endpoint, and no shared endpoints.
                    for seam in 0..3 {
                        for parameter in [0.125, 0.5, 0.875] {
                            let mut vertices = if corners == 3 {
                                vec![point(0., 0., 0.), point(4., 0., 0.), point(0., 3., 0.)]
                            } else {
                                vec![
                                    point(0., 0., 0.),
                                    point(4., 0., 0.),
                                    point(4., 3., 0.),
                                    point(0., 3., 0.),
                                ]
                            };
                            let mut upper = (0..corners as u32).collect::<Vec<_>>();
                            let mut lower = Vec::new();
                            for (endpoint, source) in [1, 0].into_iter().enumerate() {
                                if seam == 0 || (seam == 1 && endpoint == 0) {
                                    lower.push(source as u32);
                                } else {
                                    lower.push(vertices.len() as u32);
                                    vertices.push(vertices[source]);
                                }
                            }
                            let lower_corners = if corners == 3 {
                                vec![point(4., -3., 0.)]
                            } else {
                                vec![point(0., -3., 0.), point(4., -3., 0.)]
                            };
                            for point in lower_corners {
                                lower.push(vertices.len() as u32);
                                vertices.push(point);
                            }
                            upper.rotate_left(upper_rotation);
                            lower.rotate_left(lower_rotation);
                            if reverse {
                                upper.reverse();
                                lower.reverse();
                            }
                            let face = |indices: Vec<u32>| {
                                if corners == 3 {
                                    MeshFace::Triangle(indices.try_into().unwrap())
                                } else {
                                    MeshFace::Quad(indices.try_into().unwrap())
                                }
                            };
                            let mesh = TriangleMesh::try_new_faces(
                                vertices,
                                vec![face(upper), face(lower)],
                                Tolerance::DEFAULT,
                            )
                            .unwrap();
                            let edge = topology_edge_index_between(
                                &mesh,
                                point(0., 0., 0.),
                                point(4., 0., 0.),
                            );
                            let split = mesh
                                .split_topology_edge(edge, parameter, Tolerance::DEFAULT)
                                .unwrap()
                                .unwrap();
                            let context = format!(
                                "corners={corners}, rotations={upper_rotation}/{lower_rotation}, reverse={reverse}, seam={seam}, t={parameter}"
                            );
                            let sign = if reverse { -1.0 } else { 1.0 };
                            let expected_area = if corners == 3 { 12.0 } else { 24.0 };
                            let mut twice_signed_area = 0.0;
                            for triangle in split.triangles() {
                                let [a, b, c] = triangle.map(|raw| split.vertices()[raw as usize]);
                                let determinant = (b.x() - a.x()) * (c.y() - a.y())
                                    - (b.y() - a.y()) * (c.x() - a.x());
                                assert!(sign * determinant > 0.0, "{context}");
                                twice_signed_area += determinant;
                            }
                            assert_eq!(twice_signed_area, sign * 2.0 * expected_area, "{context}");
                            assert_eq!(split.area().unwrap(), expected_area, "{context}");
                            assert_eq!(split.faces().len(), 2 * (corners - 1), "{context}");
                            let split_point = point(4.0 * parameter, 0., 0.);
                            let copies = split
                                .vertices()
                                .iter()
                                .filter(|&&p| p == split_point)
                                .count();
                            assert_eq!(
                                copies,
                                if seam == 0 { 1 } else { split.faces().len() },
                                "{context}"
                            );
                            assert_eq!(
                                split.vertices().len(),
                                if seam == 0 {
                                    2 * corners - 1
                                } else {
                                    6 * (corners - 1)
                                },
                                "{context}"
                            );
                            assert!(
                                split
                                    .vertices()
                                    .iter()
                                    .all(|p| *p == split_point || mesh.vertices().contains(p)),
                                "{context}"
                            );
                        }
                    }
                }
            }
        }
    }
}

fn point(x: f64, y: f64, z: f64) -> Point3 {
    Point3::try_new(x, y, z).unwrap()
}

fn topology_edge_index_between(mesh: &TriangleMesh, first: Point3, second: Point3) -> usize {
    mesh.wireframe_lines(Tolerance::DEFAULT)
        .unwrap()
        .iter()
        .position(|edge| {
            (edge.start() == first && edge.end() == second)
                || (edge.start() == second && edge.end() == first)
        })
        .expect("test topology edge exists")
}

#[test]
fn splits_welded_mesh_edges_in_rhino_face_and_vertex_order() {
    let mesh = TriangleMesh::try_new(
        vec![
            point(0.0, 0.0, 0.0),
            point(4.0, 0.0, 0.0),
            point(0.0, 4.0, 0.0),
            point(0.0, 0.0, 4.0),
        ],
        vec![[0, 2, 1], [0, 1, 3], [1, 2, 3], [2, 0, 3]],
        Tolerance::DEFAULT,
    )
    .unwrap();
    let edge = topology_edge_index_between(&mesh, point(0.0, 0.0, 0.0), point(4.0, 0.0, 0.0));
    let split = mesh
        .split_topology_edge(edge, 0.25, Tolerance::DEFAULT)
        .unwrap()
        .unwrap();
    assert_eq!(
        split.vertices(),
        &[
            point(0.0, 0.0, 0.0),
            point(4.0, 0.0, 0.0),
            point(0.0, 4.0, 0.0),
            point(0.0, 0.0, 4.0),
            point(1.0, 0.0, 0.0),
        ]
    );
    assert_eq!(
        split.triangles(),
        &[
            [1, 2, 3],
            [2, 0, 3],
            [2, 4, 0],
            [2, 1, 4],
            [3, 0, 4],
            [3, 4, 1],
        ]
    );

    let quad = TriangleMesh::try_new_faces(
        vec![
            point(0.0, 4.0, 0.0),
            point(0.0, 0.0, 0.0),
            point(4.0, 0.0, 0.0),
            point(4.0, 4.0, 0.0),
        ],
        vec![MeshFace::Quad([0, 1, 2, 3])],
        Tolerance::DEFAULT,
    )
    .unwrap();
    let edge = topology_edge_index_between(&quad, point(0.0, 0.0, 0.0), point(4.0, 0.0, 0.0));
    let split = quad
        .split_topology_edge(edge, 0.25, Tolerance::DEFAULT)
        .unwrap()
        .unwrap();
    assert_eq!(
        split.faces(),
        &[
            MeshFace::Triangle([0, 4, 3]),
            MeshFace::Triangle([0, 1, 4]),
            MeshFace::Triangle([3, 4, 2]),
        ]
    );
}

#[test]
fn split_mesh_edge_fully_separates_unwelded_replacement_faces() {
    let mesh = TriangleMesh::try_new(
        vec![
            point(0.0, 0.0, 0.0),
            point(4.0, 0.0, 0.0),
            point(0.0, 4.0, 0.0),
            point(4.0, 0.0, 0.0),
            point(0.0, 0.0, 0.0),
            point(0.0, -4.0, 0.0),
            point(-2.0, 1.0, 0.0),
            point(6.0, -1.0, 0.0),
        ],
        vec![[0, 1, 2], [3, 4, 5], [0, 6, 2], [3, 5, 7]],
        Tolerance::DEFAULT,
    )
    .unwrap();
    let edge = topology_edge_index_between(&mesh, point(0.0, 0.0, 0.0), point(4.0, 0.0, 0.0));
    let split = mesh
        .split_topology_edge(edge, 0.25, Tolerance::DEFAULT)
        .unwrap()
        .unwrap();
    assert_eq!(
        split.faces(),
        &[
            MeshFace::Triangle([0, 4, 1]),
            MeshFace::Triangle([2, 3, 5]),
            MeshFace::Triangle([6, 7, 8]),
            MeshFace::Triangle([9, 10, 11]),
            MeshFace::Triangle([12, 14, 13]),
            MeshFace::Triangle([15, 17, 16]),
        ]
    );
    assert_eq!(split.vertices().len(), 18);
    assert_eq!(split.vertices()[6], point(0.0, 4.0, 0.0));
    assert_eq!(split.vertices()[7], point(0.0, 0.0, 0.0));
    assert_eq!(split.vertices()[8], point(1.0, 0.0, 0.0));
    assert_eq!(split.vertices()[12], point(0.0, -4.0, 0.0));
    assert_eq!(split.vertices()[13], point(0.0, 0.0, 0.0));
    assert_eq!(split.vertices()[14], point(1.0, 0.0, 0.0));
}

#[test]
fn split_mesh_edge_matches_endpoint_rejection_and_validation_behavior() {
    let triangle = TriangleMesh::try_new(
        vec![
            point(0.0, 0.0, 0.0),
            point(4.0, 0.0, 0.0),
            point(0.0, 4.0, 0.0),
        ],
        vec![[0, 1, 2]],
        Tolerance::DEFAULT,
    )
    .unwrap();
    let edge = topology_edge_index_between(&triangle, point(0.0, 0.0, 0.0), point(4.0, 0.0, 0.0));
    let endpoint = triangle
        .split_topology_edge(edge, 0.0, Tolerance::DEFAULT)
        .unwrap()
        .unwrap();
    assert_eq!(
        endpoint.vertices(),
        &[
            point(0.0, 0.0, 0.0),
            point(4.0, 0.0, 0.0),
            point(0.0, 4.0, 0.0),
            point(0.0, 0.0, 0.0),
        ]
    );
    assert_eq!(endpoint.triangles(), &[[0, 1, 2], [2, 3, 1]]);
    assert_eq!(
        triangle
            .split_topology_edge(edge, -0.25, Tolerance::DEFAULT)
            .unwrap(),
        None
    );
    assert_eq!(
        triangle
            .split_topology_edge(edge, 1.25, Tolerance::DEFAULT)
            .unwrap(),
        None
    );
    assert_eq!(
        triangle
            .split_topology_edge(edge, f64::NAN, Tolerance::DEFAULT)
            .unwrap(),
        None
    );
    assert_eq!(
        triangle.split_topology_edge(edge, 1.0e-12, Tolerance::DEFAULT),
        Err(GeometryError::DegenerateTriangle { triangle: 0 })
    );
    let edge_count = triangle.topology().edge_count();
    assert_eq!(
        triangle.split_topology_edge(edge_count, 0.5, Tolerance::DEFAULT),
        Err(GeometryError::MeshTopologyEdgeIndexOutOfRange {
            edge: edge_count,
            edge_count,
        })
    );
}

#[test]
fn replacement_limits_match_wide_integer_reference_without_allocating_faces() {
    for triangles in [0, 1, 7, usize::MAX / 2, usize::MAX] {
        for quads in [0, 1, 7, usize::MAX / 3, usize::MAX] {
            let count = 2 * triangles as u128 + 3 * quads as u128;
            let expected = usize::try_from(count).map_err(|_| GeometryError::TooManyMeshFaces);
            assert_eq!(
                replacement_count(triangles, quads),
                expected,
                "triangles={triangles}, quads={quads}"
            );
        }
    }
}

#[test]
fn output_vertex_limits_match_wide_integer_reference_without_allocating_meshes() {
    for retained in [0, 1, 7, u32::MAX as usize, usize::MAX] {
        for generated in [0, 1, 2, usize::MAX / 3, usize::MAX] {
            for welded in [false, true] {
                let count = retained as u128 + if welded { 1 } else { 3 * generated as u128 };
                let expected = if count <= usize::MAX as u128 && count <= u32::MAX as u128 + 1 {
                    Ok(count as usize)
                } else {
                    Err(GeometryError::TooManyMeshVertices)
                };
                assert_eq!(
                    output_vertex_count(retained, generated, welded),
                    expected,
                    "retained={retained}, generated={generated}, welded={welded}"
                );
            }
        }
    }
}
