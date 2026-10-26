use super::*;

#[test]
fn reparameterization_preserves_shape_and_maps_the_full_knot_vector() {
    let curve = NurbsCurve::try_new(
        2,
        vec![
            point(-2.0, 1.0),
            point(0.0, 3.0),
            point(2.0, -1.0),
            point(4.0, 2.0),
            point(7.0, 0.0),
        ],
        vec![-2.0, -1.0, 0.0, 1.0, 2.0, 3.0, 4.0, 5.0],
    )
    .unwrap();
    let mapped = curve.try_reparameterized(10.0..=16.0).unwrap();
    assert_eq!(mapped.domain(), 10.0..=16.0);
    assert_eq!(
        mapped.knots(),
        &[8.0, 8.0, 10.0, 12.0, 14.0, 16.0, 18.0, 18.0]
    );
    for sample in 0..=32 {
        let normalized = sample as Real / 32.0;
        assert_point_near(
            curve
                .evaluate(curve.parameter_at(normalized).unwrap())
                .unwrap(),
            mapped
                .evaluate(mapped.parameter_at(normalized).unwrap())
                .unwrap(),
        );
    }

    for domain in [
        1.0..=1.0,
        2.0..=-1.0,
        Real::NEG_INFINITY..=1.0,
        0.0..=Real::NAN,
    ] {
        assert!(mapped.try_reparameterized(domain).is_err());
    }
}

#[test]
fn clamped_uniform_curve_has_expected_knots_and_endpoints() {
    let controls = vec![
        point(0.0, 0.0),
        point(1.0, 2.0),
        point(2.0, 2.0),
        point(3.0, 0.0),
        point(4.0, 1.0),
    ];
    let curve = NurbsCurve::try_clamped_uniform(3, controls.clone()).unwrap();
    assert_eq!(
        curve.knots(),
        &[0.0, 0.0, 0.0, 0.0, 0.5, 1.0, 1.0, 1.0, 1.0]
    );
    assert_eq!(curve.evaluate(0.0).unwrap(), controls[0]);
    assert_eq!(curve.evaluate(1.0).unwrap(), controls[4]);
}

#[test]
fn affine_transform_preserves_knots_weights_and_evaluation() {
    let curve = NurbsCurve::try_new_rational(
        2,
        vec![
            WeightedPoint3::try_new(point(0.0, 0.0), 1.0).unwrap(),
            WeightedPoint3::try_new(point(1.0, 2.0), 0.5).unwrap(),
            WeightedPoint3::try_new(point(3.0, 0.0), 2.0).unwrap(),
        ],
        vec![0.0, 0.0, 0.0, 1.0, 1.0, 1.0],
    )
    .unwrap();
    let transform = AffineTransform3::try_new(
        [[2.0, -1.0, 0.0], [0.5, 3.0, 0.0], [0.0, 0.0, 1.0]],
        Vector3::try_new(4.0, -2.0, 7.0).unwrap(),
    )
    .unwrap();
    let transformed = curve.transformed(transform).unwrap();

    assert_eq!(transformed.knots(), curve.knots());
    assert_eq!(
        transformed
            .control_points()
            .iter()
            .map(|control| control.weight())
            .collect::<Vec<_>>(),
        vec![1.0, 0.5, 2.0]
    );
    assert_point_near(
        transformed.evaluate(0.37).unwrap(),
        transform
            .transform_point(curve.evaluate(0.37).unwrap())
            .unwrap(),
    );
}

#[test]
fn fully_multiple_knot_selects_right_hand_piece() {
    let curve = NurbsCurve::try_new(
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
    assert_eq!(curve.evaluate(0.5).unwrap(), point(10.0, 0.0));
    assert!(curve.evaluate(0.5_f64.next_down()).unwrap().x() < 1.0);
    assert_eq!(curve.derivative_at(0.5).unwrap().x(), 2.0);
}

#[test]
fn knot_insertion_preserves_a_rational_nonuniform_curve() {
    let curve = NurbsCurve::try_new_rational(
        3,
        vec![
            WeightedPoint3::try_new(point(-3.0, 1.0), 0.75).unwrap(),
            WeightedPoint3::try_new(point(-1.0, 4.0), 2.0).unwrap(),
            WeightedPoint3::try_new(point(2.0, -2.0), 0.4).unwrap(),
            WeightedPoint3::try_new(point(4.0, 3.0), 3.5).unwrap(),
            WeightedPoint3::try_new(point(7.0, -1.0), 1.25).unwrap(),
            WeightedPoint3::try_new(point(9.0, 2.0), 0.9).unwrap(),
        ],
        vec![0.0, 0.0, 0.0, 0.0, 0.35, 0.8, 1.0, 1.0, 1.0, 1.0],
    )
    .unwrap();
    let refined = curve.try_insert_knot(0.52, 3).unwrap();

    assert_eq!(refined.degree(), curve.degree());
    assert_eq!(refined.domain(), curve.domain());
    assert_eq!(refined.knot_multiplicity(0.52).unwrap(), 3);
    assert_eq!(
        refined.control_points().len(),
        curve.control_points().len() + 3
    );
    for sample in 0..=64 {
        let parameter = sample as Real / 64.0;
        assert_point_near(
            refined.evaluate(parameter).unwrap(),
            curve.evaluate(parameter).unwrap(),
        );
        let actual = refined.derivative_at(parameter).unwrap();
        let expected = curve.derivative_at(parameter).unwrap();
        assert!(Tolerance::DEFAULT.approx_eq(actual.x(), expected.x()));
        assert!(Tolerance::DEFAULT.approx_eq(actual.y(), expected.y()));
        assert!(Tolerance::DEFAULT.approx_eq(actual.z(), expected.z()));
    }

    let fully_refined = refined.try_insert_knot(0.52, 4).unwrap();
    assert_eq!(fully_refined.knot_multiplicity(0.52).unwrap(), 4);
    for parameter in [0.0, 0.2, 0.52, 0.7, 1.0] {
        assert_point_near(
            fully_refined.evaluate(parameter).unwrap(),
            curve.evaluate(parameter).unwrap(),
        );
    }
    let actual = fully_refined.derivative_at(0.52).unwrap();
    let expected = curve.derivative_at(0.52).unwrap();
    assert!(Tolerance::DEFAULT.approx_eq(actual.x(), expected.x()));
    assert!(Tolerance::DEFAULT.approx_eq(actual.y(), expected.y()));
    assert!(Tolerance::DEFAULT.approx_eq(actual.z(), expected.z()));
    assert_eq!(
        fully_refined.try_insert_knot(0.52, 2).unwrap(),
        fully_refined
    );
}

#[test]
fn knot_removal_matches_rhino_nearest_and_greville_rules() {
    let source = NurbsCurve::try_new(
        3,
        vec![
            point(0.0, 0.0),
            point(2.0, 4.0),
            point(5.0, -1.0),
            point(7.0, 3.0),
            point(9.0, 1.0),
            point(12.0, 5.0),
            point(15.0, -2.0),
        ],
        vec![0.0, 0.0, 0.0, 0.0, 1.0, 3.0, 5.0, 8.0, 8.0, 8.0, 8.0],
    )
    .unwrap();

    // Parameter 1.95 is numerically nearer knot one, but its curve point
    // is nearer the point at knot three; Rhino therefore removes three.
    let removed = source.try_remove_knot_near(1.95).unwrap();
    assert_eq!(removed.degree(), 3);
    assert_eq!(
        removed.knots(),
        &[0.0, 0.0, 0.0, 0.0, 1.0, 5.0, 8.0, 8.0, 8.0, 8.0]
    );
    let expected = [
        [0.0, 0.0, 0.0],
        [2.0145063256868987, 3.71309711419245, 0.0],
        [6.824390238905354, -0.8601625027947495, 0.0],
        [7.66282653203088, 2.112986366500404, 0.0],
        [12.02374775546043, 4.5303221697825595, 0.0],
        [15.0, -2.0, 0.0],
    ];
    for (control, expected) in removed.control_points().iter().zip(expected) {
        assert_point_near(control.point(), Point3::try_from(expected).unwrap());
    }

    assert!(matches!(
        source.try_remove_knot_near(0.1),
        Err(GeometryError::NoRemovableKnot { parameter: 0.1 })
    ));
    assert!(matches!(
        source.try_remove_knot_near(Real::NAN),
        Err(GeometryError::NonFinite { .. })
    ));

    let periodic = NurbsCurve::try_new(
        2,
        vec![
            point(0.0, 0.0),
            point(3.0, -1.0),
            point(5.0, 4.0),
            point(0.0, 0.0),
            point(3.0, -1.0),
        ],
        vec![-2.0, -1.0, 0.0, 1.0, 2.0, 3.0, 4.0, 5.0],
    )
    .unwrap();
    assert!(periodic.is_periodic());
    assert_eq!(
        periodic.try_remove_knot_near(1.0),
        Err(GeometryError::PeriodicKnotRemovalUnsupported { direction: "curve" })
    );
}

#[test]
fn knot_removal_handles_repeated_rational_knots() {
    let source = NurbsCurve::try_new_rational(
        2,
        [
            ([-1.0, 0.0, 0.0], 0.7),
            ([2.0, 5.0, 1.0], 1.6),
            ([6.0, -2.0, 0.0], 0.8),
            ([9.0, 4.0, -1.0], 1.3),
            ([12.0, 0.0, 2.0], 0.9),
        ]
        .into_iter()
        .map(|(point, weight)| {
            WeightedPoint3::try_new(Point3::try_from(point).unwrap(), weight).unwrap()
        })
        .collect(),
        vec![-2.0, -2.0, -2.0, 1.0, 1.0, 6.0, 6.0, 6.0],
    )
    .unwrap();
    let removed = source.try_remove_knot_near(1.0).unwrap();
    assert_eq!(removed.knots(), &[-2.0, -2.0, -2.0, 1.0, 6.0, 6.0, 6.0]);
    let expected_weights = [0.7, 1.3708333333333331, 1.0708333333333335, 0.9];
    for (control, expected) in removed.control_points().iter().zip(expected_weights) {
        assert!((control.weight() - expected).abs() <= 2.0e-15);
    }

    let non_clamped = NurbsCurve::try_new_rational(
        2,
        [
            ([0.0, 0.0, 0.0], 0.5),
            ([2.0, 4.0, 1.0], 1.1),
            ([5.0, -1.0, 2.0], 0.8),
            ([8.0, 2.0, 0.0], 1.4),
        ]
        .into_iter()
        .map(|(point, weight)| {
            WeightedPoint3::try_new(Point3::try_from(point).unwrap(), weight).unwrap()
        })
        .collect(),
        vec![8.0, 9.0, 10.0, 11.5, 14.0, 15.0, 16.0],
    )
    .unwrap();
    let endpoints = [
        non_clamped.evaluate(10.0).unwrap(),
        non_clamped.evaluate(14.0).unwrap(),
    ];
    let removed = non_clamped.try_remove_knot_near(11.3).unwrap();
    assert_eq!(removed.domain(), 10.0..=14.0);
    assert_eq!(removed.knots(), &[10.0, 10.0, 10.0, 14.0, 14.0, 14.0]);
    assert_point_near(removed.evaluate(10.0).unwrap(), endpoints[0]);
    assert_point_near(removed.evaluate(14.0).unwrap(), endpoints[1]);
}

#[test]
fn full_multiplicity_kink_predicate_ignores_collinear_knots() {
    let curve = |middle_y| {
        NurbsCurve::try_new(
            1,
            vec![
                Point3::try_new(0.0, 2.0, 0.0).unwrap(),
                Point3::try_new(5.0, middle_y, 0.0).unwrap(),
                Point3::try_new(10.0, 6.0, 0.0).unwrap(),
            ],
            vec![0.0, 0.0, 1.0, 2.0, 2.0],
        )
        .unwrap()
    };

    assert!(
        !curve(4.0)
            .has_full_multiplicity_kink(Tolerance::DEFAULT)
            .unwrap()
    );
    assert!(
        curve(7.0)
            .has_full_multiplicity_kink(Tolerance::DEFAULT)
            .unwrap()
    );
}

#[test]
fn multiple_knot_removal_matches_rhino_group_order_and_kink_filter() {
    let source = NurbsCurve::try_new(
        3,
        [
            [0.0, 0.0, 0.0],
            [1.0, 3.0, 1.0],
            [3.0, -2.0, 2.0],
            [5.0, 4.0, -1.0],
            [7.0, 0.0, 1.0],
            [9.0, -3.0, 2.0],
            [11.0, 5.0, -2.0],
            [13.0, 1.0, 0.0],
            [15.0, -1.0, 3.0],
            [18.0, 2.0, 1.0],
        ]
        .into_iter()
        .map(|point| Point3::try_from(point).unwrap())
        .collect(),
        vec![
            0.0, 0.0, 0.0, 0.0, 2.0, 2.0, 5.0, 5.0, 5.0, 7.0, 10.0, 10.0, 10.0, 10.0,
        ],
    )
    .unwrap();

    let (ordinary, removed) = source.try_remove_multiple_knots(false, 0.0).unwrap();
    assert_eq!(removed, 1);
    assert_eq!(
        ordinary.knots(),
        &[
            0.0, 0.0, 0.0, 0.0, 2.0, 5.0, 5.0, 5.0, 7.0, 10.0, 10.0, 10.0, 10.0
        ]
    );
    assert_point_near(
        ordinary.control_points()[1].point(),
        Point3::try_new(1.2511378848728234, 1.2599732262382861, 1.627844712182061).unwrap(),
    );

    let (none, removed) = source.try_remove_multiple_knots(true, 0.0).unwrap();
    assert_eq!((none, removed), (source.clone(), 0));

    let (below_kink, removed) = source
        .try_remove_multiple_knots(true, 130.0_f64.to_radians())
        .unwrap();
    assert_eq!(removed, 1);
    assert_eq!(below_kink.knots(), ordinary.knots());

    let (all, removed) = source
        .try_remove_multiple_knots(true, 135.0_f64.to_radians())
        .unwrap();
    assert_eq!(removed, 3);
    assert_eq!(
        all.knots(),
        &[0.0, 0.0, 0.0, 0.0, 2.0, 5.0, 7.0, 10.0, 10.0, 10.0, 10.0]
    );
    assert_point_near(
        all.control_points()[1].point(),
        Point3::try_new(1.277439372269475, 0.7925502880770412, 1.80206788629584).unwrap(),
    );

    assert_eq!(
        source.try_remove_multiple_knots(true, -1.0),
        Err(GeometryError::InvalidKnotRemovalAngle)
    );
    assert_eq!(
        source.try_remove_multiple_knots(true, Real::NAN),
        Err(GeometryError::InvalidKnotRemovalAngle)
    );

    let non_clamped = NurbsCurve::try_new(
        3,
        [
            [0.0, 0.0, 0.0],
            [1.0, 3.0, 1.0],
            [3.0, -2.0, 2.0],
            [6.0, 4.0, -1.0],
            [8.0, 0.0, 1.0],
            [11.0, 2.0, 0.0],
        ]
        .into_iter()
        .map(|point| Point3::try_from(point).unwrap())
        .collect(),
        vec![0.0, 1.0, 2.0, 3.0, 4.0, 4.0, 6.0, 7.0, 8.0, 9.0],
    )
    .unwrap();
    let endpoints = [
        non_clamped.evaluate(3.0).unwrap(),
        non_clamped.evaluate(6.0).unwrap(),
    ];
    let (non_clamped, removed) = non_clamped.try_remove_multiple_knots(false, 0.0).unwrap();
    assert_eq!(removed, 1);
    assert_eq!(non_clamped.domain(), 3.0..=6.0);
    assert_eq!(
        non_clamped.knots(),
        &[3.0, 3.0, 3.0, 3.0, 4.0, 6.0, 6.0, 6.0, 6.0]
    );
    assert_point_near(non_clamped.evaluate(3.0).unwrap(), endpoints[0]);
    assert_point_near(non_clamped.evaluate(6.0).unwrap(), endpoints[1]);

    let linear = NurbsCurve::try_new(
        1,
        [
            [0.0, 0.0, 0.0],
            [2.0, 3.0, 0.0],
            [5.0, -1.0, 1.0],
            [9.0, 2.0, 0.0],
            [12.0, 0.0, 2.0],
        ]
        .into_iter()
        .map(|point| Point3::try_from(point).unwrap())
        .collect(),
        vec![0.0, 0.0, 2.0, 4.0, 6.0, 8.0, 8.0],
    )
    .unwrap();
    let (linear, removed) = linear
        .try_remove_multiple_knots(true, std::f64::consts::PI)
        .unwrap();
    assert_eq!(removed, 3);
    assert_eq!(linear.knots(), &[0.0, 0.0, 8.0, 8.0]);
    assert_eq!(linear.control_points().len(), 2);
}

#[test]
fn endpoint_refinement_extracts_inactive_controls_at_full_order_groups() {
    let controls = vec![
        point(99.0, 99.0),
        point(0.0, 0.0),
        point(1.0, 3.0),
        point(4.0, -1.0),
        point(6.0, 2.0),
    ];
    let original =
        NurbsCurve::try_new(2, controls, vec![-2.0, 0.0, 0.0, 0.0, 0.5, 1.0, 2.0, 3.0]).unwrap();
    for source in [original.clone(), original.reversed().unwrap()] {
        let before = source.clone();
        for (parameter, target) in [
            (*source.domain().start(), 3),
            (*source.domain().end(), 3),
            (*source.domain().start(), 1),
            (*source.domain().end(), 1),
            (source.parameter_at(0.5).unwrap(), 1),
            (source.parameter_at(0.25).unwrap(), 2),
        ] {
            let result = source.try_insert_knot(parameter, target).unwrap();
            // Revalidate the complete representation, including exterior
            // knot multiplicities that evaluation alone would not expose.
            assert_eq!(
                NurbsCurve::try_new_rational(
                    result.degree(),
                    result.control_points().to_vec(),
                    result.knots().to_vec(),
                )
                .unwrap(),
                result
            );
            assert_eq!(result.domain(), source.domain());
            for i in 0..=32 {
                let parameter = source.parameter_at(i as Real / 32.0).unwrap();
                assert_point_near(
                    result.evaluate(parameter).unwrap(),
                    source.evaluate(parameter).unwrap(),
                );
            }
        }
        let endpoint = if source.knot_multiplicity(*source.domain().start()).unwrap() == 3 {
            *source.domain().start()
        } else {
            *source.domain().end()
        };
        let clamped = source.try_insert_knot(endpoint, 3).unwrap();
        assert_eq!(clamped.control_points().len(), 4);
        assert_eq!(clamped.try_insert_knot(endpoint, 3).unwrap(), clamped);
        assert_eq!(source, before);
    }
}

#[test]
fn endpoint_refinement_preserves_nonclamped_curve_evaluation() {
    let curve = NurbsCurve::try_new(
        2,
        vec![
            point(-2.0, 1.0),
            point(0.0, 4.0),
            point(5.0, -2.0),
            point(8.0, 3.0),
        ],
        vec![-2.0, -1.0, 0.0, 0.8, 2.0, 3.0, 4.0],
    )
    .unwrap();

    for parameter in [0.0, 2.0] {
        let refined = curve.try_insert_knot(parameter, 3).unwrap();
        assert_eq!(refined.knot_multiplicity(parameter).unwrap(), 3);
        assert_eq!(refined.knots()[0], refined.knots()[1]);
        assert_eq!(
            refined.knots()[refined.knots().len() - 1],
            refined.knots()[refined.knots().len() - 2]
        );
        for sample in 0..=32 {
            let t = sample as Real / 16.0;
            let actual = refined.evaluate(t).unwrap();
            let expected = curve.evaluate(t).unwrap();
            assert!(
                actual.is_near(
                    expected,
                    Tolerance::try_new(1.0e-12, 1.0e-12, 1.0e-12).unwrap()
                ),
                "refined endpoint {parameter}, sample {t}: actual {actual:?}, expected {expected:?}"
            );
        }
    }
}

#[test]
fn refinement_handles_large_coordinates_and_weight_ranges() {
    let curve = NurbsCurve::try_new_rational(
        2,
        vec![
            WeightedPoint3::try_new(Point3::try_new(-1.0e300, 2.0e299, 0.0).unwrap(), 1.0e-100)
                .unwrap(),
            WeightedPoint3::try_new(Point3::try_new(8.0e299, -4.0e299, 0.0).unwrap(), 1.0e100)
                .unwrap(),
            WeightedPoint3::try_new(Point3::try_new(1.0e300, 6.0e299, 0.0).unwrap(), 2.0e99)
                .unwrap(),
        ],
        vec![0.0, 0.0, 0.0, 1.0, 1.0, 1.0],
    )
    .unwrap();
    let refined = curve.try_insert_knot(0.5, 2).unwrap();
    for parameter in [0.0, 0.2, 0.5, 0.8, 1.0] {
        let actual = refined.evaluate(parameter).unwrap().to_array();
        let expected = curve.evaluate(parameter).unwrap().to_array();
        assert!(
            actual
                .into_iter()
                .zip(expected)
                .all(|(actual, expected)| Tolerance::DEFAULT.approx_eq(actual, expected))
        );
    }
    assert!(refined.control_points().iter().all(|control| {
        control.weight().is_finite() && control.point().to_array().into_iter().all(Real::is_finite)
    }));
}

#[test]
fn refinement_split_and_trim_reject_invalid_requests() {
    let curve =
        NurbsCurve::try_clamped_uniform(2, vec![point(0.0, 0.0), point(1.0, 2.0), point(3.0, 0.0)])
            .unwrap();
    assert_eq!(
        curve.try_insert_knot(0.5, 0),
        Err(GeometryError::InvalidKnotMultiplicity {
            actual: 0,
            maximum: 3
        })
    );
    assert!(matches!(
        curve.try_insert_knot(0.5, 4),
        Err(GeometryError::InvalidKnotMultiplicity { .. })
    ));
    assert!(matches!(
        curve.try_insert_knot(-0.1, 1),
        Err(GeometryError::ParameterOutOfDomain { .. })
    ));
    assert!(curve.try_insert_knot(Real::NAN, 1).is_err());
    for parameter in [-1.0, 0.0, 1.0, 2.0, Real::NAN] {
        assert!(curve.try_split(parameter).is_err());
    }
    for interval in [
        0.5..=0.5,
        0.8..=0.2,
        -0.1..=0.8,
        0.2..=1.1,
        Real::NAN..=0.5,
        0.2..=Real::INFINITY,
    ] {
        assert_eq!(
            curve.try_trimmed(interval),
            Err(GeometryError::InvalidCurveTrimInterval)
        );
    }
}
