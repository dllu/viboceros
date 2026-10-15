use super::*;
fn p(x: f64, y: f64, z: f64) -> Point3 {
    Point3::try_new(x, y, z).unwrap()
}
fn tol() -> Tolerance {
    Tolerance::try_new(1e-6, 1e-12, 1e-10).unwrap()
}
fn path(vertices: Vec<Point3>) -> Polyline3 {
    Polyline3::try_new(vertices, Tolerance::NUMERICAL_VALIDATION).unwrap()
}
fn square() -> TriangleMesh {
    TriangleMesh::try_new(
        vec![
            p(-4., -4., 0.),
            p(4., -4., 0.),
            p(4., 4., 0.),
            p(-4., 4., 0.),
        ],
        vec![[0, 1, 2], [0, 2, 3]],
        Tolerance::DEFAULT,
    )
    .unwrap()
}
fn check_on_mesh(mesh: &TriangleMesh, polyline: &Polyline3) {
    let index = MeshProjectionIndex::new(mesh).unwrap();
    for pair in polyline.vertices().windows(2) {
        for i in 0..=16 {
            let q = Point3::try_from(std::array::from_fn::<_, 3, _>(|j| {
                pair[0].to_array()[j]
                    + (pair[1].to_array()[j] - pair[0].to_array()[j]) * i as f64 / 16.
            }))
            .unwrap();
            let foot = index.closest_point(q).unwrap().unwrap();
            assert!(q.distance_to(foot).unwrap() < 1e-6, "{q:?} -> {foot:?}");
        }
    }
}
#[test]
fn flat_faces_keep_boundary_clamping_and_internal_diagonal_crossings() {
    let mesh = square();
    let source = path(vec![p(-6., 1., 3.), p(6., 1., 3.)]);
    let output = MeshProjectionIndex::new(&mesh)
        .unwrap()
        .pull_polyline(&source, tol())
        .unwrap()
        .unwrap();
    assert_eq!(output.vertices().first(), Some(&p(-4., 1., 0.)));
    assert_eq!(output.vertices().last(), Some(&p(4., 1., 0.)));
    assert!((output.length().unwrap() - 8.).abs() < 1e-10);
    check_on_mesh(&mesh, &output);
}
#[test]
fn face_edge_and_vertex_regions_resolve_without_source_sampling() {
    let mesh = TriangleMesh::try_new(
        vec![p(0., 0., 0.), p(4., 0., 0.), p(0., 4., 0.)],
        vec![[0, 1, 2]],
        Tolerance::DEFAULT,
    )
    .unwrap();
    let source = path(vec![p(-1., -1., 2.), p(5., 2., 2.)]);
    let index = MeshProjectionIndex::new(&mesh).unwrap();
    let output = index.pull_polyline(&source, tol()).unwrap().unwrap();
    assert_eq!(output.vertices().first(), Some(&p(0., 0., 0.)));
    assert_eq!(output.vertices().last(), Some(&p(3.5, 0.5, 0.)));
    assert!(output.vertices().contains(&p(1., 0., 0.)));
    check_on_mesh(&mesh, &output);
}
#[test]
fn folded_faces_follow_the_shared_edge_instead_of_an_off_mesh_chord() {
    let mesh = TriangleMesh::try_new(
        vec![
            p(-4., 0., 0.),
            p(0., -4., 0.),
            p(0., 4., 0.),
            p(4., 0., -4.),
        ],
        vec![[0, 1, 2], [1, 3, 2]],
        Tolerance::DEFAULT,
    )
    .unwrap();
    let output = MeshProjectionIndex::new(&mesh)
        .unwrap()
        .pull_polyline(&path(vec![p(-1., 0., 2.), p(4., 0., 2.)]), tol())
        .unwrap()
        .unwrap();
    assert_eq!(output.vertices().first(), Some(&p(-1., 0., 0.)));
    assert_eq!(output.vertices().last(), Some(&p(1., 0., -1.)));
    assert!(output.vertices().contains(&p(0., 0., 0.)));
    assert!((output.length().unwrap() - (1. + 2_f64.sqrt())).abs() < 1e-10);
    check_on_mesh(&mesh, &output);
}
#[test]
fn disconnected_closest_branch_jump_declines() {
    let mesh = TriangleMesh::try_new(
        vec![
            p(-4., -2., 0.),
            p(0., -2., 0.),
            p(0., 2., 0.),
            p(2., -2., 0.),
            p(6., -2., 0.),
            p(2., 2., 0.),
        ],
        vec![[0, 1, 2], [3, 4, 5]],
        Tolerance::DEFAULT,
    )
    .unwrap();
    let result = MeshProjectionIndex::new(&mesh)
        .unwrap()
        .pull_polyline(&path(vec![p(-1., 0., 1.), p(3., 0., 1.)]), tol());
    assert!(matches!(result, Err(GeometryError::AmbiguousMeshPull)));
}
#[test]
fn connected_concave_fold_with_a_nearest_branch_jump_declines() {
    let mesh = TriangleMesh::try_new(
        vec![p(-4., 0., 0.), p(0., -4., 0.), p(0., 4., 0.), p(4., 0., 4.)],
        vec![[0, 1, 2], [1, 3, 2]],
        Tolerance::DEFAULT,
    )
    .unwrap();
    let result = MeshProjectionIndex::new(&mesh)
        .unwrap()
        .pull_polyline(&path(vec![p(-1., 0., 2.), p(2., 0., 2.)]), tol());
    assert!(matches!(result, Err(GeometryError::AmbiguousMeshPull)));
}
#[test]
fn closed_rational_curve_stays_closed_and_matches_planar_normal_image() {
    let mesh = square();
    let curve = crate::Circle3::try_new(
        p(0., 0., 2.),
        2.,
        Vector3::try_new(0., 0., 1.)
            .unwrap()
            .normalized_nonzero()
            .unwrap(),
        Tolerance::DEFAULT,
    )
    .unwrap()
    .to_nurbs()
    .unwrap();
    let tolerance = Tolerance::try_new(1e-3, 1e-12, 1e-10).unwrap();
    let output = MeshProjectionIndex::new(&mesh)
        .unwrap()
        .pull_curve(&curve, tolerance)
        .unwrap()
        .unwrap();
    assert_eq!(output.vertices().first(), output.vertices().last());
    assert!(output.vertices().len() > 32);
    for q in output.vertices() {
        assert_eq!(q.z(), 0.);
        assert!((q.x().hypot(q.y()) - 2.).abs() < 1e-10);
    }
    check_on_mesh(&mesh, &output);
}
#[test]
fn collapsed_image_is_none_and_source_reversals_remain() {
    let mesh = square();
    let index = MeshProjectionIndex::new(&mesh).unwrap();
    assert_eq!(
        index
            .pull_polyline(&path(vec![p(6., 6., 1.), p(7., 7., 2.)]), tol())
            .unwrap(),
        None
    );
    let output = index
        .pull_polyline(
            &path(vec![p(-2., 0., 2.), p(2., 0., 2.), p(-1., 0., 2.)]),
            tol(),
        )
        .unwrap()
        .unwrap();
    assert!((output.length().unwrap() - 7.).abs() < 1e-10);
}

#[test]
fn thin_nearer_sheet_between_sample_stations_is_not_skipped() {
    let mesh = TriangleMesh::try_new(
        vec![
            p(-4., -4., 0.),
            p(4., -4., 0.),
            p(0., 4., 0.),
            p(0.01, -1., 0.99),
            p(0.01000001, -1., 0.99),
            p(0.01, 1., 0.99),
        ],
        vec![[0, 1, 2], [3, 4, 5]],
        Tolerance::MESH_VALIDATION,
    )
    .unwrap();
    let index = MeshProjectionIndex::new(&mesh).unwrap();
    assert_eq!(
        index.closest_point(p(0.01, 0., 1.)).unwrap().unwrap().z(),
        0.99
    );
    assert!(matches!(
        index.pull_polyline(&path(vec![p(-1., 0., 1.), p(1., 0., 1.)]), tol()),
        Err(GeometryError::AmbiguousMeshPull)
    ));
}

#[test]
fn warped_quad_uses_the_same_short_diagonal_under_cyclic_rotation() {
    for face in [[0, 1, 2, 3], [1, 2, 3, 0]] {
        let mesh = TriangleMesh::try_new_faces(
            vec![
                p(-4., 0., 0.),
                p(0., -4., 0.),
                p(4., 0., -4.),
                p(0., 4., 0.),
            ],
            vec![MeshFace::Quad(face)],
            Tolerance::DEFAULT,
        )
        .unwrap();
        let output = MeshProjectionIndex::new(&mesh)
            .unwrap()
            .pull_polyline(&path(vec![p(-1., 0., 2.), p(4., 0., 2.)]), tol())
            .unwrap()
            .unwrap();
        assert!(output.vertices().contains(&p(0., 0., 0.)));
        assert!((output.length().unwrap() - (1. + 2_f64.sqrt())).abs() < 1e-10);
        check_on_mesh(&mesh, &output);
    }
}

#[test]
fn planar_paths_match_independent_nearest_queries_at_dense_stations() {
    let mesh = square();
    let index = MeshProjectionIndex::new(&mesh).unwrap();
    for i in 0..12 {
        let a = p(-7. + i as f64 * 0.25, -5. + i as f64 * 0.75, 3.);
        let b = p(7. - i as f64 * 0.4, 5. - i as f64 * 0.6, -2.);
        let output = index
            .pull_polyline(&path(vec![a, b]), tol())
            .unwrap()
            .unwrap();
        for j in 0..=32 {
            let t = j as f64 / 32.;
            let q = Point3::try_from(std::array::from_fn::<_, 3, _>(|k| {
                a.to_array()[k] * (1. - t) + b.to_array()[k] * t
            }))
            .unwrap();
            let expected = index.closest_point(q).unwrap().unwrap();
            assert!(
                output
                    .closest_point(expected, Tolerance::NUMERICAL_VALIDATION)
                    .unwrap()
                    .distance_to(expected)
                    .unwrap()
                    < 1e-6
            );
        }
    }
}

#[test]
fn unrepresentable_closest_edge_coordinates_decline_at_absolute_tolerance() {
    let base = 1e12;
    let mesh = TriangleMesh::try_new(
        vec![p(base, 0., 0.), p(base + 1., 2., 0.), p(base + 1., 0., 0.)],
        vec![[0, 1, 2]],
        Tolerance::DEFAULT,
    )
    .unwrap();
    let result = MeshProjectionIndex::new(&mesh).unwrap().pull_polyline(
        &path(vec![p(base - 0.25, 0.5, 1.), p(base - 0.25, 1., 1.)]),
        tol(),
    );
    assert_eq!(result, Err(GeometryError::UnrepresentableMeshProjection));
}
