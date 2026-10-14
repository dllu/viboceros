use super::*;
fn p(x: Real, y: Real, z: Real) -> Point3 {
    Point3::try_new(x, y, z).unwrap()
}
fn z() -> Vector3 {
    Vector3::try_new(0., 0., 1.).unwrap()
}
fn triangle(z: Real) -> TriangleMesh {
    TriangleMesh::try_new(
        vec![p(0., 0., z), p(1., 0., z), p(0., 1., z)],
        vec![[0, 1, 2]],
        Tolerance::DEFAULT,
    )
    .unwrap()
}
#[test]
fn exact_membership_keeps_vertices_edges_and_rejects_one_ulp_outside() {
    let mesh = triangle(0.);
    let index = MeshProjectionIndex::new(&mesh).unwrap();
    for source in [p(0., 0., 2.), p(0.5, 0.5, 2.), p(0.25, 0.25, -2.)] {
        assert_eq!(
            index
                .project_point(source, z(), Tolerance::DEFAULT)
                .unwrap(),
            [p(source.x(), source.y(), 0.)]
        );
    }
    assert!(
        index
            .project_point(p(0.5, 0.5_f64.next_up(), 2.), z(), Tolerance::DEFAULT)
            .unwrap()
            .is_empty()
    );
}
#[test]
fn bidirectional_hits_deduplicate_shared_diagonals_but_keep_nearby_sheets() {
    let mesh = TriangleMesh::try_new(
        vec![
            p(0., 0., 0.),
            p(1., 0., 0.),
            p(1., 1., 0.),
            p(0., 1., 0.),
            p(0., 0., 1e-9),
            p(1., 0., 1e-9),
            p(0., 1., 1e-9),
        ],
        vec![[0, 1, 2], [0, 2, 3], [4, 5, 6], [1, 0, 2]],
        Tolerance::DEFAULT,
    )
    .unwrap();
    let index = MeshProjectionIndex::new(&mesh).unwrap();
    assert_eq!(
        index
            .project_point(p(0.5, 0.5, 3.), z(), Tolerance::DEFAULT)
            .unwrap(),
        [p(0.5, 0.5, 0.), p(0.5, 0.5, 1e-9)]
    );
}
#[test]
fn nearly_parallel_hits_and_parallel_misses_are_decided_exactly() {
    let mesh = triangle(0.);
    let index = MeshProjectionIndex::new(&mesh).unwrap();
    assert_eq!(
        index
            .project_point(
                p(1., 0.5, 1e-9),
                Vector3::try_new(1., 0., 1e-9).unwrap(),
                Tolerance::DEFAULT
            )
            .unwrap(),
        [p(0., 0.5, 0.)]
    );
    assert!(
        index
            .project_point(
                p(0.5, 0.5, 1.),
                Vector3::try_new(1., 0., 0.).unwrap(),
                Tolerance::DEFAULT
            )
            .unwrap()
            .is_empty()
    );
    assert!(
        index
            .project_point(
                p(0.5, 0.5, 0.),
                Vector3::try_new(1., 0., 0.).unwrap(),
                Tolerance::DEFAULT
            )
            .is_err()
    );
}
#[test]
fn index_prunes_a_large_grid_and_retains_shared_boundary_candidates() {
    let count = 100;
    let stride = count + 1;
    let vertices = (0..=count)
        .flat_map(|y| (0..=count).map(move |x| p(x as Real, y as Real, 0.)))
        .collect();
    let faces = (0..count)
        .flat_map(|y| {
            (0..count).map(move |x| {
                MeshFace::Quad([
                    (y * stride + x) as u32,
                    (y * stride + x + 1) as u32,
                    ((y + 1) * stride + x + 1) as u32,
                    ((y + 1) * stride + x) as u32,
                ])
            })
        })
        .collect();
    let mesh = TriangleMesh::try_new_faces(vertices, faces, Tolerance::DEFAULT).unwrap();
    let index = MeshProjectionIndex::new(&mesh).unwrap();
    for source in [p(50.25, 50.75, 100.), p(50., 50., 100.)] {
        let candidates = index
            .candidates(BoundingBox3::from_points([source]).unwrap(), z())
            .unwrap();
        assert!(!candidates.is_empty());
        assert!(candidates.len() <= 8);
        assert_eq!(
            index
                .project_point(source, z(), Tolerance::DEFAULT)
                .unwrap(),
            [p(source.x(), source.y(), 0.)]
        );
    }
}
#[test]
fn interval_candidate_rejection_never_discards_an_exact_slab_hit() {
    let mut state = 13_u64;
    for _ in 0..2000 {
        let mut number = || {
            state = state.wrapping_mul(6364136223846793005).wrapping_add(1);
            ((state >> 32) as i32) as Real / 1e8
        };
        let target = BoundingBox3::from_points([
            p(number(), number(), number()),
            p(number(), number(), number()),
        ])
        .unwrap();
        let source = BoundingBox3::from_points([
            p(number(), number(), number()),
            p(number(), number(), number()),
        ])
        .unwrap();
        let d = [number(), number(), number()];
        if exact_slabs(target, source, d) {
            assert!(may_meet(target, source, d));
        }
    }
}
#[test]
fn subnormal_slabs_fall_back_without_losing_a_boundary_hit() {
    let quantum = Real::from_bits(1);
    let a = BoundingBox3::from_points([p(quantum, 0., 0.), p(2. * quantum, 0., 0.)]).unwrap();
    let b = BoundingBox3::from_points([p(0., 0., 0.)]).unwrap();
    assert!(may_meet(a, b, [quantum, 0., 0.]));
    assert!(!may_meet(a, b, [0., quantum, 0.]));
}
#[test]
fn oblique_hit_requires_representable_absolute_error() {
    let mesh = triangle(0.);
    let index = MeshProjectionIndex::new(&mesh).unwrap();
    let result = index.project_point(
        p(0., 0., 1.),
        Vector3::try_new(-1., -1., 3.).unwrap(),
        Tolerance::try_new(1e-20, 1e-30, 1e-8).unwrap(),
    );
    assert!(matches!(
        result,
        Err(GeometryError::UnrepresentableMeshProjection)
    ));
    assert_eq!(
        index
            .project_point(
                p(0., 0., 1.),
                Vector3::try_new(-1., -1., 3.).unwrap(),
                Tolerance::DEFAULT
            )
            .unwrap()
            .len(),
        1
    );
}

#[test]
fn coplanar_bbox_candidates_distinguish_miss_vertex_contact_and_continuum() {
    let mesh = triangle(0.);
    let index = MeshProjectionIndex::new(&mesh).unwrap();
    let d = Vector3::try_new(1., -1., 0.).unwrap();
    assert!(
        index
            .project_point(p(0.75, 0.75, 0.), d, Tolerance::DEFAULT)
            .unwrap()
            .is_empty()
    );
    assert_eq!(
        index
            .project_point(p(-1., 1., 0.), d, Tolerance::DEFAULT)
            .unwrap(),
        [p(0., 0., 0.)]
    );
    assert!(
        index
            .project_point(p(0.5, 0.5, 0.), d, Tolerance::DEFAULT)
            .is_err()
    );
}
#[test]
fn ruled_plane_pruning_keeps_only_a_strip_for_diagonal_line_sources() {
    let count = 50;
    let stride = count + 1;
    let vertices = (0..=count)
        .flat_map(|y| (0..=count).map(move |x| p(x as Real, y as Real, 0.)))
        .collect();
    let faces = (0..count)
        .flat_map(|y| {
            (0..count).map(move |x| {
                MeshFace::Quad([
                    (y * stride + x) as u32,
                    (y * stride + x + 1) as u32,
                    ((y + 1) * stride + x + 1) as u32,
                    ((y + 1) * stride + x) as u32,
                ])
            })
        })
        .collect();
    let mesh = TriangleMesh::try_new_faces(vertices, faces, Tolerance::DEFAULT).unwrap();
    let index = MeshProjectionIndex::new(&mesh).unwrap();
    let curve = crate::LineSegment::try_new(p(0., 0., 3.), p(50., 50., 3.), Tolerance::DEFAULT)
        .unwrap()
        .to_nurbs()
        .unwrap();
    let candidates = index.curve_candidates(&curve, z()).unwrap();
    assert!(candidates.len() <= 6 * count);
    assert!(candidates.len() >= 2 * count);
    for i in 0..count {
        assert!(candidates.contains(&(2 * (i * count + i))));
        assert!(candidates.contains(&(2 * (i * count + i) + 1)));
    }
}

#[test]
fn warped_quad_shorter_diagonal_is_invariant_under_cyclic_face_rotation() {
    let vertices = vec![p(0., 0., 0.), p(1., 0., 0.), p(1., 1., 1.), p(0., 1., 0.)];
    for face in [[0, 1, 2, 3], [1, 2, 3, 0], [2, 3, 0, 1], [3, 0, 1, 2]] {
        let mesh = TriangleMesh::try_new_faces(
            vertices.clone(),
            vec![MeshFace::Quad(face)],
            Tolerance::DEFAULT,
        )
        .unwrap();
        let index = MeshProjectionIndex::new(&mesh).unwrap();
        for source in [p(0.25, 0.75, 3.), p(0.25, 0.25, 3.), p(0.75, 0.75, 3.)] {
            assert_eq!(
                index
                    .project_point(source, z(), Tolerance::DEFAULT)
                    .unwrap(),
                [p(
                    source.x(),
                    source.y(),
                    (source.x() + source.y() - 1.).max(0.)
                )]
            );
        }
    }
}

#[test]
fn candidate_faces_coalesce_exact_coplanar_cells_and_preserve_warped_planes() {
    let curve = crate::LineSegment::try_new(p(-1., 0.25, 3.), p(2., 0.25, 3.), Tolerance::DEFAULT)
        .unwrap()
        .to_nurbs()
        .unwrap();
    let flat = TriangleMesh::try_new_faces(
        vec![p(0., 0., 0.), p(1., 0., 0.), p(1., 1., 0.), p(0., 1., 0.)],
        vec![MeshFace::Quad([0, 1, 2, 3])],
        Tolerance::DEFAULT,
    )
    .unwrap();
    let index = MeshProjectionIndex::new(&flat).unwrap();
    let faces = index.curve_faces(&curve, z(), Tolerance::DEFAULT).unwrap();
    assert_eq!(faces.len(), 1);
    assert!(
        faces[0]
            .bounds()
            .min()
            .distance_to(flat.bounds().min())
            .unwrap()
            < 1e-8
    );
    assert!(
        faces[0]
            .bounds()
            .max()
            .distance_to(flat.bounds().max())
            .unwrap()
            < 1e-8
    );
    let warped = TriangleMesh::try_new_faces(
        vec![p(0., 0., 0.), p(1., 0., 0.), p(1., 1., 1.), p(0., 1., 0.)],
        vec![MeshFace::Quad([0, 1, 2, 3])],
        Tolerance::DEFAULT,
    )
    .unwrap();
    let index = MeshProjectionIndex::new(&warped).unwrap();
    assert_eq!(
        index
            .curve_faces(&curve, z(), Tolerance::DEFAULT)
            .unwrap()
            .len(),
        2
    );
}
