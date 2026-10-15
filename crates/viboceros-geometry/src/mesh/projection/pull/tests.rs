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

#[test]
fn weighted_degree_one_sources_keep_chords_reversals_and_constant_spans() {
    let mesh = square();
    let index = MeshProjectionIndex::new(&mesh).unwrap();
    for weights in [[1., 1e6, 0.5, 2.], [-1., -1e6, -0.5, -2.]] {
        let curve = NurbsCurve::try_new_rational(
            1,
            [p(-2., 0., 2.), p(2., 0., 2.), p(2., 0., 2.), p(-1., 0., 2.)]
                .into_iter()
                .zip(weights)
                .map(|(p, w)| crate::WeightedPoint3::try_new(p, w).unwrap())
                .collect(),
            vec![0., 0., 0.2, 0.7, 1., 1.],
        )
        .unwrap();
        let output = index
            .pull_curve(&curve, Tolerance::DEFAULT)
            .unwrap()
            .unwrap();
        assert!((output.length().unwrap() - 7.).abs() < 1e-10);
        assert_eq!(output.vertices().first(), Some(&p(-2., 0., 0.)));
        assert_eq!(output.vertices().last(), Some(&p(-1., 0., 0.)));
        assert!(output.vertices().len() < 16);
    }
}

#[test]
fn degree_one_fast_path_declines_poles_and_discontinuities() {
    let mesh = square();
    let index = MeshProjectionIndex::new(&mesh).unwrap();
    let pole = NurbsCurve::try_new_rational(
        1,
        vec![
            crate::WeightedPoint3::try_new(p(-1., 0., 2.), 1.).unwrap(),
            crate::WeightedPoint3::try_new(p(1., 0., 2.), -1.).unwrap(),
        ],
        vec![0., 0., 1., 1.],
    )
    .unwrap();
    let discontinuous = NurbsCurve::try_new(
        1,
        vec![p(-2., 0., 2.), p(-1., 0., 2.), p(1., 0., 2.), p(2., 0., 2.)],
        vec![0., 0., 0.5, 0.5, 1., 1.],
    )
    .unwrap();
    for c in [pole, discontinuous] {
        assert!(matches!(
            index.pull_curve(&c, tol()),
            Err(GeometryError::UnsupportedCurveApproximation { .. })
        ));
    }
    let constant = NurbsCurve::try_new(1, vec![p(1., 0., 2.); 2], vec![-1., 0., 1., 2.]).unwrap();
    assert_eq!(index.pull_curve(&constant, tol()).unwrap(), None);
}

fn grid(n: u32, origin: f64) -> TriangleMesh {
    let vertices = (0..=n)
        .flat_map(|y| {
            (0..=n).map(move |x| {
                p(
                    origin - 4. + 8. * x as f64 / n as f64,
                    -4. + 8. * y as f64 / n as f64,
                    0.,
                )
            })
        })
        .collect();
    let mut faces = Vec::new();
    for y in 0..n {
        for x in 0..n {
            let a = y * (n + 1) + x;
            faces.extend([[a, a + 1, a + n + 2], [a, a + n + 2, a + n + 1]]);
        }
    }
    TriangleMesh::try_new(vertices, faces, Tolerance::DEFAULT).unwrap()
}

#[test]
fn partitioned_long_chord_crosses_a_dense_mesh_within_the_original_budget() {
    for origin in [0., 1e12] {
        let mesh = grid(24, origin);
        let tolerance = if origin == 0. {
            tol()
        } else {
            Tolerance::try_new(1e-2, 1e-12, 1e-10).unwrap()
        };
        let output = MeshProjectionIndex::new(&mesh)
            .unwrap()
            .pull_polyline(
                &path(vec![p(origin - 3.5, 0.25, 2.), p(origin + 3.5, 0.25, 2.)]),
                tolerance,
            )
            .unwrap()
            .unwrap();
        assert_eq!(output.vertices().first(), Some(&p(origin - 3.5, 0.25, 0.)));
        assert_eq!(output.vertices().last(), Some(&p(origin + 3.5, 0.25, 0.)));
        assert!((output.length().unwrap() - 7.).abs() < 1e-10);
    }
}

#[test]
fn partitioning_keeps_a_thin_nearer_sheet_and_declines_the_branch_jump() {
    let base = grid(24, 0.);
    let mut vertices = base.vertices().to_vec();
    let mut faces = base.triangles().to_vec();
    let start = vertices.len() as u32;
    vertices.extend([
        p(0.01, -1., 1.99),
        p(0.01000001, -1., 1.99),
        p(0.01, 1., 1.99),
    ]);
    faces.push([start, start + 1, start + 2]);
    let mesh = TriangleMesh::try_new(vertices, faces, Tolerance::MESH_VALIDATION).unwrap();
    assert!(matches!(
        MeshProjectionIndex::new(&mesh)
            .unwrap()
            .pull_polyline(&path(vec![p(-3.5, 0.25, 2.), p(3.5, 0.25, 2.)]), tol()),
        Err(GeometryError::AmbiguousMeshPull)
    ));
}
