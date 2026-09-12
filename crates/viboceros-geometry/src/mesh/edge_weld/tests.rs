use super::*;
use crate::{Point3, Tolerance};

fn point(x: f64, y: f64, z: f64) -> Point3 {
    Point3::try_new(x, y, z).unwrap()
}

#[test]
fn welds_selected_topology_edges_with_earliest_survivors_and_compaction() {
    let mesh = TriangleMesh::try_new(
        vec![
            point(0.0, 0.0, 0.0),
            point(4.0, 0.0, 0.0),
            point(0.0, 3.0, 0.0),
            point(4.0, 0.0, 0.0),
            point(0.0, 0.0, 0.0),
            point(0.0, -3.0, 0.0),
            point(99.0, 99.0, 99.0),
        ],
        vec![[0, 1, 2], [3, 4, 5]],
        Tolerance::DEFAULT,
    )
    .unwrap();
    let (welded, edge_count) = mesh.welded_topology_edges(&[0]).unwrap();
    assert_eq!(edge_count, 1);
    assert_eq!(
        welded.vertices(),
        &[
            point(0.0, 0.0, 0.0),
            point(4.0, 0.0, 0.0),
            point(0.0, 3.0, 0.0),
            point(0.0, -3.0, 0.0),
        ]
    );
    assert_eq!(welded.triangles(), &[[0, 1, 2], [1, 0, 3]]);
    assert_eq!(
        mesh.welded_topology_edges(&[0, 0]).unwrap(),
        (welded.clone(), 1)
    );

    let (empty, edge_count) = mesh.welded_topology_edges(&[]).unwrap();
    assert_eq!((empty, edge_count), (mesh.clone(), 0));
    for (indices, invalid) in [
        (vec![5], 5),
        (vec![0, 5], 5),
        (vec![usize::MAX, 5], usize::MAX),
        (vec![5, usize::MAX], 5),
        (vec![0, 0, usize::MAX], usize::MAX),
    ] {
        assert_eq!(
            mesh.welded_topology_edges(&indices),
            Err(GeometryError::MeshTopologyEdgeIndexOutOfRange {
                edge: invalid,
                edge_count: 5
            })
        );
    }

    let already_welded = TriangleMesh::try_new(
        vec![
            point(0.0, 0.0, 0.0),
            point(4.0, 0.0, 0.0),
            point(0.0, 3.0, 0.0),
            point(0.0, -3.0, 0.0),
            point(99.0, 99.0, 99.0),
        ],
        vec![[0, 1, 2], [1, 0, 3]],
        Tolerance::DEFAULT,
    )
    .unwrap();
    assert_eq!(
        already_welded.welded_topology_edges(&[0]).unwrap(),
        (welded.clone(), 0)
    );

    let half_welded = TriangleMesh::try_new(
        vec![
            point(0.0, 0.0, 0.0),
            point(4.0, 0.0, 0.0),
            point(0.0, 3.0, 0.0),
            point(4.0, 0.0, 0.0),
            point(0.0, -3.0, 0.0),
            point(99.0, 99.0, 99.0),
        ],
        vec![[0, 1, 2], [3, 0, 4]],
        Tolerance::DEFAULT,
    )
    .unwrap();
    assert_eq!(
        half_welded.welded_topology_edges(&[0]).unwrap(),
        (welded.clone(), 1)
    );

    let naked = TriangleMesh::try_new(
        vec![
            point(0.0, 0.0, 0.0),
            point(4.0, 0.0, 0.0),
            point(0.0, 3.0, 0.0),
            point(99.0, 99.0, 99.0),
        ],
        vec![[0, 1, 2]],
        Tolerance::DEFAULT,
    )
    .unwrap();
    let (compacted, edge_count) = naked.welded_topology_edges(&[0]).unwrap();
    assert_eq!(edge_count, 0);
    assert_eq!(compacted.vertices(), &naked.vertices()[..3]);
    assert_eq!(compacted.triangles(), naked.triangles());
}

#[test]
fn selected_edge_welding_handles_closed_non_manifold_and_disjoint_seams() {
    let fan = TriangleMesh::try_new(
        vec![
            point(0.0, 0.0, 0.0),
            point(1.0, 0.0, 0.0),
            point(0.0, 1.0, 0.0),
            point(-1.0, 0.0, 0.0),
            point(0.0, -1.0, 0.0),
            point(0.0, 0.0, 0.0),
            point(1.0, 0.0, 0.0),
        ],
        vec![[0, 2, 1], [0, 3, 2], [0, 4, 3], [5, 6, 4]],
        Tolerance::DEFAULT,
    )
    .unwrap();
    let (welded, edge_count) = fan.welded_topology_edges(&[0]).unwrap();
    assert_eq!(edge_count, 1);
    assert_eq!(welded.vertices(), &fan.vertices()[..5]);
    assert_eq!(
        welded.triangles(),
        &[[0, 2, 1], [0, 3, 2], [0, 4, 3], [0, 1, 4]]
    );

    let non_manifold = TriangleMesh::try_new(
        vec![
            point(0.0, 0.0, 0.0),
            point(1.0, 0.0, 0.0),
            point(0.0, 1.0, 0.0),
            point(1.0, 0.0, 0.0),
            point(0.0, 0.0, 0.0),
            point(0.0, -1.0, 0.0),
            point(0.0, 0.0, 0.0),
            point(1.0, 0.0, 0.0),
            point(0.0, 0.0, 1.0),
            point(99.0, 99.0, 99.0),
        ],
        vec![[0, 1, 2], [3, 4, 5], [6, 7, 8]],
        Tolerance::DEFAULT,
    )
    .unwrap();
    let (welded, edge_count) = non_manifold.welded_topology_edges(&[0]).unwrap();
    assert_eq!(edge_count, 1);
    assert_eq!(
        welded.vertices(),
        &[
            point(0.0, 0.0, 0.0),
            point(1.0, 0.0, 0.0),
            point(0.0, 1.0, 0.0),
            point(0.0, -1.0, 0.0),
            point(0.0, 0.0, 1.0),
        ]
    );
    assert_eq!(welded.triangles(), &[[0, 1, 2], [1, 0, 3], [0, 1, 4]]);

    let disjoint = TriangleMesh::try_new(
        vec![
            point(0.0, 0.0, 0.0),
            point(1.0, 0.0, 0.0),
            point(0.0, 1.0, 0.0),
            point(1.0, 0.0, 0.0),
            point(0.0, 0.0, 0.0),
            point(0.0, -1.0, 0.0),
            point(10.0, 0.0, 0.0),
            point(11.0, 0.0, 0.0),
            point(10.0, 1.0, 0.0),
            point(11.0, 0.0, 0.0),
            point(10.0, 0.0, 0.0),
            point(10.0, -1.0, 0.0),
        ],
        vec![[0, 1, 2], [3, 4, 5], [6, 7, 8], [9, 10, 11]],
        Tolerance::DEFAULT,
    )
    .unwrap();
    let (welded, edge_count) = disjoint.welded_topology_edges(&[5, 0]).unwrap();
    assert_eq!(edge_count, 2);
    assert_eq!(welded.vertices().len(), 8);
    assert_eq!(
        welded.triangles(),
        &[[0, 1, 2], [1, 0, 3], [4, 5, 6], [5, 4, 7]]
    );
}
