use super::*;
fn p(x: f64, y: f64, z: f64) -> Point3 {
    Point3::try_new(x, y, z).unwrap()
}
fn tolerance() -> Tolerance {
    Tolerance::try_new(1e-8, 1e-12, 1e-10).unwrap()
}
#[test]
fn original_rational_basis_and_knots_survive_edit_point_reconstruction() {
    for gauge in [1., -1.] {
        let curve = NurbsCurve::try_new_rational(
            2,
            [p(0., 0., 2.), p(1., 3., 4.), p(3., -2., 3.), p(4., 0., 2.)]
                .into_iter()
                .zip([1., 0.5, 2., 1.])
                .map(|(p, w)| WeightedPoint3::try_new(p, w * gauge).unwrap())
                .collect(),
            vec![2., 2., 2., 3., 7., 7., 7.],
        )
        .unwrap();
        let points = curve
            .edit_points()
            .unwrap()
            .iter()
            .map(|e| p(e.point.x() + 10., e.point.y() * 2., 0.))
            .collect::<Vec<_>>();
        let output = curve.try_with_edit_points(&points, tolerance()).unwrap();
        assert_eq!(output.knots(), curve.knots());
        for (a, b) in output.control_points().iter().zip(curve.control_points()) {
            assert_eq!(a.weight(), b.weight());
        }
        for t in (0..=16).map(|i| 2. + 5. * i as f64 / 16.) {
            let a = curve.evaluate(t).unwrap();
            let b = output.evaluate(t).unwrap();
            assert!(b.distance_to(p(a.x() + 10., a.y() * 2., 0.)).unwrap() < 1e-8);
        }
    }
}
#[test]
fn sided_full_order_knots_do_not_merge_independent_edit_points() {
    let curve = NurbsCurve::try_new(
        1,
        vec![p(0., 0., 1.), p(1., 0., 1.), p(2., 1., 1.), p(3., 1., 1.)],
        vec![0., 0., 0.5, 0.5, 1., 1.],
    )
    .unwrap();
    let edits = curve.edit_points().unwrap();
    assert_eq!(edits[1].parameter, edits[2].parameter);
    assert_ne!(edits[1].side, edits[2].side);
    let target = edits
        .iter()
        .map(|e| p(e.point.x(), e.point.y(), 0.))
        .collect::<Vec<_>>();
    let output = curve.try_with_edit_points(&target, tolerance()).unwrap();
    assert_eq!(
        output
            .control_points()
            .iter()
            .map(|c| c.point())
            .collect::<Vec<_>>(),
        target
    );
}
#[test]
fn malformed_and_inaccurate_backends_decline_without_mutating_source() {
    let curve = NurbsCurve::try_new(
        2,
        vec![p(0., 0., 0.), p(1., 2., 0.), p(2., 0., 0.)],
        vec![0., 0., 0., 1., 1., 1.],
    )
    .unwrap();
    let original = curve.clone();
    let points = curve
        .edit_points()
        .unwrap()
        .iter()
        .map(|e| e.point)
        .collect::<Vec<_>>();
    for wrong in [vec![], vec![0.; 12], vec![f64::NAN; 12]] {
        assert!(
            curve
                .try_with_edit_points_with_solver(&points, tolerance(), &mut |_| Ok::<
                    _,
                    GeometryError,
                >(
                    wrong.clone()
                ))
                .is_err()
        );
    }
    assert_eq!(curve, original);
    assert!(
        curve
            .try_with_edit_points(&points[..2], tolerance())
            .is_err()
    );
}

#[test]
fn periodic_editing_keeps_the_original_seam_knots_and_repeated_tail() {
    for degree in [2, 3, 4] {
        let curve = NurbsCurve::try_control_point_curve_with_closure(
            degree,
            vec![
                p(2., 0., 2.),
                p(1., 2., 3.),
                p(-1., 2., 2.),
                p(-2., 0., 3.),
                p(-1., -2., 2.),
                p(1., -2., 3.),
            ],
            crate::ControlPointCurveClosure::Smooth,
        )
        .unwrap();
        assert!(curve.is_periodic());
        let edits = curve.edit_points().unwrap();
        assert_eq!(edits.len(), curve.control_points().len() - degree);
        let points = edits
            .iter()
            .map(|e| p(e.point.x() + 3., e.point.y(), 0.))
            .collect::<Vec<_>>();
        let output = curve.try_with_edit_points(&points, tolerance()).unwrap();
        assert!(output.is_periodic());
        assert_eq!(output.knots(), curve.knots());
        for i in 0..=32 {
            let t = curve.parameter_at(i as f64 / 32.).unwrap();
            let a = curve.evaluate(t).unwrap();
            assert!(
                output
                    .evaluate(t)
                    .unwrap()
                    .distance_to(p(a.x() + 3., a.y(), 0.))
                    .unwrap()
                    < 1e-8
            );
        }
    }
}

#[test]
fn exact_station_admission_rejects_error_hidden_by_large_coordinate_rounding() {
    let base = 2_f64.powi(52);
    let curve = NurbsCurve::try_new(
        2,
        vec![p(base, 0., 0.), p(base + 1., 0., 0.), p(base, 0., 0.)],
        vec![0., 0., 0., 1., 1., 1.],
    )
    .unwrap();
    assert_eq!(curve.evaluate(0.5).unwrap(), p(base, 0., 0.));
    let wrong = vec![0., 0., 0., 1., 1., 0., 0., 1., 0., 0., 0., 1.];
    assert_eq!(
        curve.try_with_edit_points_with_solver(
            &[p(base, 0., 0.); 3],
            Tolerance::try_new(0.1, 1e-12, 1e-10).unwrap(),
            &mut |_| Ok::<_, GeometryError>(wrong.clone())
        ),
        Err(GeometryError::CurveEditPointInterpolationFailed)
    );
}

#[test]
fn collapsed_native_greville_parameters_decline_before_solving() {
    let a = 1.;
    let b = 1_f64.next_up();
    let curve = NurbsCurve::try_new(
        2,
        vec![p(0., 0., 0.), p(1., 2., 0.), p(2., 0., 0.)],
        vec![a, a, a, b, b, b],
    )
    .unwrap();
    assert!(matches!(
        curve.edit_points(),
        Err(GeometryError::UnsupportedCurveEditPoints { .. })
    ));
}
