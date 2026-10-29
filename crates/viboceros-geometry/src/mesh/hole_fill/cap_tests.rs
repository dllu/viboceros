use super::super::test_support::point;
use super::*;

#[test]
fn planar_cap_leaves_nonplanar_boundary_open() {
    let mesh = TriangleMesh::try_new(
        vec![
            point(0., 0., 0.),
            point(2., 0., 0.),
            point(2., 2., 0.5),
            point(0., 2., 0.),
            point(1., 1., -2.),
            point(10., 0., 0.),
            point(12., 0., 0.),
            point(10., 2., 0.),
            point(10., 0., -2.),
        ],
        vec![
            [0, 1, 4],
            [1, 2, 4],
            [2, 3, 4],
            [3, 0, 4],
            [5, 6, 8],
            [6, 7, 8],
            [7, 5, 8],
        ],
        Tolerance::DEFAULT,
    )
    .unwrap();
    let (capped, count) = mesh.cap_planar_holes(Tolerance::DEFAULT).unwrap();
    assert_eq!(count, 1);
    assert_eq!(capped.face_count(), mesh.face_count() + 1);
    assert_eq!(capped.topology().boundary_edge_count(), 4);
    assert_eq!(capped.cap_planar_holes(Tolerance::DEFAULT).unwrap().1, 0);
    assert_eq!(mesh.fill_holes(Tolerance::DEFAULT).unwrap().1, 2);
    let (welded, welded_count) = mesh
        .cap_planar_holes_with_crease(Tolerance::DEFAULT, false)
        .unwrap();
    assert_eq!(welded_count, 1);
    assert_eq!(welded.vertices(), mesh.vertices());
    assert_eq!(&welded.faces()[..4], &mesh.faces()[..4]);
    assert_eq!(welded.topology().boundary_edge_count(), 4);
}

#[test]
fn planar_cap_crease_controls_raw_boundary_welding() {
    let mesh = TriangleMesh::try_new(
        vec![
            point(0., 0., 0.),
            point(4., 0., 0.),
            point(0., 4., 0.),
            point(0., 0., 4.),
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
    let (creased, count) = mesh
        .cap_planar_holes_with_crease(Tolerance::DEFAULT, true)
        .unwrap();
    assert_eq!(count, 1);
    assert_eq!(creased.vertices().len(), 7);
    assert_eq!(
        creased.vertex_colors(),
        Some(
            &[
                [10, 0, 0, 0],
                [20, 0, 0, 0],
                [30, 0, 0, 0],
                [40, 0, 0, 0],
                [10, 0, 0, 0],
                [20, 0, 0, 0],
                [30, 0, 0, 0],
            ][..]
        )
    );
    assert!(creased.topology().is_solid());
    assert!(
        !creased
            .filtered_edge_polylines(MeshEdgeFilter::Unwelded, Tolerance::DEFAULT)
            .unwrap()
            .is_empty()
    );

    let (welded, count) = mesh
        .cap_planar_holes_with_crease(Tolerance::DEFAULT, false)
        .unwrap();
    assert_eq!(count, 1);
    assert_eq!(welded.vertices(), mesh.vertices());
    assert_eq!(welded.vertex_colors(), mesh.vertex_colors());
    assert!(welded.topology().is_solid());
    assert!(
        welded
            .filtered_edge_polylines(MeshEdgeFilter::Unwelded, Tolerance::DEFAULT)
            .unwrap()
            .is_empty()
    );
    assert!((welded.signed_volume().unwrap() - creased.signed_volume().unwrap()).abs() < 1e-12);
    assert_eq!(
        mesh.cap_planar_holes(Tolerance::DEFAULT).unwrap().0,
        creased
    );
}

#[test]
fn planar_cap_groups_simple_triangles_into_one_ngon() {
    let mesh = TriangleMesh::try_new(
        vec![
            point(0., 0., 0.),
            point(4., 0., 0.),
            point(4., 4., 0.),
            point(0., 4., 0.),
            point(2., 2., 4.),
        ],
        vec![[0, 1, 4], [1, 2, 4], [2, 3, 4], [3, 0, 4]],
        Tolerance::DEFAULT,
    )
    .unwrap();
    for crease in [true, false] {
        let (capped, count) = mesh
            .cap_planar_holes_with_options(Tolerance::DEFAULT, crease, false)
            .unwrap();
        assert_eq!(count, 1);
        assert_eq!(capped.face_count(), 6);
        assert_eq!(capped.ngons().len(), 1);
        assert_eq!(capped.ngons()[0].faces(), &[4, 5]);
        assert_eq!(capped.ngons()[0].vertices().len(), 4);
        assert_eq!(capped.vertices().len(), if crease { 9 } else { 5 });
        assert!(capped.topology().is_solid());
        assert_eq!(capped.reversed().reversed(), capped);
        assert!(
            capped
                .clone()
                .try_with_ngons(capped.ngons().to_vec())
                .is_ok()
        );
    }
    let source_ngon = mesh
        .try_with_ngons(vec![MeshNgon::from_parts(vec![0, 1, 2, 4], vec![0, 1])])
        .unwrap();
    for crease in [true, false] {
        let (capped, _) = source_ngon
            .cap_planar_holes_with_options(Tolerance::DEFAULT, crease, false)
            .unwrap();
        assert_eq!(capped.ngons().len(), 2);
        assert_eq!(capped.ngons()[0].faces(), &[0, 1]);
        assert_eq!(capped.ngons()[1].faces(), &[4, 5]);
    }
}

#[test]
fn welded_cap_reuses_seamed_boundary_vertices_without_welding_remote_seams() {
    let positions = [
        point(0., 0., 0.),
        point(4., 0., 0.),
        point(0., 4., 0.),
        point(0., 0., 4.),
    ];
    let mesh = TriangleMesh::try_new(
        vec![
            positions[0],
            positions[1],
            positions[3],
            positions[1],
            positions[2],
            positions[3],
            positions[2],
            positions[0],
            positions[3],
        ],
        vec![[0, 1, 2], [3, 4, 5], [6, 7, 8]],
        Tolerance::DEFAULT,
    )
    .unwrap();
    let (welded, count) = mesh
        .cap_planar_holes_with_crease(Tolerance::DEFAULT, false)
        .unwrap();
    assert_eq!(count, 1);
    assert_eq!(welded.vertices(), mesh.vertices());
    assert!(welded.topology().is_solid());
    assert_eq!(
        welded.faces()[3]
            .indices()
            .iter()
            .copied()
            .collect::<BTreeSet<_>>(),
        BTreeSet::from([0, 1, 4])
    );
    // The source's three independent apex copies are not on the cap
    // boundary and remain independent raw vertices.
    assert_eq!(
        [
            welded.faces()[0].indices()[2],
            welded.faces()[1].indices()[2],
            welded.faces()[2].indices()[2]
        ],
        [2, 5, 8]
    );
}

#[test]
fn planar_cap_does_not_fill_nested_ring_boundaries_as_overlapping_disks() {
    let mesh = TriangleMesh::try_new(
        vec![
            point(0., 0., 0.),
            point(4., 0., 0.),
            point(4., 4., 0.),
            point(0., 4., 0.),
            point(1., 1., 0.),
            point(3., 1., 0.),
            point(3., 3., 0.),
            point(1., 3., 0.),
        ],
        vec![
            [0, 1, 5],
            [0, 5, 4],
            [1, 2, 6],
            [1, 6, 5],
            [2, 3, 7],
            [2, 7, 6],
            [3, 0, 4],
            [3, 4, 7],
        ],
        Tolerance::DEFAULT,
    )
    .unwrap();
    assert_eq!(mesh.topology().boundary_edge_count(), 8);
    assert_eq!(
        mesh.cap_planar_holes(Tolerance::DEFAULT).unwrap(),
        (mesh.clone(), 0)
    );
}

#[test]
fn planar_cap_closes_both_annular_ends_of_a_square_tube() {
    let mut vertices = Vec::new();
    for (z, ring) in [
        (0., [[0., 0.], [4., 0.], [4., 4.], [0., 4.]]),
        (2., [[0., 0.], [4., 0.], [4., 4.], [0., 4.]]),
        (0., [[1., 1.], [3., 1.], [3., 3.], [1., 3.]]),
        (2., [[1., 1.], [3., 1.], [3., 3.], [1., 3.]]),
    ] {
        vertices.extend(ring.map(|[x, y]| point(x, y, z)));
    }
    let mut faces = Vec::new();
    for side in 0..4_u32 {
        let next = (side + 1) % 4;
        faces.push(MeshFace::Quad([side, next, next + 4, side + 4]));
        faces.push(MeshFace::Quad([side + 8, side + 12, next + 12, next + 8]));
    }
    let wall = TriangleMesh::try_new_faces(vertices, faces, Tolerance::DEFAULT)
        .unwrap()
        .try_with_vertex_colors(Some((0..16).map(|value| [value, 0, 0, 0]).collect()))
        .unwrap();
    assert_eq!(wall.topology().boundary_edge_count(), 16);
    let (capped, count) = wall.cap_planar_holes(Tolerance::DEFAULT).unwrap();
    assert_eq!(
        capped.vertex_colors().unwrap().len(),
        capped.vertices().len()
    );
    for (&vertex, &color) in capped
        .vertices()
        .iter()
        .zip(capped.vertex_colors().unwrap())
    {
        let source = wall
            .vertices()
            .iter()
            .position(|&candidate| candidate == vertex)
            .unwrap();
        assert_eq!(color, wall.vertex_colors().unwrap()[source]);
    }
    assert_eq!(count, 4);
    assert_eq!(capped.face_count(), 24);
    assert!(capped.topology().is_solid());
    assert!((capped.signed_volume().unwrap() - 24.).abs() < 1e-12);
    assert_eq!(capped.cap_planar_holes(Tolerance::DEFAULT).unwrap().1, 0);
    let (welded, welded_count) = wall
        .cap_planar_holes_with_crease(Tolerance::DEFAULT, false)
        .unwrap();
    assert_eq!(welded_count, 4);
    assert_eq!(welded.vertices(), wall.vertices());
    assert_eq!(welded.vertex_colors(), wall.vertex_colors());
    assert_eq!(welded.face_count(), capped.face_count());
    assert!(welded.topology().is_solid());
    assert!(
        welded
            .filtered_edge_polylines(MeshEdgeFilter::Unwelded, Tolerance::DEFAULT)
            .unwrap()
            .is_empty()
    );
    assert!((welded.signed_volume().unwrap() - 24.).abs() < 1e-12);
    let (ngon_mode, _) = wall
        .cap_planar_holes_with_options(Tolerance::DEFAULT, true, false)
        .unwrap();
    assert!(ngon_mode.ngons().is_empty());

    let decorated = wall
        .clone()
        .try_with_ngons(vec![MeshNgon::from_parts(vec![0, 1, 5, 4], vec![0])])
        .unwrap();
    for crease in [false, true] {
        let (capped, count) = decorated
            .cap_planar_holes_with_crease(Tolerance::DEFAULT, crease)
            .unwrap();
        assert_eq!(count, 4);
        assert_eq!(capped.ngons().len(), 1);
        assert!(
            capped
                .clone()
                .try_with_ngons(capped.ngons().to_vec())
                .is_ok()
        );
    }

    let rotation = AffineTransform3::try_rotation(
        point(0., 0., 0.),
        UnitVector3::try_new(1., 1., 1., Tolerance::DEFAULT).unwrap(),
        0.37,
    )
    .unwrap();
    let rotated = wall.transformed(rotation, Tolerance::DEFAULT).unwrap();
    let (rotated_cap, rotated_count) = rotated.cap_planar_holes(Tolerance::DEFAULT).unwrap();
    assert_eq!(rotated_count, 4);
    assert!(rotated_cap.topology().is_solid());
    assert!((rotated_cap.signed_volume().unwrap() - 24.).abs() < 1e-9);
}

#[test]
fn planar_cap_supports_two_inner_boundaries_per_end() {
    let rings = [
        [[0., 0.], [10., 0.], [10., 10.], [0., 10.]],
        [[1., 1.], [3., 1.], [3., 3.], [1., 3.]],
        [[7., 7.], [9., 7.], [9., 9.], [7., 9.]],
    ];
    let mut vertices = Vec::new();
    let mut faces = Vec::new();
    for (ring_index, ring) in rings.into_iter().enumerate() {
        let base = u32::try_from(vertices.len()).unwrap();
        vertices.extend(ring.map(|[x, y]| point(x, y, 0.)));
        vertices.extend(ring.map(|[x, y]| point(x, y, 2.)));
        for side in 0..4_u32 {
            let next = (side + 1) % 4;
            let indices = if ring_index == 0 {
                [base + side, base + next, base + next + 4, base + side + 4]
            } else {
                [base + side, base + side + 4, base + next + 4, base + next]
            };
            faces.push(MeshFace::Quad(indices));
        }
    }
    let wall = TriangleMesh::try_new_faces(vertices, faces, Tolerance::DEFAULT).unwrap();
    let (capped, count) = wall.cap_planar_holes(Tolerance::DEFAULT).unwrap();
    assert_eq!(count, 6);
    assert_eq!(capped.face_count(), 40);
    assert!(capped.topology().is_solid());
    assert!((capped.signed_volume().unwrap() - 184.).abs() < 1e-12);
}
