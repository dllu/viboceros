use super::*;

#[test]
fn closed_state_matches_opennurbs_endpoint_and_size_rules() {
    let closed = NurbsCurve::try_new(
        2,
        vec![
            point(0.0, 0.0),
            point(3.0, 0.0),
            point(3.0, 2.0),
            point(0.0, 0.0),
        ],
        vec![0.0, 0.0, 0.0, 1.0, 2.0, 2.0, 2.0],
    )
    .unwrap();
    assert!(closed.is_closed().unwrap());
    assert_eq!(
        closed.extract_point_locations().unwrap(),
        vec![point(0.0, 0.0), point(3.0, 0.0), point(3.0, 2.0)]
    );

    let nearly_closed = NurbsCurve::try_new(
        2,
        vec![
            point(0.0, 0.0),
            point(3.0, 0.0),
            point(3.0, 2.0),
            point(1.0e-10, 0.0),
        ],
        vec![0.0, 0.0, 0.0, 1.0, 2.0, 2.0, 2.0],
    )
    .unwrap();
    assert!(nearly_closed.is_closed().unwrap());
    assert_eq!(
        nearly_closed.extract_point_locations().unwrap(),
        vec![point(0.0, 0.0), point(3.0, 0.0), point(3.0, 2.0),]
    );

    let open = NurbsCurve::try_new(
        2,
        vec![
            point(0.0, 0.0),
            point(3.0, 0.0),
            point(3.0, 2.0),
            point(1.0e-6, 0.0),
        ],
        vec![0.0, 0.0, 0.0, 1.0, 2.0, 2.0, 2.0],
    )
    .unwrap();
    assert!(!open.is_closed().unwrap());

    let two_segment_loop = NurbsCurve::try_new(
        1,
        vec![point(0.0, 0.0), point(3.0, 0.0), point(0.0, 0.0)],
        vec![0.0, 0.0, 1.0, 2.0, 2.0],
    )
    .unwrap();
    assert!(!two_segment_loop.is_closed().unwrap());

    let periodic = NurbsCurve::try_new(
        2,
        vec![
            point(0.0, 0.0),
            point(2.0, 0.0),
            point(1.0, 2.0),
            point(0.0, 0.0),
            point(2.0, 0.0),
        ],
        vec![-2.0, -1.0, 0.0, 1.0, 2.0, 3.0, 4.0, 5.0],
    )
    .unwrap();
    assert!(periodic.is_periodic());
    assert!(periodic.is_closed().unwrap());
    assert_eq!(
        periodic.extract_point_locations().unwrap(),
        vec![point(0.0, 0.0), point(2.0, 0.0), point(1.0, 2.0)]
    );
}

#[test]
fn reversal_negates_the_full_domain_and_parameter_direction() {
    let curve = NurbsCurve::try_new(
        2,
        vec![
            point(0.0, 0.0),
            point(1.0, 3.0),
            point(4.0, -1.0),
            point(7.0, 2.0),
        ],
        vec![-2.0, -2.0, -2.0, 1.0, 5.0, 5.0, 5.0],
    )
    .unwrap();
    let reversed = curve.reversed().unwrap();
    assert_eq!(reversed.domain(), -5.0..=2.0);
    assert_eq!(reversed.knots(), &[-5.0, -5.0, -5.0, -1.0, 2.0, 2.0, 2.0]);
    for sample in 0..=16 {
        let normalized = sample as Real / 16.0;
        let reversed_parameter = reversed.parameter_at(normalized).unwrap();
        let original_parameter = curve.parameter_at(1.0 - normalized).unwrap();
        assert_point_near(
            reversed.evaluate(reversed_parameter).unwrap(),
            curve.evaluate(original_parameter).unwrap(),
        );
        let actual = reversed.derivative_at(reversed_parameter).unwrap();
        let expected = curve.derivative_at(original_parameter).unwrap();
        assert!(Tolerance::DEFAULT.approx_eq(actual.x(), -expected.x()));
        assert!(Tolerance::DEFAULT.approx_eq(actual.y(), -expected.y()));
        assert!(Tolerance::DEFAULT.approx_eq(actual.z(), -expected.z()));
    }
    assert_eq!(reversed.reversed().unwrap(), curve);
}

#[test]
fn closed_curve_seam_relocation_rotates_periodic_structure_exactly() {
    let unique = [
        point(0.0, 0.0),
        point(2.0, 0.0),
        point(2.0, 2.0),
        point(0.0, 2.0),
    ];
    let curve = NurbsCurve::try_new(
        3,
        unique.iter().chain(&unique[..3]).copied().collect(),
        vec![0.0, 0.0, 1.0, 2.0, 3.0, 4.0, 5.0, 6.0, 7.0, 8.0, 8.0],
    )
    .unwrap();
    assert!(curve.is_periodic());

    let relocated = curve.try_change_closed_seam(3.25).unwrap();
    assert!(relocated.is_periodic());
    assert_eq!(relocated.domain(), 3.25..=7.25);
    assert_eq!(relocated.control_points().len(), 8);
    assert_eq!(
        relocated.knots(),
        &[2.0, 2.0, 3.0, 3.25, 4.0, 5.0, 6.0, 7.0, 7.25, 8.0, 9.0, 9.0]
    );
    let expected_controls = [
        point(2.0, 1.5),
        point(7.0 / 6.0, 2.0),
        point(0.0, 11.0 / 6.0),
        unique[0],
        unique[1],
        point(2.0, 1.5),
        point(7.0 / 6.0, 2.0),
        point(0.0, 11.0 / 6.0),
    ];
    for (actual, expected) in relocated.control_points().iter().zip(expected_controls) {
        assert_point_near(actual.point(), expected);
    }
    for sample in 0..=40 {
        let parameter = 3.25 + sample as Real / 40.0 * 4.0;
        let source_parameter = if parameter <= 6.0 {
            parameter
        } else {
            parameter - 4.0
        };
        assert_point_near(
            relocated.evaluate(parameter).unwrap(),
            curve.evaluate(source_parameter).unwrap(),
        );
    }

    assert_eq!(curve.try_change_closed_seam(2.0).unwrap(), curve);
    let shifted = curve.try_change_closed_seam(6.0).unwrap();
    assert_eq!(shifted.domain(), 6.0..=10.0);
    assert_eq!(
        shifted.knots(),
        &[4.0, 4.0, 5.0, 6.0, 7.0, 8.0, 9.0, 10.0, 11.0, 12.0, 12.0]
    );
    assert_eq!(shifted.control_points(), curve.control_points());

    let reparameterized = curve.try_reparameterized(100.0..=180.0).unwrap();
    let relocated = reparameterized.try_change_closed_seam(133.0).unwrap();
    assert!(relocated.is_periodic());
    assert_eq!(relocated.domain(), 133.0..=213.0);
    assert_eq!(relocated.control_points().len(), 8);
}

#[test]
fn closed_curve_seam_relocation_preserves_rational_segments_and_rhino_knot_rules() {
    let curve = NurbsCurve::try_new_rational(
        2,
        [
            ([0.0, 0.0, 0.0], 0.5),
            ([3.0, -1.0, 0.0], 1.3),
            ([5.0, 3.0, 0.0], 0.8),
            ([0.0, 4.0, 0.0], 1.7),
            ([0.0, 0.0, 0.0], 2.0),
        ]
        .into_iter()
        .map(|(coordinates, weight)| {
            WeightedPoint3::try_new(Point3::try_from(coordinates).unwrap(), weight).unwrap()
        })
        .collect(),
        vec![4.0, 4.0, 4.0, 5.0, 9.0, 12.0, 12.0, 12.0],
    )
    .unwrap();
    assert!(curve.is_closed().unwrap());
    let relocated = curve.try_change_closed_seam(7.0).unwrap();
    assert!(!relocated.is_periodic());
    assert_eq!(relocated.domain(), 7.0..=15.0);
    assert_eq!(
        relocated.knots(),
        &[7.0, 7.0, 7.0, 9.0, 12.0, 12.0, 13.0, 15.0, 15.0, 15.0]
    );
    let expected_weights = [
        1.028_571_428_571_428_5,
        1.057_142_857_142_857_2,
        1.7,
        2.0,
        5.2,
        4.0,
        4.114_285_714_285_714,
    ];
    for (control, expected) in relocated.control_points().iter().zip(expected_weights) {
        assert!(Tolerance::DEFAULT.approx_eq(control.weight(), expected));
    }
    for sample in 0..=40 {
        let parameter = 7.0 + sample as Real / 40.0 * 8.0;
        let source_parameter = if parameter <= 12.0 {
            parameter
        } else {
            parameter - 8.0
        };
        assert_point_near(
            relocated.evaluate(parameter).unwrap(),
            curve.evaluate(source_parameter).unwrap(),
        );
    }

    let periodic = NurbsCurve::try_new_rational(
        3,
        [
            ([0.0, 0.0, 0.0], 0.5),
            ([3.0, -1.0, 0.0], 1.2),
            ([5.0, 3.0, 1.0], 0.8),
            ([1.0, 5.0, -1.0], 1.5),
            ([-2.0, 2.0, 0.0], 0.7),
            ([0.0, 0.0, 0.0], 0.5),
            ([3.0, -1.0, 0.0], 1.2),
            ([5.0, 3.0, 1.0], 0.8),
        ]
        .into_iter()
        .map(|(coordinates, weight)| {
            WeightedPoint3::try_new(Point3::try_from(coordinates).unwrap(), weight).unwrap()
        })
        .collect(),
        vec![
            -2.0, -2.0, -1.0, 0.0, 1.0, 1.0, 2.0, 3.0, 4.0, 5.0, 5.0, 5.0,
        ],
    )
    .unwrap();
    assert!(periodic.is_periodic());
    let clamped = periodic.try_change_closed_seam(1.0).unwrap();
    assert!(!clamped.is_periodic());
    assert_eq!(clamped.domain(), 1.0..=5.0);
    assert_eq!(
        clamped.knots(),
        &[
            1.0, 1.0, 1.0, 1.0, 2.0, 3.0, 4.0, 4.0, 4.0, 5.0, 5.0, 5.0, 5.0
        ]
    );
    assert_eq!(clamped.control_points().len(), 9);
}

#[test]
fn closed_curve_seam_relocation_rejects_open_curves_and_wraps_finite_parameters() {
    let open = NurbsCurve::try_new(
        2,
        vec![point(0.0, 0.0), point(2.0, 3.0), point(5.0, 0.0)],
        vec![0.0, 0.0, 0.0, 1.0, 1.0, 1.0],
    )
    .unwrap();
    assert_eq!(
        open.try_change_closed_seam(0.5),
        Err(GeometryError::CurveSeamMustBeClosed)
    );
    let closed = NurbsCurve::try_control_point_curve_with_closure(
        1,
        vec![
            point(0.0, 0.0),
            point(4.0, 0.0),
            point(4.0, 3.0),
            point(0.0, 3.0),
        ],
        ControlPointCurveClosure::Sharp,
    )
    .unwrap();
    assert!(matches!(
        closed.try_change_closed_seam(f64::NAN),
        Err(GeometryError::NonFinite { .. })
    ));
    let t = *closed.domain().end() + 1.0;
    let shifted = closed.try_change_closed_seam(t).unwrap();
    assert_eq!(*shifted.domain().start(), t);
    assert!(
        shifted
            .evaluate(t)
            .unwrap()
            .distance_to(closed.evaluate(*closed.domain().start() + 1.0).unwrap())
            .unwrap()
            < 1e-12
    );
}

#[test]
fn knot_insertion_restores_periodic_controls_and_knot_intervals() {
    let source = NurbsCurve::try_new(
        3,
        vec![
            point(0.0, 0.0),
            point(2.0, 0.0),
            point(2.0, 2.0),
            point(0.0, 2.0),
            point(0.0, 0.0),
            point(2.0, 0.0),
            point(2.0, 2.0),
        ],
        vec![0.0, 0.0, 1.0, 2.0, 3.0, 4.0, 5.0, 6.0, 7.0, 8.0, 8.0],
    )
    .unwrap();
    assert!(source.is_periodic());

    let cases = [
        (
            4.5,
            vec![0.5, 0.5, 1.0, 2.0, 3.0, 4.0, 4.5, 5.0, 6.0, 7.0, 8.0, 8.0],
        ),
        (
            2.25,
            vec![0.0, 0.0, 1.0, 2.0, 2.25, 3.0, 4.0, 5.0, 6.0, 6.25, 7.0, 7.0],
        ),
    ];
    for (parameter, expected_knots) in cases {
        let refined = source.try_insert_knot(parameter, 1).unwrap();
        assert_eq!(refined.knots(), expected_knots);
        assert!(refined.is_periodic());
        assert!(refined.is_closed().unwrap());
        let repeat_start = refined.control_points().len() - refined.degree();
        assert_eq!(
            &refined.control_points()[..refined.degree()],
            &refined.control_points()[repeat_start..]
        );
        for sample in 0..=64 {
            let normalized = sample as Real / 64.0;
            let source_parameter = source.parameter_at(normalized).unwrap();
            let refined_parameter = refined.parameter_at(normalized).unwrap();
            assert_point_near(
                refined.evaluate(refined_parameter).unwrap(),
                source.evaluate(source_parameter).unwrap(),
            );
        }
    }
}

#[test]
fn make_non_periodic_clamps_a_periodic_curve_without_changing_its_locus() {
    let source = NurbsCurve::try_new(
        3,
        vec![
            point(0.0, 0.0),
            point(2.0, 0.0),
            point(2.0, 2.0),
            point(0.0, 2.0),
            point(0.0, 0.0),
            point(2.0, 0.0),
            point(2.0, 2.0),
        ],
        vec![0.0, 0.0, 1.0, 2.0, 3.0, 4.0, 5.0, 6.0, 7.0, 8.0, 8.0],
    )
    .unwrap();
    let domain = source.domain();
    let clamped = source.try_make_non_periodic().unwrap();
    assert!(!clamped.is_periodic());
    assert!(clamped.is_closed().unwrap());
    assert_eq!(clamped.domain(), domain);
    assert!(
        clamped.knots()[..=clamped.degree()]
            .iter()
            .all(|knot| *knot == *domain.start())
    );
    assert!(
        clamped.knots()[clamped.knots().len() - clamped.degree() - 1..]
            .iter()
            .all(|knot| *knot == *domain.end())
    );
    for sample in 0..=64 {
        let parameter = source.parameter_at(sample as Real / 64.0).unwrap();
        assert_point_near(
            clamped.evaluate(parameter).unwrap(),
            source.evaluate(parameter).unwrap(),
        );
    }
    assert_eq!(clamped.try_make_non_periodic().unwrap(), clamped);
}

#[test]
fn make_periodic_without_smoothing_matches_rhino_control_rotation_and_knots() {
    let source = NurbsCurve::try_new(
        3,
        vec![
            point(1.0, 0.0),
            point(4.0, -1.0),
            Point3::try_new(6.0, 2.0, 1.0).unwrap(),
            point(4.0, 5.0),
            Point3::try_new(0.0, 4.0, -1.0).unwrap(),
            point(-2.0, 1.0),
            point(1.0, 0.0),
        ],
        vec![
            10.0, 10.0, 10.0, 10.0, 11.0, 13.0, 19.0, 25.0, 25.0, 25.0, 25.0,
        ],
    )
    .unwrap();

    let periodic = source.try_make_periodic(false).unwrap();

    assert!(periodic.is_periodic());
    assert!(periodic.is_closed().unwrap());
    assert_eq!(periodic.domain(), 10.0..=25.0);
    let expected_points = [
        [-2.0, 1.0, 0.0],
        [1.0, 0.0, 0.0],
        [4.0, -1.0, 0.0],
        [6.0, 2.0, 1.0],
        [4.0, 5.0, 0.0],
        [0.0, 4.0, -1.0],
        [-2.0, 1.0, 0.0],
        [1.0, 0.0, 0.0],
        [4.0, -1.0, 0.0],
    ];
    assert_eq!(periodic.control_points().len(), expected_points.len());
    for (control, expected) in periodic.control_points().iter().zip(expected_points) {
        assert_point_near(control.point(), Point3::try_from(expected).unwrap());
        assert_eq!(control.weight(), 1.0);
    }
    let expected_knots = [
        5.227272727272728,
        5.227272727272728,
        7.613636363636364,
        10.0,
        12.386363636363637,
        14.772727272727273,
        16.136363636363637,
        20.227272727272727,
        22.613636363636363,
        25.0,
        27.386363636363637,
        29.772727272727273,
        29.772727272727273,
    ];
    for (actual, expected) in periodic.knots().iter().zip(expected_knots) {
        assert!((actual - expected).abs() <= 4.0e-15);
    }
}

#[test]
fn make_periodic_without_smoothing_handles_single_span_odd_degrees() {
    let cubic = NurbsCurve::try_new(
        3,
        vec![
            point(0.0, 0.0),
            point(5.0, -2.0),
            Point3::try_new(4.0, 6.0, 1.0).unwrap(),
            point(0.0, 0.0),
        ],
        vec![3.0, 3.0, 3.0, 3.0, 11.0, 11.0, 11.0, 11.0],
    )
    .unwrap();

    let periodic = cubic.try_make_periodic(false).unwrap();

    assert!(periodic.is_periodic());
    assert_eq!(periodic.control_points().len(), 6);
    let expected_knots = [
        -2.333333333333333,
        -2.333333333333333,
        0.3333333333333335,
        3.0,
        5.666666666666666,
        8.333333333333334,
        11.0,
        13.666666666666666,
        16.333333333333332,
        16.333333333333332,
    ];
    for (actual, expected) in periodic.knots().iter().zip(expected_knots) {
        assert!((actual - expected).abs() <= 4.0e-15);
    }
}

#[test]
fn make_periodic_with_smoothing_matches_rhino_homogeneous_interpolation() {
    let source = NurbsCurve::try_new_rational(
        2,
        [
            ([-1.0, 0.0, 0.0], 0.75),
            ([2.0, -2.0, 0.0], 1.5),
            ([4.0, 2.0, 1.0], 0.6),
            ([0.0, 4.0, 0.0], 1.8),
            ([-1.0, 0.0, 0.0], 0.75),
        ]
        .into_iter()
        .map(|(point, weight)| {
            WeightedPoint3::try_new(Point3::try_from(point).unwrap(), weight).unwrap()
        })
        .collect(),
        vec![0.0, 0.0, 0.0, 2.0, 5.0, 8.0, 8.0, 8.0],
    )
    .unwrap();

    let periodic = source.try_make_periodic(true).unwrap();

    assert!(periodic.is_periodic());
    assert!(periodic.is_closed().unwrap());
    assert_eq!(periodic.domain(), source.domain());
    assert_eq!(
        periodic.knots(),
        &[-3.0, -3.0, 0.0, 2.0, 5.0, 8.0, 10.0, 10.0]
    );
    let expected = [
        (
            [-0.5057939242092077, 4.476041340432196, 0.0],
            1.5350961538461538,
        ),
        (
            [1.8129330254041567, -2.651270207852194, 0.0],
            1.2490384615384615,
        ),
        (
            [3.8504479669193663, 1.893866299104066, 0.8600964851826325],
            0.697596153846154,
        ),
        (
            [-0.5057939242092077, 4.476041340432196, 0.0],
            1.5350961538461538,
        ),
        (
            [1.8129330254041567, -2.651270207852194, 0.0],
            1.2490384615384615,
        ),
    ];
    for (control, (expected_point, expected_weight)) in
        periodic.control_points().iter().zip(expected)
    {
        assert_point_near(control.point(), Point3::try_from(expected_point).unwrap());
        assert!((control.weight() - expected_weight).abs() <= 2.0e-15);
    }
}

#[test]
fn smooth_periodic_greville_phase_starts_with_the_first_in_domain_value() {
    let source_knots = [0.0, 0.0, 0.0, 0.0, 0.1, 9.0, 9.5, 10.0, 10.0, 10.0, 10.0];
    let knots = periodic_knots_preserving_active(3, 7, &source_knots).unwrap();
    let parameters = periodic_greville_parameters(3, 7, 4, &knots).unwrap().0;
    let expected = [3.033333333333333, 6.2, 9.5, 9.866666666666667];
    for (actual, expected) in parameters.into_iter().zip(expected) {
        assert!((actual - expected).abs() <= 2.0e-15);
    }

    let source_knots = [0.0, 0.0, 0.0, 9.8, 9.9, 10.0, 10.0, 10.0];
    let knots = periodic_knots_preserving_active(2, 5, &source_knots).unwrap();
    let parameters = periodic_greville_parameters(2, 5, 3, &knots).unwrap().0;
    assert!((parameters[0] - 4.9).abs() <= 4.0e-15);
    assert!((parameters[1] - 9.85).abs() <= 4.0e-15);
    assert!((parameters[2] - 9.95).abs() <= 4.0e-15);
}

#[test]
fn make_periodic_validates_degree_closure_and_smooth_control_count() {
    let linear_loop = NurbsCurve::try_new(
        1,
        vec![point(0.0, 0.0), point(2.0, 0.0), point(0.0, 0.0)],
        vec![0.0, 0.0, 1.0, 2.0, 2.0],
    )
    .unwrap();
    assert_eq!(
        linear_loop.try_make_periodic(false),
        Err(GeometryError::PeriodicNurbsDegreeTooLow)
    );

    let open =
        NurbsCurve::try_clamped_uniform(2, vec![point(0.0, 0.0), point(1.0, 2.0), point(3.0, 0.0)])
            .unwrap();
    assert_eq!(
        open.try_make_periodic(false),
        Err(GeometryError::PeriodicCurveMustBeClosed)
    );

    let short_quadratic = NurbsCurve::try_new(
        2,
        vec![
            point(0.0, 0.0),
            point(5.0, -2.0),
            Point3::try_new(4.0, 6.0, 1.0).unwrap(),
            point(0.0, 0.0),
        ],
        vec![0.0, 0.0, 0.0, 9.9, 10.0, 10.0, 10.0],
    )
    .unwrap();
    assert_eq!(
        short_quadratic.try_make_periodic(true),
        Err(GeometryError::InsufficientSmoothPeriodicControlPoints {
            degree: 2,
            required: 5,
            actual: 4,
        })
    );

    let short_cubic = NurbsCurve::try_new(
        3,
        vec![
            point(0.0, 0.0),
            point(4.0, 0.0),
            point(5.0, 3.0),
            point(0.0, 4.0),
            point(0.0, 0.0),
        ],
        vec![0.0, 0.0, 0.0, 0.0, 1.0, 2.0, 2.0, 2.0, 2.0],
    )
    .unwrap();
    assert_eq!(
        short_cubic.try_make_periodic(true),
        Err(GeometryError::InsufficientSmoothPeriodicControlPoints {
            degree: 3,
            required: 6,
            actual: 5,
        })
    );
    assert!(short_cubic.try_make_periodic(false).unwrap().is_periodic());
}

#[test]
fn make_periodic_is_a_no_op_for_periodic_curves() {
    let periodic = NurbsCurve::try_new(
        2,
        vec![
            point(0.0, 0.0),
            point(2.0, 0.0),
            point(1.0, 2.0),
            point(0.0, 0.0),
            point(2.0, 0.0),
        ],
        vec![-2.0, -1.0, 0.0, 1.0, 2.0, 3.0, 4.0, 5.0],
    )
    .unwrap();
    assert_eq!(periodic.try_make_periodic(false).unwrap(), periodic);
    assert_eq!(periodic.try_make_periodic(true).unwrap(), periodic);
}

#[test]
fn directed_subcurves_reverse_open_curves_and_cross_closed_seams() {
    let open = NurbsCurve::try_new_rational(
        2,
        vec![
            WeightedPoint3::try_new(point(-2.0, 1.0), 0.75).unwrap(),
            WeightedPoint3::try_new(point(0.0, 4.0), 2.0).unwrap(),
            WeightedPoint3::try_new(point(5.0, -2.0), 0.5).unwrap(),
            WeightedPoint3::try_new(point(8.0, 3.0), 1.25).unwrap(),
        ],
        vec![-2.0, -1.0, 0.0, 0.8, 2.0, 3.0, 4.0],
    )
    .unwrap();
    assert_eq!(
        open.try_subcurve(0.25, 1.6).unwrap(),
        open.try_trimmed(0.25..=1.6).unwrap()
    );
    let reversed = open.try_subcurve(1.6, 0.25).unwrap();
    assert_eq!(reversed.domain(), -1.6..=-0.25);
    assert_point_near(
        reversed.evaluate(-1.6).unwrap(),
        open.evaluate(1.6).unwrap(),
    );
    assert_point_near(
        reversed.evaluate(-0.25).unwrap(),
        open.evaluate(0.25).unwrap(),
    );

    let closed = NurbsCurve::try_control_point_curve_with_closure(
        3,
        vec![
            point(-3.0, 0.0),
            point(-1.0, 3.0),
            point(2.0, 4.0),
            point(5.0, 1.0),
            point(4.0, -3.0),
            point(0.0, -4.0),
        ],
        ControlPointCurveClosure::Smooth,
    )
    .unwrap();
    let domain = closed.domain();
    let period = *domain.end() - *domain.start();
    let start = closed.parameter_at(0.8).unwrap();
    let end = closed.parameter_at(0.2).unwrap();
    let wrapped = closed.try_subcurve(start, end).unwrap();
    assert_eq!(wrapped.domain(), start..=end + period);
    assert!(!wrapped.is_closed().unwrap());
    for sample in 0..=32 {
        let parameter = start + (end + period - start) * sample as Real / 32.0;
        let source_parameter = if parameter <= *domain.end() {
            parameter
        } else {
            parameter - period
        };
        assert_point_near(
            wrapped.evaluate(parameter).unwrap(),
            closed.evaluate(source_parameter).unwrap(),
        );
    }
    assert_eq!(
        closed.try_subcurve(*domain.end(), end).unwrap(),
        closed.try_trimmed(*domain.start()..=end).unwrap()
    );
    assert_eq!(
        closed.try_subcurve(start, *domain.start()).unwrap(),
        closed.try_trimmed(start..=*domain.end()).unwrap()
    );

    for (start, end) in [
        (0.5, 0.5),
        (Real::NAN, 0.5),
        (*open.domain().start() - 1.0, 0.5),
    ] {
        assert_eq!(
            open.try_subcurve(start, end),
            Err(GeometryError::InvalidCurveTrimInterval)
        );
    }
}

#[test]
fn splitting_periodic_curve_opens_and_clamps_both_pieces() {
    let curve = NurbsCurve::try_control_point_curve_with_closure(
        3,
        vec![
            point(-3.0, 0.0),
            point(-1.0, 3.0),
            point(2.0, 4.0),
            point(5.0, 1.0),
            point(4.0, -3.0),
            point(0.0, -4.0),
        ],
        ControlPointCurveClosure::Smooth,
    )
    .unwrap();
    let domain = curve.domain();
    assert_eq!(curve.try_trimmed(domain.clone()).unwrap(), curve);
    let split = curve.parameter_at(0.43).unwrap();
    let (left, right) = curve.try_split(split).unwrap();

    assert!(!left.is_periodic());
    assert!(!right.is_periodic());
    assert!(
        left.knots()[..4]
            .iter()
            .all(|knot| *knot == *domain.start())
    );
    assert!(
        left.knots()[left.knots().len() - 4..]
            .iter()
            .all(|knot| *knot == split)
    );
    assert!(right.knots()[..4].iter().all(|knot| *knot == split));
    assert!(
        right.knots()[right.knots().len() - 4..]
            .iter()
            .all(|knot| *knot == *domain.end())
    );
    for sample in 0..=40 {
        let parameter = curve.parameter_at(sample as Real / 40.0).unwrap();
        let piece = if parameter <= split { &left } else { &right };
        assert_point_near(
            piece.evaluate(parameter).unwrap(),
            curve.evaluate(parameter).unwrap(),
        );
    }
    assert_point_near(
        left.evaluate(*domain.start()).unwrap(),
        right.evaluate(*domain.end()).unwrap(),
    );
}
