use super::*;

#[test]
fn normalized_multispan_trim_retains_piecewise_bezier_breaks() {
    let curve = NurbsCurve::try_new_rational(
        2,
        [
            ([0.0, 2.0], 1.0),
            ([2.0, 3.0], 0.8),
            ([5.0, 6.0], 1.2),
            ([8.0, 7.5], 0.9),
            ([10.0, 8.0], 1.0),
        ]
        .into_iter()
        .map(|(coordinates, weight)| {
            WeightedPoint3::try_new(point(coordinates[0], coordinates[1]), weight).unwrap()
        })
        .collect(),
        vec![2.0, 2.0, 2.0, 3.0, 5.0, 6.0, 6.0, 6.0],
    )
    .unwrap();
    let end = 3.361_818_303_014_144;
    let trimmed = curve
        .try_trimmed_with_normalized_end_weights(2.0..=end)
        .unwrap();

    assert_eq!(trimmed.degree(), 2);
    assert_eq!(trimmed.knots(), &[2.0, 2.0, 2.0, 3.0, 3.0, end, end, end]);
    for ((control, expected_point), expected_weight) in trimmed
        .control_points()
        .iter()
        .zip([
            point(0.0, 2.0),
            point(2.0, 3.0),
            point(3.285_714_285_714_285_6, 4.285_714_285_714_286),
            point(3.664_855_640_638_289, 4.664_855_640_638_289),
            point(4.0, 4.970_966_978_942_94),
        ])
        .zip([
            1.0,
            0.8,
            0.933_333_333_333_333_2,
            0.974_514_160_436_564_2,
            1.0,
        ])
    {
        assert_point_near(control.point(), expected_point);
        assert!(Tolerance::DEFAULT.approx_eq(control.weight(), expected_weight));
    }
    for sample in 0..=32 {
        let point = trimmed
            .evaluate(trimmed.parameter_at(sample as Real / 32.0).unwrap())
            .unwrap();
        let source_parameter = curve.closest_parameter(point, Tolerance::DEFAULT).unwrap();
        assert!(source_parameter >= 2.0 && source_parameter <= end);
        assert_point_near(point, curve.evaluate(source_parameter).unwrap());
    }
}

#[test]
fn split_preserves_rational_curve_parameters_and_derivatives() {
    let middle_weight = 0.5_f64.sqrt();
    let curve = NurbsCurve::try_new_rational(
        2,
        vec![
            WeightedPoint3::try_new(point(1.0, 0.0), 1.0).unwrap(),
            WeightedPoint3::try_new(point(1.0, 1.0), middle_weight).unwrap(),
            WeightedPoint3::try_new(point(0.0, 1.0), 1.0).unwrap(),
        ],
        vec![0.0, 0.0, 0.0, 1.0, 1.0, 1.0],
    )
    .unwrap();
    let (left, right) = curve.try_split(0.4).unwrap();

    assert_eq!(left.domain(), 0.0..=0.4);
    assert_eq!(right.domain(), 0.4..=1.0);
    assert!(
        left.knots()[left.knots().len() - 3..]
            .iter()
            .all(|knot| *knot == 0.4)
    );
    assert!(right.knots()[..3].iter().all(|knot| *knot == 0.4));
    assert_point_near(left.evaluate(0.4).unwrap(), right.evaluate(0.4).unwrap());
    for (piece, start, end) in [(&left, 0.0_f64, 0.4_f64), (&right, 0.4, 1.0)] {
        for sample in 0..=16 {
            let fraction = sample as Real / 16.0;
            let parameter = start.mul_add(1.0 - fraction, end * fraction);
            assert_point_near(
                piece.evaluate(parameter).unwrap(),
                curve.evaluate(parameter).unwrap(),
            );
            let actual = piece.derivative_at(parameter).unwrap();
            let expected = curve.derivative_at(parameter).unwrap();
            assert!(Tolerance::DEFAULT.approx_eq(actual.x(), expected.x()));
            assert!(Tolerance::DEFAULT.approx_eq(actual.y(), expected.y()));
            assert!(Tolerance::DEFAULT.approx_eq(actual.z(), expected.z()));
        }
    }
}

#[test]
fn split_reuses_degree_multiplicity_control_and_preserves_full_break() {
    let continuous = NurbsCurve::try_new(
        2,
        vec![
            point(0.0, 0.0),
            point(1.0, 3.0),
            point(2.0, 1.0),
            point(5.0, -2.0),
            point(8.0, 0.0),
        ],
        vec![0.0, 0.0, 0.0, 0.5, 0.5, 1.0, 1.0, 1.0],
    )
    .unwrap();
    let (left, right) = continuous.try_split(0.5).unwrap();
    assert_eq!(left.control_points().len(), 3);
    assert_eq!(right.control_points().len(), 3);
    assert_eq!(left.control_points()[2], right.control_points()[0]);
    assert_point_near(
        left.evaluate(0.5).unwrap(),
        continuous.evaluate(0.5).unwrap(),
    );
    assert_point_near(
        right.evaluate(0.5).unwrap(),
        continuous.evaluate(0.5).unwrap(),
    );

    let discontinuous = NurbsCurve::try_new(
        1,
        vec![
            point(0.0, 0.0),
            point(1.0, 0.0),
            point(10.0, 0.0),
            point(11.0, 0.0),
        ],
        vec![0.0, 0.0, 0.5, 0.5, 1.0, 1.0],
    )
    .unwrap();
    let (left, right) = discontinuous.try_split(0.5).unwrap();
    assert_eq!(left.control_points().len(), 2);
    assert_eq!(right.control_points().len(), 2);
    assert_eq!(left.evaluate(0.5).unwrap(), point(1.0, 0.0));
    assert_eq!(right.evaluate(0.5).unwrap(), point(10.0, 0.0));
    assert_ne!(left.control_points()[1], right.control_points()[0]);
}

#[test]
fn partial_trim_is_exact_and_clamps_nonclamped_ends() {
    let curve = NurbsCurve::try_new_rational(
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
    let trimmed = curve.try_trimmed(0.25..=1.6).unwrap();

    assert_eq!(trimmed.domain(), 0.25..=1.6);
    assert!(trimmed.knots()[..3].iter().all(|knot| *knot == 0.25));
    assert!(
        trimmed.knots()[trimmed.knots().len() - 3..]
            .iter()
            .all(|knot| *knot == 1.6)
    );
    for sample in 0..=32 {
        let fraction = sample as Real / 32.0;
        let parameter = 0.25_f64.mul_add(1.0 - fraction, 1.6 * fraction);
        assert_point_near(
            trimmed.evaluate(parameter).unwrap(),
            curve.evaluate(parameter).unwrap(),
        );
    }

    let from_start = curve.try_trimmed(0.0..=1.6).unwrap();
    assert!(from_start.knots()[..3].iter().all(|knot| *knot == 0.0));
    assert_point_near(
        from_start.evaluate(0.0).unwrap(),
        curve.evaluate(0.0).unwrap(),
    );
    assert_eq!(curve.try_trimmed(curve.domain()).unwrap(), curve);
}

#[test]
fn splitting_at_multiple_parameters_sorts_and_preserves_exact_pieces() {
    let curve = NurbsCurve::try_new_rational(
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
    let pieces = curve.try_split_at_parameters(&[1.6, 0.25, 0.8]).unwrap();
    assert_eq!(pieces.len(), 4);
    assert_eq!(pieces[0].domain(), 0.0..=0.25);
    assert_eq!(pieces[1].domain(), 0.25..=0.8);
    assert_eq!(pieces[2].domain(), 0.8..=1.6);
    assert_eq!(pieces[3].domain(), 1.6..=2.0);
    for piece in &pieces {
        for sample in 0..=8 {
            let parameter = piece.parameter_at(sample as Real / 8.0).unwrap();
            assert_point_near(
                piece.evaluate(parameter).unwrap(),
                curve.evaluate(parameter).unwrap(),
            );
        }
    }
    for parameters in [&[][..], &[0.5, 0.5], &[0.0, 0.5], &[Real::NAN]] {
        assert_eq!(
            curve.try_split_at_parameters(parameters),
            Err(GeometryError::InvalidCurveSplitParameter)
        );
    }

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
    let parameters = [
        closed.parameter_at(0.8).unwrap(),
        closed.parameter_at(0.2).unwrap(),
        closed.parameter_at(0.55).unwrap(),
    ];
    let pieces = closed.try_split_at_parameters(&parameters).unwrap();
    assert_eq!(pieces.len(), 3);
    let mut sorted = parameters;
    sorted.sort_by(Real::total_cmp);
    let period = *closed.domain().end() - *closed.domain().start();
    assert_eq!(pieces[0].domain(), sorted[0]..=sorted[1]);
    assert_eq!(pieces[1].domain(), sorted[1]..=sorted[2]);
    assert_eq!(pieces[2].domain(), sorted[2]..=sorted[0] + period);
    let relocated = closed.try_split_at_parameters(&[sorted[1]]).unwrap();
    assert_eq!(relocated.len(), 1);
    assert_eq!(relocated[0].domain(), sorted[1]..=sorted[1] + period);
    assert!(!relocated[0].is_periodic());
}
