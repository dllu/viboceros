use super::*;
#[test]
fn apex_kink_partitions_retain_exact_poles_and_native_edge_charts() {
    let tolerance = Tolerance::DEFAULT;
    let point = |p| Point3::try_from(p).unwrap();
    for [start, end] in [[0., 4.], [0., 1.], [100., 104.], [1e6, 1e6 + 1.]] {
        let points = [
            [3., 0., 0.],
            [4., 0., 1.],
            [3., 0., 2.],
            [2., 0., 1.],
            [3., 0., 0.],
        ];
        let mut knots = vec![start];
        knots.extend((0..=4).map(|i| start + (end - start) * i as f64 / 4.));
        knots.push(end);
        let curve = NurbsCurve::try_new(1, points.into_iter().map(point).collect(), knots).unwrap();
        let apex = point([1., -4., 3.]);
        let surface = NurbsSurface::try_extruded_curve_to_point(&curve, apex).unwrap();
        let source =
            Brep::try_surface_face_with_native_edge_parameters(surface, tolerance).unwrap();
        let result = source
            .try_split_kinky_faces(0.1_f64.to_radians(), tolerance)
            .unwrap()
            .unwrap();
        assert_eq!(result.faces().len(), 4);
        assert_eq!(
            result
                .vertices()
                .iter()
                .filter(|v| v.point() == apex)
                .count(),
            1
        );
        assert!(!result.is_solid());
        assert!(result.is_manifold());
    }
}

#[test]
fn negative_uv_charts_keep_exact_linear_crossings_without_rescale_roundoff() {
    for interval in [[-4., 0.], [0., 4.], [-4., 4.]] {
        let curve = NurbsCurve2::try_new(
            1,
            vec![
                Point2::try_new(interval[1], 0.).unwrap(),
                Point2::try_new(interval[0], 0.).unwrap(),
            ],
            vec![interval[0], interval[0], interval[1], interval[1]],
        )
        .unwrap();
        for cut in [-3., -2., -1., 0., 1., 2., 3.]
            .into_iter()
            .filter(|t| interval[0] < *t && *t < interval[1])
        {
            let roots = curves::crossing_parameters(&curve, 0, cut, &mut Budget(MAX_WORK)).unwrap();
            assert_eq!(roots.len(), 1);
            assert_eq!(curve.evaluate(roots[0]).unwrap().x(), cut);
        }
    }
}

#[test]
fn apex_g2_and_g1_partitions_are_independent_and_leave_source_untouched() {
    let p = |x, z| Point3::try_new(x, 0., z).unwrap();
    let curve = NurbsCurve::try_new(
        2,
        vec![p(3., 0.), p(3.5, 1.), p(4.25, 2.), p(5., 3.), p(6.5, 5.)],
        vec![0., 0., 0., 0.5, 0.5, 1., 1., 1.],
    )
    .unwrap();
    let surface =
        NurbsSurface::try_extruded_curve_to_point(&curve, Point3::try_new(0., 3., 2.).unwrap())
            .unwrap();
    let source =
        Brep::try_surface_face_with_native_edge_parameters(surface.clone(), Tolerance::DEFAULT)
            .unwrap();
    let before = source.clone();
    assert!(
        source
            .try_split_kinky_faces(0.1_f64.to_radians(), Tolerance::DEFAULT)
            .unwrap()
            .is_none()
    );
    let result = source
        .try_split_faces_at_discontinuities(
            crate::CurveContinuity::G2,
            1_f64.to_radians(),
            Tolerance::DEFAULT,
        )
        .unwrap()
        .unwrap();
    assert_eq!(result.faces.len(), 2);
    assert!(result.is_manifold());
    assert_eq!(
        result
            .vertices
            .iter()
            .filter(|v| v.point() == Point3::try_new(0., 3., 2.).unwrap())
            .count(),
        1
    );
    assert_eq!(source, before);
    assert!(
        source
            .try_split_faces_at_discontinuities(
                crate::CurveContinuity::G2,
                f64::NAN,
                Tolerance::DEFAULT
            )
            .is_err()
    );
    assert_eq!(source, before);
}
