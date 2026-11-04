use viboceros_geometry::{MeshFace, Point3, SubD, Tolerance, TriangleMesh};
fn tube() -> TriangleMesh {
    TriangleMesh::try_new_faces(
        [
            [0., 0., 0.],
            [3., 0., 0.],
            [3., 2., 0.],
            [0., 2., 0.],
            [0., 0., 4.],
            [3., 0., 4.],
            [3., 2., 4.],
            [0., 2., 4.],
        ]
        .map(|p| Point3::try_from(p).unwrap())
        .to_vec(),
        vec![
            MeshFace::Quad([0, 1, 5, 4]),
            MeshFace::Quad([1, 2, 6, 5]),
            MeshFace::Quad([2, 3, 7, 6]),
            MeshFace::Quad([3, 0, 4, 7]),
        ],
        Tolerance::DEFAULT,
    )
    .unwrap()
}
#[test]
fn cyclic_rings_visit_every_opposite_edge_once_and_leave_sources_unchanged() {
    let mesh = tube();
    let original = mesh.clone();
    let edges = mesh.topology_edge_points();
    let seed = edges.iter().position(|[a, b]| a.z() != b.z()).unwrap();
    let ring = mesh.try_edge_ring(seed).unwrap();
    let faces = mesh.try_face_loop(seed).unwrap();
    assert_eq!(faces.edges, ring.edges);
    assert_eq!(faces.faces, ring.faces);
    assert!(faces.is_closed());
    let index = mesh.edge_ring_index();
    for &edge in &faces.edges {
        assert_eq!(
            index
                .try_face_loop(edge)
                .unwrap()
                .faces
                .into_iter()
                .collect::<std::collections::BTreeSet<_>>(),
            faces.faces.iter().copied().collect()
        );
    }
    assert!(index.try_face_loop(usize::MAX).is_err());
    assert!(ring.is_closed());
    assert_eq!(ring.edges.len(), 4);
    assert_eq!(ring.faces.len(), 4);
    for &i in &ring.edges {
        assert!(edges[i][0].z() != edges[i][1].z());
    }
    let set = ring
        .edges
        .iter()
        .copied()
        .collect::<std::collections::BTreeSet<_>>();
    assert_eq!(set.len(), 4);
    for &i in &ring.edges {
        assert_eq!(
            mesh.try_edge_ring(i)
                .unwrap()
                .edges
                .into_iter()
                .collect::<std::collections::BTreeSet<_>>(),
            set
        );
    }
    assert!(mesh.try_edge_ring(edges.len()).is_err());
    assert_eq!(mesh, original);
    let subd = SubD::try_from_mesh(&mesh, Default::default(), Tolerance::DEFAULT).unwrap();
    let original = subd.clone();
    let seed = subd
        .edges()
        .iter()
        .position(|e| {
            subd.control_points()[e[0] as usize].z() != subd.control_points()[e[1] as usize].z()
        })
        .unwrap();
    let ring = subd.try_edge_ring(seed).unwrap();
    let faces = subd.try_face_loop(seed).unwrap();
    assert_eq!(faces.edges, ring.edges);
    assert_eq!(faces.faces, ring.faces);
    assert!(faces.is_closed());
    assert!(subd.try_face_loop(usize::MAX).is_err());
    assert!(ring.is_closed());
    assert_eq!(ring.edges.len(), 4);
    assert_eq!(ring.faces.len(), 4);
    assert!(subd.try_edge_ring(usize::MAX).is_err());
    assert_eq!(subd, original);
}
