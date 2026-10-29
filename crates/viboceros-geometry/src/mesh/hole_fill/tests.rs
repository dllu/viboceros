use super::super::test_support::{point, topology_edge_index_between};
use super::*;

#[test]
fn fills_a_naked_mesh_hole_with_an_oriented_delaunay_patch() {
    let mesh = TriangleMesh::try_new(
        vec![
            point(0.0, 0.0, 0.0),
            point(4.0, 0.0, 0.0),
            point(0.0, 4.0, 0.0),
            point(0.0, 0.0, 4.0),
        ],
        vec![[0, 1, 3], [1, 2, 3], [2, 0, 3]],
        Tolerance::DEFAULT,
    )
    .unwrap()
    .try_with_vertex_colors(Some(vec![
        [10, 0, 0, 0],
        [20, 0, 0, 0],
        [30, 0, 0, 0],
        [40, 0, 0, 0],
    ]))
    .unwrap();
    let edge = topology_edge_index_between(&mesh, point(0.0, 0.0, 0.0), point(4.0, 0.0, 0.0));
    let fill = mesh
        .fill_topology_hole(edge, Tolerance::DEFAULT)
        .unwrap()
        .unwrap();

    assert_eq!(
        fill.patch().vertices(),
        &[
            point(0.0, 0.0, 0.0),
            point(4.0, 0.0, 0.0),
            point(0.0, 4.0, 0.0),
        ]
    );
    assert_eq!(
        fill.patch().vertex_colors(),
        Some(&[[10, 0, 0, 0], [20, 0, 0, 0], [30, 0, 0, 0]][..])
    );
    assert_eq!(
        &fill.filled().vertex_colors().unwrap()[4..],
        &[[10, 0, 0, 0], [20, 0, 0, 0], [30, 0, 0, 0], [10, 0, 0, 0]]
    );
    let mut patch_triangle = fill.patch().triangles()[0];
    patch_triangle.sort_unstable();
    assert_eq!(patch_triangle, [0, 1, 2]);
    assert_eq!(
        &fill.filled().vertices()[4..],
        &[
            point(0.0, 0.0, 0.0),
            point(4.0, 0.0, 0.0),
            point(0.0, 4.0, 0.0),
            point(0.0, 0.0, 0.0),
        ]
    );
    let mut joined_triangle = *fill.filled().triangles().last().unwrap();
    joined_triangle.sort_unstable();
    assert_eq!(joined_triangle, [4, 5, 6]);
    assert!(fill.filled().topology().is_solid());
}

#[test]
fn fills_concave_and_nonplanar_mesh_holes_deterministically() {
    let concave = TriangleMesh::try_new(
        vec![
            point(0.0, 0.0, 0.0),
            point(4.0, 0.0, 0.0),
            point(4.0, 4.0, 0.0),
            point(2.0, 2.0, 0.0),
            point(0.0, 4.0, 0.0),
            point(2.0, 2.0, 4.0),
        ],
        vec![[0, 1, 5], [1, 2, 5], [2, 3, 5], [3, 4, 5], [4, 0, 5]],
        Tolerance::DEFAULT,
    )
    .unwrap();
    let edge = topology_edge_index_between(&concave, point(0.0, 0.0, 0.0), point(4.0, 0.0, 0.0));
    let patch = concave
        .fill_topology_hole(edge, Tolerance::DEFAULT)
        .unwrap()
        .unwrap()
        .patch()
        .clone();
    let mut triangles = patch
        .triangles()
        .iter()
        .map(|triangle| {
            let mut triangle = *triangle;
            triangle.sort_unstable();
            triangle
        })
        .collect::<Vec<_>>();
    triangles.sort_unstable();
    assert_eq!(triangles, [[0, 1, 3], [0, 3, 4], [1, 2, 3]]);

    let nonplanar = TriangleMesh::try_new(
        vec![
            point(0.0, 0.0, 0.0),
            point(4.0, 0.0, 1.0),
            point(4.0, 4.0, 0.0),
            point(0.0, 4.0, -1.0),
            point(2.0, 2.0, 4.0),
        ],
        vec![[0, 1, 4], [1, 2, 4], [2, 3, 4], [3, 0, 4]],
        Tolerance::DEFAULT,
    )
    .unwrap();
    let edge = topology_edge_index_between(&nonplanar, point(0.0, 0.0, 0.0), point(4.0, 0.0, 1.0));
    let filled = nonplanar
        .fill_topology_hole(edge, Tolerance::DEFAULT)
        .unwrap()
        .unwrap()
        .filled()
        .clone();
    assert_eq!(filled.face_count(), 6);
    assert!(filled.topology().is_solid());
}

#[test]
fn mesh_hole_fill_rejects_non_naked_open_and_branched_boundaries() {
    let square = TriangleMesh::try_new(
        vec![
            point(0.0, 0.0, 0.0),
            point(4.0, 0.0, 0.0),
            point(4.0, 4.0, 0.0),
            point(0.0, 4.0, 0.0),
        ],
        vec![[0, 1, 2], [0, 2, 3]],
        Tolerance::DEFAULT,
    )
    .unwrap();
    let diagonal = topology_edge_index_between(&square, point(0.0, 0.0, 0.0), point(4.0, 4.0, 0.0));
    assert_eq!(
        square
            .fill_topology_hole(diagonal, Tolerance::DEFAULT)
            .unwrap(),
        None
    );

    let branched = TriangleMesh::try_new(
        vec![
            point(0.0, 0.0, 0.0),
            point(2.0, 0.0, 0.0),
            point(0.0, 2.0, 0.0),
            point(-2.0, 0.0, 0.0),
            point(0.0, -2.0, 0.0),
        ],
        vec![[0, 1, 2], [0, 3, 4]],
        Tolerance::DEFAULT,
    )
    .unwrap();
    let edge = topology_edge_index_between(&branched, point(0.0, 0.0, 0.0), point(2.0, 0.0, 0.0));
    assert_eq!(
        branched
            .fill_topology_hole(edge, Tolerance::DEFAULT)
            .unwrap(),
        None
    );

    let edge_count = square.topology().edge_count();
    assert_eq!(
        square.fill_topology_hole(edge_count, Tolerance::DEFAULT),
        Err(GeometryError::MeshTopologyEdgeIndexOutOfRange {
            edge: edge_count,
            edge_count,
        })
    );
}

#[test]
fn fills_all_simple_mesh_holes_without_unused_closing_vertices() {
    let mesh = TriangleMesh::try_new(
        vec![
            point(0.0, 0.0, 0.0),
            point(4.0, 0.0, 0.0),
            point(0.0, 4.0, 0.0),
            point(0.0, 0.0, 4.0),
            point(10.0, 0.0, 0.0),
            point(14.0, 0.0, 0.0),
            point(10.0, 4.0, 0.0),
            point(10.0, 0.0, 4.0),
        ],
        vec![
            [0, 1, 3],
            [1, 2, 3],
            [2, 0, 3],
            [4, 5, 7],
            [5, 6, 7],
            [6, 4, 7],
        ],
        Tolerance::DEFAULT,
    )
    .unwrap();
    let (filled, hole_count) = mesh.fill_holes(Tolerance::DEFAULT).unwrap();
    assert_eq!(hole_count, 2);
    assert_eq!(filled.face_count(), 8);
    assert_eq!(filled.vertices().len(), 14);
    assert_eq!(
        &filled.vertices()[8..],
        &[
            point(0.0, 0.0, 0.0),
            point(4.0, 0.0, 0.0),
            point(0.0, 4.0, 0.0),
            point(10.0, 0.0, 0.0),
            point(14.0, 0.0, 0.0),
            point(10.0, 4.0, 0.0),
        ]
    );
    assert_eq!(filled.topology().boundary_edge_count(), 0);
    assert_eq!(filled.topology().orientation_conflict_edge_count(), 0);

    let (unchanged, hole_count) = filled.fill_holes(Tolerance::DEFAULT).unwrap();
    assert_eq!(hole_count, 0);
    assert_eq!(unchanged, filled);
}

#[test]
fn fill_all_holes_leaves_ambiguous_branched_boundaries_unchanged() {
    let branched = TriangleMesh::try_new(
        vec![
            point(0.0, 0.0, 0.0),
            point(2.0, 0.0, 0.0),
            point(0.0, 2.0, 0.0),
            point(-2.0, 0.0, 0.0),
            point(0.0, -2.0, 0.0),
        ],
        vec![[0, 1, 2], [0, 3, 4]],
        Tolerance::DEFAULT,
    )
    .unwrap();
    assert_eq!(
        branched.fill_holes(Tolerance::DEFAULT).unwrap(),
        (branched.clone(), 0)
    );
}
