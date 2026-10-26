use super::*;

#[test]
fn nurbs_end_weight_normalization_preserves_the_projective_locus() {
    let curve = NurbsCurve::try_new_rational(
        2,
        vec![
            WeightedPoint3::try_new(point(0.0, 0.0), 2.0).unwrap(),
            WeightedPoint3::try_new(point(3.0, 5.0), 3.0).unwrap(),
            WeightedPoint3::try_new(point(8.0, 1.0), 8.0).unwrap(),
        ],
        vec![2.0, 2.0, 2.0, 6.0, 6.0, 6.0],
    )
    .unwrap();
    let normalized = curve.try_normalized_end_weights().unwrap();
    assert_eq!(normalized.domain(), curve.domain());
    assert_eq!(normalized.knots(), curve.knots());
    assert_eq!(
        normalized
            .control_points()
            .iter()
            .map(|control| control.point())
            .collect::<Vec<_>>(),
        curve
            .control_points()
            .iter()
            .map(|control| control.point())
            .collect::<Vec<_>>()
    );
    for (control, expected) in normalized.control_points().iter().zip([1.0, 0.75, 1.0]) {
        assert!(Tolerance::DEFAULT.approx_eq(control.weight(), expected));
    }

    let projective_scale = 0.5;
    for sample in 0..=32 {
        let normalized_parameter = sample as Real / 32.0;
        let source_fraction = projective_scale * normalized_parameter
            / (1.0 - normalized_parameter + projective_scale * normalized_parameter);
        let source_parameter = 2.0 + 4.0 * source_fraction;
        let target_parameter = 2.0 + 4.0 * normalized_parameter;
        assert_point_near(
            curve.evaluate(source_parameter).unwrap(),
            normalized.evaluate(target_parameter).unwrap(),
        );
    }

    let multispan = NurbsCurve::try_new_rational(
        2,
        vec![
            WeightedPoint3::try_new(point(0.0, 0.0), 2.0).unwrap(),
            WeightedPoint3::try_new(point(2.0, 3.0), 3.0).unwrap(),
            WeightedPoint3::try_new(point(4.0, 1.0), 5.0).unwrap(),
            WeightedPoint3::try_new(point(6.0, 0.0), 8.0).unwrap(),
        ],
        vec![0.0, 0.0, 0.0, 1.0, 2.0, 2.0, 2.0],
    )
    .unwrap();
    let normalized_multispan = multispan.try_normalized_end_weights().unwrap();
    assert_eq!(normalized_multispan.control_points()[0].weight(), 1.0);
    assert_eq!(normalized_multispan.control_points()[3].weight(), 1.0);
    assert_ne!(normalized_multispan.knots(), multispan.knots());
    for sample in 0..=32 {
        let normalized_parameter = sample as Real / 32.0;
        let source_fraction =
            0.5 * normalized_parameter / (1.0 - normalized_parameter + 0.5 * normalized_parameter);
        assert_point_near(
            multispan
                .evaluate(multispan.parameter_at(source_fraction).unwrap())
                .unwrap(),
            normalized_multispan
                .evaluate(
                    normalized_multispan
                        .parameter_at(normalized_parameter)
                        .unwrap(),
                )
                .unwrap(),
        );
    }
}

#[test]
fn make_uniform_preserves_rational_controls_and_resets_clamped_knots() {
    let controls = vec![
        WeightedPoint3::try_new(point(0.0, 0.0), 1.0).unwrap(),
        WeightedPoint3::try_new(point(1.0, 2.0), 0.5).unwrap(),
        WeightedPoint3::try_new(point(3.0, 1.0), 2.0).unwrap(),
        WeightedPoint3::try_new(point(4.0, 0.0), 1.0).unwrap(),
    ];
    let curve = NurbsCurve::try_new_rational(
        2,
        controls.clone(),
        vec![10.0, 10.0, 10.0, 10.2, 11.0, 11.0, 11.0],
    )
    .unwrap();

    let uniform = curve.try_make_uniform().unwrap();

    assert_eq!(uniform.degree(), 2);
    assert_eq!(uniform.control_points(), controls);
    assert_eq!(uniform.knots(), &[0.0, 0.0, 0.0, 1.0, 2.0, 2.0, 2.0]);
    assert_eq!(uniform.domain(), 0.0..=2.0);
    assert!(uniform.is_rational());
}

#[test]
fn make_uniform_retains_each_endpoint_clamp_independently() {
    let controls = vec![
        point(0.0, 0.0),
        point(1.0, 3.0),
        point(3.0, -2.0),
        point(6.0, 4.0),
        point(9.0, -1.0),
        point(10.0, 0.0),
    ];
    let cases = [
        (
            vec![0.0, 0.0, 1.0, 2.0, 4.0, 7.0, 10.0, 12.0, 13.0, 13.0],
            vec![0.0, 0.0, 1.0, 2.0, 3.0, 4.0, 5.0, 6.0, 7.0, 7.0],
        ),
        (
            vec![0.0, 0.0, 0.0, 0.0, 2.0, 5.0, 8.0, 10.0, 12.0, 12.0],
            vec![0.0, 0.0, 0.0, 0.0, 1.0, 2.0, 3.0, 4.0, 5.0, 5.0],
        ),
        (
            vec![0.0, 0.0, 2.0, 4.0, 6.0, 8.0, 10.0, 10.0, 10.0, 10.0],
            vec![0.0, 0.0, 1.0, 2.0, 3.0, 4.0, 5.0, 5.0, 5.0, 5.0],
        ),
    ];

    for (source_knots, expected_knots) in cases {
        let curve = NurbsCurve::try_new(3, controls.clone(), source_knots).unwrap();
        assert_eq!(curve.try_make_uniform().unwrap().knots(), expected_knots);
    }
}

#[test]
fn make_uniform_retains_periodic_topology() {
    let points = [
        (0.2857142857142857, 2.0),
        (-0.5714285714285714, -0.5714285714285714),
        (2.0, 0.2857142857142857),
        (4.571428571428571, -0.5714285714285714),
        (3.714285714285714, 2.0),
        (4.571428571428571, 4.571428571428571),
        (2.0, 3.714285714285714),
        (-0.5714285714285714, 4.571428571428571),
        (0.2857142857142857, 2.0),
        (-0.5714285714285714, -0.5714285714285714),
        (2.0, 0.2857142857142857),
    ];
    let controls = points
        .into_iter()
        .map(|(x, y)| point(x, y))
        .collect::<Vec<_>>();
    let curve = NurbsCurve::try_new(
        3,
        controls,
        vec![
            -2.0, -2.0, -1.0, 0.0, 1.0, 2.0, 3.0, 4.0, 5.0, 6.0, 7.0, 8.0, 9.0, 10.0, 10.0,
        ],
    )
    .unwrap();
    assert!(curve.is_periodic());

    let uniform = curve.try_make_uniform().unwrap();

    assert_eq!(
        uniform.knots(),
        &[
            0.0, 0.0, 1.0, 2.0, 3.0, 4.0, 5.0, 6.0, 7.0, 8.0, 9.0, 10.0, 11.0, 12.0, 12.0
        ]
    );
    assert_eq!(uniform.domain(), 2.0..=10.0);
    assert!(uniform.is_periodic());
    assert!(uniform.is_closed().unwrap());
}

#[test]
fn uniformly_scaling_weights_does_not_change_curve() {
    let points = [point(0.0, 0.0), point(1.0, 2.0), point(2.0, 0.0)];
    let make_curve = |scale: Real| {
        NurbsCurve::try_new_rational(
            2,
            points
                .into_iter()
                .zip([1.0, 0.25, 2.0])
                .map(|(point, weight)| WeightedPoint3::try_new(point, weight * scale).unwrap())
                .collect(),
            vec![0.0, 0.0, 0.0, 1.0, 1.0, 1.0],
        )
        .unwrap()
    };
    let ordinary = make_curve(1.0);
    let huge = make_curve(1.0e200);
    assert_point_near(
        ordinary.evaluate(0.37).unwrap(),
        huge.evaluate(0.37).unwrap(),
    );
}

#[test]
fn change_degree_matches_rhino_knot_and_greville_interpolation_rules() {
    let source = NurbsCurve::try_new(
        3,
        vec![
            Point3::try_new(0.0, 0.0, 0.0).unwrap(),
            Point3::try_new(2.0, 4.0, 1.0).unwrap(),
            Point3::try_new(5.0, -1.0, 2.0).unwrap(),
            Point3::try_new(7.0, 3.0, -1.0).unwrap(),
            Point3::try_new(9.0, 1.0, 0.0).unwrap(),
            Point3::try_new(12.0, 5.0, 2.0).unwrap(),
        ],
        vec![0.0, 0.0, 0.0, 0.0, 1.0, 3.0, 7.0, 7.0, 7.0, 7.0],
    )
    .unwrap();

    let elevated = source.try_change_degree(5, false).unwrap();
    assert_eq!(elevated.degree(), 5);
    assert_eq!(elevated.control_points().len(), 12);
    assert_eq!(
        elevated.knots(),
        &[
            0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 1.0, 1.0, 1.0, 3.0, 3.0, 3.0, 7.0, 7.0, 7.0, 7.0, 7.0,
            7.0,
        ]
    );
    for sample in 0..=64 {
        let parameter = source.parameter_at(sample as Real / 64.0).unwrap();
        assert_point_near(
            elevated.evaluate(parameter).unwrap(),
            source.evaluate(parameter).unwrap(),
        );
    }

    let reduced = source.try_change_degree(2, false).unwrap();
    assert_eq!(reduced.knots(), &[0.0, 0.0, 0.0, 1.0, 3.0, 7.0, 7.0, 7.0]);
    let expected = [
        [0.0, 0.0, 0.0],
        [2.795509342977698, 3.882157926461724, 1.4232971669680532],
        [5.778782399035563, -0.4382157926461724, 1.2326702833031946],
        [7.876130198915011, 1.465340566606389, -1.1549125979505726],
        [12.0, 5.0, 2.0],
    ];
    for (control, expected) in reduced.control_points().iter().zip(expected) {
        assert_point_near(control.point(), Point3::try_from(expected).unwrap());
    }

    let deformable = source.try_change_degree(5, true).unwrap();
    assert_eq!(
        deformable.knots(),
        &[
            0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 1.0, 3.0, 7.0, 7.0, 7.0, 7.0, 7.0, 7.0
        ]
    );
    assert_point_near(
        deformable.control_points()[3].point(),
        Point3::try_new(6.353798126845488, -3.487896224537079, 1.5666404989296816).unwrap(),
    );
    assert_eq!(source.try_change_degree(3, true).unwrap(), source);
    assert_eq!(
        source.try_change_degree(0, false),
        Err(GeometryError::InvalidDegree)
    );
}

#[test]
fn signed_rational_weights_evaluate_and_refine_projectively() {
    assert!(WeightedPoint3::try_new(point(0.0, 0.0), -0.2).is_ok());
    assert!(WeightedPoint3::try_new(point(0.0, 0.0), 0.0).is_err());
    let curve = NurbsCurve::try_new_rational(
        2,
        vec![
            WeightedPoint3::try_new(point(0.0, 0.0), 1.0).unwrap(),
            WeightedPoint3::try_new(point(2.0, 3.0), -0.2).unwrap(),
            WeightedPoint3::try_new(point(5.0, 0.0), 1.0).unwrap(),
        ],
        vec![0.0, 0.0, 0.0, 1.0, 1.0, 1.0],
    )
    .unwrap();
    assert_point_near(curve.evaluate(0.5).unwrap(), point(2.625, -0.75));

    let refined = curve.try_insert_knot(0.25, 1).unwrap();
    for sample in 0..=32 {
        let parameter = sample as Real / 32.0;
        assert_point_near(
            refined.evaluate(parameter).unwrap(),
            curve.evaluate(parameter).unwrap(),
        );
    }
}
