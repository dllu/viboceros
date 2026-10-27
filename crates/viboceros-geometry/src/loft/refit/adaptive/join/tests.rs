use super::*;

#[test]
fn unequal_handles_are_retimed_before_hermite_reconstruction() {
    let points = [0., 0.25, 0.75, 1., 1.5, 2.5, 3.]
        .map(|x| Point3::try_new(x, 0., 0.).unwrap())
        .to_vec();
    let raw = NurbsCurve::try_new(
        3,
        points.clone(),
        [vec![0.; 4], vec![0.5; 3], vec![1.; 4]].concat(),
    )
    .unwrap();
    let retimed = NurbsCurve::try_new(
        3,
        points,
        [vec![0.; 4], vec![1. / 3.; 3], vec![1.; 4]].concat(),
    )
    .unwrap();
    let before = raw.clone();
    let fitted = c1_region(&raw, &[0., 0.5, 1.], 0, 2).unwrap();
    assert_eq!(fitted.control_points().len(), 6);
    assert_eq!(fitted.knots(), &[0., 0., 0., 0., 0.5, 0.5, 1., 1., 1., 1.]);
    for t in [0., 0.5, 1.] {
        let (point, derivative) = fitted.evaluate_with_derivative(t).unwrap();
        let (expected, tangent) = retimed.evaluate_with_derivative(t).unwrap();
        assert!(point.distance_to(expected).unwrap() < 2e-15);
        for (a, b) in derivative.to_array().into_iter().zip(tangent.to_array()) {
            assert!((a - b).abs() < 4e-15);
        }
    }
    assert_eq!(raw, before);
}

#[test]
fn zero_speed_at_a_composite_join_returns_no_fit() {
    let points = [0., 0.25, 1., 1., 1.5, 2.5, 3.]
        .map(|x| Point3::try_new(x, 0., 0.).unwrap())
        .to_vec();
    let raw =
        NurbsCurve::try_new(3, points, [vec![0.; 4], vec![0.5; 3], vec![1.; 4]].concat()).unwrap();
    assert!(matches!(
        c1_region(&raw, &[0., 0.5, 1.], 0, 2),
        Err(GeometryError::UnsupportedCurveApproximation { .. })
    ));
}

#[test]
fn multiple_regions_keep_retimed_handles_instead_of_rebuilding_at_old_cuts() {
    let points = [0., 0.25, 0.75, 1., 1.5, 2.5, 3.]
        .map(|x| Point3::try_new(x, 0., 0.).unwrap())
        .to_vec();
    let raw = NurbsCurve::try_new(
        3,
        points.clone(),
        [vec![0.; 4], vec![0.5; 3], vec![1.; 4]].concat(),
    )
    .unwrap();
    let retimed = NurbsCurve::try_new(
        3,
        points.clone(),
        [vec![0.; 4], vec![1. / 3.; 3], vec![1.; 4]].concat(),
    )
    .unwrap();
    let reduced = c1_region_impl(&raw, &[0., 0.5, 1.], 0, 2, false).unwrap();
    assert_eq!(
        reduced
            .control_points()
            .iter()
            .map(|p| p.point())
            .collect::<Vec<_>>(),
        points
            .into_iter()
            .enumerate()
            .filter_map(|(i, p)| (i != 3).then_some(p))
            .collect::<Vec<_>>()
    );
    assert_eq!(
        reduced.knots(),
        [0., 0., 0., 0., 1. / 3., 1. / 3., 1., 1., 1., 1.]
    );
    for i in 0..=32 {
        let t = f64::from(i) / 32.;
        assert!(
            reduced
                .evaluate(t)
                .unwrap()
                .distance_to(retimed.evaluate(t).unwrap())
                .unwrap()
                < 2e-15
        );
    }
}
