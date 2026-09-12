use super::*;
use std::collections::BTreeSet;

#[test]
fn collapse_moves_retained_coincident_peers_without_merging_their_raw_vertices() {
    let mesh = TriangleMesh::try_new(
        vec![
            point(99., 99., 0.),
            point(0., 0., 0.),
            point(2., 0., 0.),
            point(0., 2., 0.),
            point(0., 0., 0.),
            point(-2., 1., 0.),
            point(-2., -1., 0.),
            point(2., 0., 0.),
            point(4., -1., 0.),
            point(4., 1., 0.),
            point(0., 0., 0.),
        ],
        vec![[1, 2, 3], [4, 5, 6], [7, 8, 9]],
        Tolerance::DEFAULT,
    )
    .unwrap();
    let edge = topology_edge_index_between(&mesh, point(0., 0., 0.), point(2., 0., 0.));
    let collapsed = mesh
        .collapse_topology_edge(edge, Tolerance::DEFAULT)
        .unwrap()
        .unwrap();
    assert_eq!(
        collapsed.vertices(),
        &[
            point(1., 0., 0.),
            point(-2., 1., 0.),
            point(-2., -1., 0.),
            point(1., 0., 0.),
            point(4., -1., 0.),
            point(4., 1., 0.),
        ]
    );
    assert_eq!(collapsed.triangles(), &[[0, 1, 2], [3, 4, 5]]);
}

#[test]
fn face_reduction_matches_distinct_vertex_reference_for_every_index_pattern() {
    for count in [3, 4] {
        for mut code in 0..4usize.pow(count) {
            let indices = (0..count)
                .map(|_| {
                    let index = (code % 4) as u32;
                    code /= 4;
                    index
                })
                .collect::<Vec<_>>();
            let face = if count == 3 {
                MeshFace::Triangle(indices.clone().try_into().unwrap())
            } else {
                MeshFace::Quad(indices.clone().try_into().unwrap())
            };
            let distinct = indices.iter().copied().collect::<BTreeSet<_>>().len();
            let expected = if distinct == count as usize {
                Some(face)
            } else if count == 4 && distinct == 3 {
                // A repeated diagonal is discarded, but a single repeated side
                // produces a triangle beginning at the vertex after that side.
                (0..4)
                    .find(|&side| indices[side] == indices[(side + 1) % 4])
                    .map(|side| {
                        MeshFace::Triangle([
                            indices[(side + 2) % 4],
                            indices[(side + 3) % 4],
                            indices[side],
                        ])
                    })
            } else {
                None
            };
            assert_eq!(reduced_face(face), expected, "{face:?}");
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
fn collapse_midpoint_preserves_small_offsets_and_rounds_subnormal_ties() {
    let tiny = f64::from_bits(1);
    for (first_z, second_z, expected_z) in [
        (tiny, tiny, tiny),
        (tiny, 2.0 * tiny, 2.0 * tiny),
        (-tiny, -2.0 * tiny, -2.0 * tiny),
        (-tiny, 2.0 * tiny, 0.0),
        (1e308, 1e308, 1e308),
    ] {
        for quad in [false, true] {
            let vertices = vec![
                point(0.0, 0.0, first_z),
                point(2.0, 0.0, second_z),
                point(2.0, 2.0, second_z),
                point(0.0, 2.0, first_z),
            ];
            let first = vertices[0];
            let second = vertices[1];
            let faces = if quad {
                vec![MeshFace::Quad([0, 1, 2, 3])]
            } else {
                vec![MeshFace::Triangle([0, 1, 2]), MeshFace::Triangle([0, 2, 3])]
            };
            let mesh = TriangleMesh::try_new_faces(vertices, faces, Tolerance::DEFAULT).unwrap();
            let edge = topology_edge_index_between(&mesh, first, second);
            let collapsed = mesh
                .collapse_topology_edge(edge, Tolerance::DEFAULT)
                .unwrap()
                .unwrap();
            assert_eq!(
                collapsed.vertices(),
                &[
                    point(1.0, 0.0, expected_z),
                    point(2.0, 2.0, second_z),
                    point(0.0, 2.0, first_z),
                ],
                "first_z={first_z:e}, second_z={second_z:e}, quad={quad}"
            );
            assert_eq!(collapsed.faces().len(), 1);
        }
    }
}

#[test]
fn collapses_mesh_edge_to_midpoint_in_rhino_source_order() {
    let vertices = vec![
        point(0.0, 0.0, 0.0),
        point(2.0, 0.0, 0.0),
        point(0.0, 2.0, 0.0),
        point(0.0, 0.0, 2.0),
    ];
    let mesh = TriangleMesh::try_new(
        vertices,
        vec![[0, 2, 1], [0, 1, 3], [1, 2, 3], [2, 0, 3]],
        Tolerance::DEFAULT,
    )
    .unwrap();
    let edge = topology_edge_index_between(&mesh, point(0.0, 0.0, 0.0), point(2.0, 0.0, 0.0));
    let collapsed = mesh
        .collapse_topology_edge(edge, Tolerance::DEFAULT)
        .unwrap()
        .unwrap();
    assert_eq!(
        collapsed.vertices(),
        &[
            point(1.0, 0.0, 0.0),
            point(0.0, 2.0, 0.0),
            point(0.0, 0.0, 2.0),
        ]
    );
    assert_eq!(collapsed.triangles(), &[[0, 1, 2], [1, 0, 2]]);
}

#[test]
fn collapse_mesh_edge_turns_adjacent_quads_into_rotated_triangles() {
    let boundary = TriangleMesh::try_new(
        vec![
            point(0.0, 0.0, 0.0),
            point(2.0, 0.0, 0.0),
            point(2.0, 2.0, 0.0),
            point(0.0, 2.0, 0.0),
        ],
        vec![[0, 1, 2], [0, 2, 3]],
        Tolerance::DEFAULT,
    )
    .unwrap();
    let edge = topology_edge_index_between(&boundary, point(0.0, 0.0, 0.0), point(2.0, 0.0, 0.0));
    let collapsed = boundary
        .collapse_topology_edge(edge, Tolerance::DEFAULT)
        .unwrap()
        .unwrap();
    assert_eq!(
        collapsed.vertices(),
        &[
            point(1.0, 0.0, 0.0),
            point(2.0, 2.0, 0.0),
            point(0.0, 2.0, 0.0),
        ]
    );
    assert_eq!(collapsed.triangles(), &[[0, 1, 2]]);

    let quad = TriangleMesh::try_new_faces(
        vec![
            point(0.0, 2.0, 0.0),
            point(0.0, 0.0, 0.0),
            point(2.0, 0.0, 0.0),
            point(2.0, 2.0, 0.0),
        ],
        vec![MeshFace::Quad([0, 1, 2, 3])],
        Tolerance::DEFAULT,
    )
    .unwrap();
    let edge = topology_edge_index_between(&quad, point(0.0, 0.0, 0.0), point(2.0, 0.0, 0.0));
    let collapsed = quad
        .collapse_topology_edge(edge, Tolerance::DEFAULT)
        .unwrap()
        .unwrap();
    assert_eq!(
        collapsed.vertices(),
        &[
            point(0.0, 2.0, 0.0),
            point(1.0, 0.0, 0.0),
            point(2.0, 2.0, 0.0),
        ]
    );
    assert_eq!(collapsed.triangles(), &[[2, 0, 1]]);
}

#[test]
fn collapse_mesh_edge_preserves_independent_unwelded_components() {
    let mesh = TriangleMesh::try_new(
        vec![
            point(0.0, 0.0, 0.0),
            point(2.0, 0.0, 0.0),
            point(0.0, 2.0, 0.0),
            point(2.0, 0.0, 0.0),
            point(0.0, 0.0, 0.0),
            point(0.0, -2.0, 0.0),
            point(-2.0, 1.0, 0.0),
            point(4.0, -1.0, 0.0),
        ],
        vec![[0, 1, 2], [3, 4, 5], [0, 6, 2], [3, 5, 7]],
        Tolerance::DEFAULT,
    )
    .unwrap();
    let edge = topology_edge_index_between(&mesh, point(0.0, 0.0, 0.0), point(2.0, 0.0, 0.0));
    let collapsed = mesh
        .collapse_topology_edge(edge, Tolerance::DEFAULT)
        .unwrap()
        .unwrap();
    assert_eq!(
        collapsed.vertices(),
        &[
            point(1.0, 0.0, 0.0),
            point(0.0, 2.0, 0.0),
            point(1.0, 0.0, 0.0),
            point(0.0, -2.0, 0.0),
            point(-2.0, 1.0, 0.0),
            point(4.0, -1.0, 0.0),
        ]
    );
    assert_eq!(collapsed.triangles(), &[[0, 4, 1], [2, 3, 5]]);
}

#[test]
fn collapse_mesh_edge_reports_empty_invalid_and_degenerate_results() {
    let square = TriangleMesh::try_new(
        vec![
            point(0.0, 0.0, 0.0),
            point(2.0, 0.0, 0.0),
            point(2.0, 2.0, 0.0),
            point(0.0, 2.0, 0.0),
        ],
        vec![[0, 1, 2], [0, 2, 3]],
        Tolerance::DEFAULT,
    )
    .unwrap();
    let diagonal = topology_edge_index_between(&square, point(0.0, 0.0, 0.0), point(2.0, 2.0, 0.0));
    assert_eq!(
        square
            .collapse_topology_edge(diagonal, Tolerance::DEFAULT)
            .unwrap(),
        None
    );
    let edge_count = square.topology().edge_count();
    assert_eq!(
        square.collapse_topology_edge(edge_count, Tolerance::DEFAULT),
        Err(GeometryError::MeshTopologyEdgeIndexOutOfRange {
            edge: edge_count,
            edge_count,
        })
    );

    let degenerate = TriangleMesh::try_new(
        vec![
            point(0.0, 0.0, 0.0),
            point(2.0, 0.0, 0.0),
            point(0.0, -2.0, 0.0),
            point(0.0, 1.0, 0.0),
            point(2.0, -1.0, 0.0),
        ],
        vec![[0, 1, 2], [0, 3, 4]],
        Tolerance::DEFAULT,
    )
    .unwrap();
    let edge = topology_edge_index_between(&degenerate, point(0.0, 0.0, 0.0), point(2.0, 0.0, 0.0));
    assert_eq!(
        degenerate.collapse_topology_edge(edge, Tolerance::DEFAULT),
        Err(GeometryError::DegenerateTriangle { triangle: 0 })
    );

    let disconnected_quad = TriangleMesh::try_new_faces(
        vec![
            point(0.0, 0.0, 0.0),
            point(2.0, 0.0, 0.0),
            point(0.0, -2.0, 0.0),
            point(0.0, 0.0, 0.0),
            point(-2.0, 1.0, 0.0),
            point(2.0, 0.0, 0.0),
            point(3.0, 1.0, 0.0),
        ],
        vec![MeshFace::Triangle([0, 1, 2]), MeshFace::Quad([3, 4, 5, 6])],
        Tolerance::DEFAULT,
    )
    .unwrap();
    let edge = topology_edge_index_between(
        &disconnected_quad,
        point(0.0, 0.0, 0.0),
        point(2.0, 0.0, 0.0),
    );
    assert_eq!(
        disconnected_quad.collapse_topology_edge(edge, Tolerance::DEFAULT),
        Err(GeometryError::DegenerateQuad { face: 0 })
    );
}
