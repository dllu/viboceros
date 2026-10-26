use super::*;

#[test]
fn command_control_extension_opens_periodic_nets_and_resets_weights() {
    let unique = [[0., 0., 0.], [1., 3., 0.], [3., -1., 0.], [5., 0., 0.]]
        .into_iter()
        .map(|p| WeightedPoint3::try_new(Point3::try_from(p).unwrap(), 2.).unwrap())
        .collect();
    let source = NurbsCurve::try_unit_periodic_from_unique_controls(3, unique).unwrap();
    assert!(source.is_periodic());
    let extended = source.try_extend_control_point_net(false).unwrap();
    assert!(!extended.is_periodic());
    assert!(!extended.is_rational());
    assert_eq!(extended.domain(), source.domain());
    assert_point_near(
        extended.control_points()[0].point(),
        Point3::try_new(-0.5, -1.5, 0.).unwrap(),
    );
    assert!(extended.control_points().iter().all(|c| c.weight() == 1.));
    for (a, b) in extended.control_points()[1..]
        .iter()
        .zip(source.control_points())
    {
        assert_eq!(a.point(), b.point());
    }
}

#[test]
fn natural_extension_extrapolates_the_endpoint_nurbs_spans_exactly() {
    let curve = NurbsCurve::try_new(
        2,
        vec![point(0.0, 0.0), point(1.0, 2.0), point(3.0, 1.0)],
        vec![0.0, 0.0, 0.0, 1.0, 1.0, 1.0],
    )
    .unwrap();
    let extended = curve.try_extended_to(-1.0..=2.0).unwrap();

    assert_eq!(extended.domain(), -1.0..=2.0);
    assert_eq!(extended.knots(), &[-1.0, -1.0, -1.0, 2.0, 2.0, 2.0]);
    for parameter in [-1.0, -0.5, 0.0, 0.3, 1.0, 1.5, 2.0] {
        assert_point_near(
            extended.evaluate(parameter).unwrap(),
            point(
                parameter.mul_add(parameter, 2.0 * parameter),
                (-3.0 * parameter).mul_add(parameter, 4.0 * parameter),
            ),
        );
    }
}

#[test]
fn natural_extension_clamps_nonclamped_rational_ends_and_preserves_the_source() {
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
    let extended = curve.try_extended_to(-0.25..=2.25).unwrap();

    assert_eq!(extended.domain(), -0.25..=2.25);
    assert!(extended.knots()[..3].iter().all(|knot| *knot == -0.25));
    assert!(
        extended.knots()[extended.knots().len() - 3..]
            .iter()
            .all(|knot| *knot == 2.25)
    );
    for sample in 0..=32 {
        let parameter = 2.0 * sample as Real / 32.0;
        assert_point_near(
            extended.evaluate(parameter).unwrap(),
            curve.evaluate(parameter).unwrap(),
        );
    }
}

#[test]
fn natural_extension_by_length_matches_rhino_smooth_extension_parameters() {
    let curve = NurbsCurve::try_new(
        2,
        vec![
            Point3::try_new(0.0, 0.0, 0.0).unwrap(),
            Point3::try_new(1.0, 2.0, 1.0).unwrap(),
            Point3::try_new(3.0, 1.0, -1.0).unwrap(),
        ],
        vec![0.0, 0.0, 0.0, 1.0, 1.0, 1.0],
    )
    .unwrap();

    let start = curve
        .try_extended_by_length(CurveExtensionSide::Start, 1.75, Tolerance::DEFAULT)
        .unwrap();
    assert!(Tolerance::DEFAULT.approx_eq(*start.domain().start(), -0.294_749_475_954_422_64));
    assert_eq!(*start.domain().end(), 1.0);
    assert!(
        Tolerance::DEFAULT.approx_eq(
            start
                .try_trimmed(*start.domain().start()..=0.0)
                .unwrap()
                .length(Tolerance::DEFAULT)
                .unwrap(),
            1.75
        )
    );

    let both = curve
        .try_extended_by_length(CurveExtensionSide::Both, 1.5, Tolerance::DEFAULT)
        .unwrap();
    assert!(Tolerance::DEFAULT.approx_eq(*both.domain().start(), -0.258_483_515_102_362_95));
    assert!(Tolerance::DEFAULT.approx_eq(*both.domain().end(), 1.219_618_451_394_380_4));
}

#[test]
fn rational_natural_extension_by_length_reaches_the_requested_distance() {
    let curve = NurbsCurve::try_new_rational(
        2,
        vec![
            WeightedPoint3::try_new(point(1.0, 0.0), 1.0).unwrap(),
            WeightedPoint3::try_new(point(1.0, 1.0), 0.6).unwrap(),
            WeightedPoint3::try_new(point(0.0, 1.0), 1.0).unwrap(),
        ],
        vec![0.0, 0.0, 0.0, 1.0, 1.0, 1.0],
    )
    .unwrap();
    let extended = curve
        .try_extended_by_length(CurveExtensionSide::End, 0.75, Tolerance::DEFAULT)
        .unwrap();
    assert!(
        (*extended.domain().end() - 2.087_387_394_770_060_7).abs() < 5.0e-9,
        "actual domain {:?}",
        extended.domain()
    );
    let extension = extended
        .try_trimmed(1.0..=*extended.domain().end())
        .unwrap();
    assert!(Tolerance::DEFAULT.approx_eq(extension.length(Tolerance::DEFAULT).unwrap(), 0.75));
}

#[test]
fn linear_extension_by_length_adds_degree_matched_tangent_spans() {
    let curve = NurbsCurve::try_new(
        2,
        vec![
            Point3::try_new(0.0, 0.0, 0.0).unwrap(),
            Point3::try_new(1.0, 2.0, 1.0).unwrap(),
            Point3::try_new(3.0, 1.0, -1.0).unwrap(),
        ],
        vec![0.0, 0.0, 0.0, 1.0, 1.0, 1.0],
    )
    .unwrap();
    let extended = curve
        .try_extended_linearly_by_length(CurveExtensionSide::Both, 1.5, Tolerance::DEFAULT)
        .unwrap();

    let start = -1.5 / 24.0_f64.sqrt();
    let end = 1.0 + 1.5 / 6.0;
    assert!(Tolerance::DEFAULT.approx_eq(*extended.domain().start(), start));
    assert_eq!(*extended.domain().end(), end);
    assert_eq!(extended.knots().len(), 10);
    assert!(
        extended.knots()[..3]
            .iter()
            .all(|knot| Tolerance::DEFAULT.approx_eq(*knot, start))
    );
    assert_eq!(&extended.knots()[3..], &[0.0, 0.0, 1.0, 1.0, end, end, end]);
    let expected = [
        [
            -0.612_372_435_695_794_6,
            -1.224_744_871_391_589_2,
            -0.612_372_435_695_794_6,
        ],
        [
            -0.306_186_217_847_897_3,
            -0.612_372_435_695_794_6,
            -0.306_186_217_847_897_3,
        ],
        [0.0, 0.0, 0.0],
        [1.0, 2.0, 1.0],
        [3.0, 1.0, -1.0],
        [3.5, 0.75, -1.5],
        [4.0, 0.5, -2.0],
    ];
    assert_eq!(extended.control_points().len(), expected.len());
    for (control, expected) in extended.control_points().iter().zip(expected) {
        assert_point_near(control.point(), Point3::try_from(expected).unwrap());
        assert_eq!(control.weight(), 1.0);
    }
    assert!(
        Tolerance::DEFAULT.approx_eq(
            extended
                .try_trimmed(*extended.domain().start()..=0.0)
                .unwrap()
                .length(Tolerance::DEFAULT)
                .unwrap(),
            1.5,
        )
    );
    assert!(
        Tolerance::DEFAULT.approx_eq(
            extended
                .try_trimmed(1.0..=*extended.domain().end())
                .unwrap()
                .length(Tolerance::DEFAULT)
                .unwrap(),
            1.5,
        )
    );
}

#[test]
fn linear_extension_clamps_rational_nonclamped_ends_without_moving_the_source() {
    let curve = NurbsCurve::try_new_rational(
        2,
        vec![
            WeightedPoint3::try_new(Point3::try_new(-2.0, 1.0, 0.0).unwrap(), 0.75).unwrap(),
            WeightedPoint3::try_new(Point3::try_new(0.0, 4.0, 1.0).unwrap(), 2.0).unwrap(),
            WeightedPoint3::try_new(Point3::try_new(5.0, -2.0, -1.0).unwrap(), 0.5).unwrap(),
            WeightedPoint3::try_new(Point3::try_new(8.0, 3.0, 2.0).unwrap(), 1.25).unwrap(),
        ],
        vec![-1.0, -1.0, 0.0, 0.8, 2.0, 3.0, 3.0],
    )
    .unwrap();
    let extended = curve
        .try_extended_linearly_by_length(CurveExtensionSide::Both, 0.5, Tolerance::DEFAULT)
        .unwrap();

    assert_eq!(extended.control_points().len(), 8);
    assert!(
        extended.control_points()[..3]
            .iter()
            .all(|control| control.weight() == extended.control_points()[2].weight())
    );
    assert!(
        extended.control_points()[5..]
            .iter()
            .all(|control| control.weight() == extended.control_points()[5].weight())
    );
    for sample in 0..=32 {
        let parameter = 2.0 * sample as Real / 32.0;
        assert_point_near(
            extended.evaluate(parameter).unwrap(),
            curve.evaluate(parameter).unwrap(),
        );
    }
}

#[test]
fn circular_extension_uses_the_exact_endpoint_osculating_arc() {
    let curve = NurbsCurve::try_new(
        2,
        vec![point(0.0, 0.0), point(1.0, 2.0), point(3.0, 1.0)],
        vec![0.0, 0.0, 0.0, 1.0, 1.0, 1.0],
    )
    .unwrap();
    let extended = curve
        .try_extended_circularly_by_length(CurveExtensionSide::End, 2.0, Tolerance::DEFAULT)
        .unwrap();

    let end = 1.0 + 1.0 / 5.0_f64.sqrt();
    assert_eq!(extended.degree(), 2);
    assert!(Tolerance::DEFAULT.approx_eq(*extended.domain().end(), end));
    assert_eq!(extended.knot_multiplicity(1.0).unwrap(), 2);
    assert!(Tolerance::DEFAULT.approx_eq(
        extended.control_points()[3].weight(),
        0.975_103_993_210_479_4,
    ));
    assert_point_near(
        extended.control_points()[4].point(),
        Point3::try_new(4.533_130_546_115_57, -0.258_287_297_307_559_06, 0.0).unwrap(),
    );
    assert!(
        Tolerance::DEFAULT.approx_eq(
            extended
                .try_trimmed(1.0..=end)
                .unwrap()
                .length(Tolerance::DEFAULT)
                .unwrap(),
            2.0,
        )
    );

    let capped = curve
        .try_extended_circularly_by_length(CurveExtensionSide::End, 30.0, Tolerance::DEFAULT)
        .unwrap();
    assert_eq!(capped.control_points().len(), 11);
    assert_point_near(
        capped.evaluate(*capped.domain().end()).unwrap(),
        point(3.0, 1.0),
    );
    let capped_length = capped
        .try_trimmed(1.0..=*capped.domain().end())
        .unwrap()
        .length(Tolerance::DEFAULT)
        .unwrap();
    let circumference = std::f64::consts::TAU * 2.0 * 5.0_f64.sqrt();
    assert!(
        (capped_length - circumference).abs() < 1.0e-8,
        "actual full-circle length {capped_length}"
    );
}

#[test]
fn circular_extension_elevates_a_zero_curvature_line_fallback() {
    let curve = NurbsCurve::try_new(
        3,
        vec![
            point(0.0, 0.0),
            point(1.0, 1.0),
            point(2.0, 0.0),
            point(3.0, -1.0),
        ],
        vec![0.0, 0.0, 0.0, 0.0, 1.0, 1.0, 1.0, 1.0],
    )
    .unwrap();
    let extended = curve
        .try_extended_circularly_by_length(CurveExtensionSide::End, 2.0, Tolerance::DEFAULT)
        .unwrap();

    assert_eq!(extended.degree(), 3);
    assert_eq!(extended.control_points().len(), 7);
    assert_eq!(extended.knot_multiplicity(1.0).unwrap(), 3);
    assert_point_near(
        extended.control_points()[6].point(),
        Point3::try_new(4.414_213_562_373_095, -2.414_213_562_373_095, 0.0).unwrap(),
    );
    assert!(
        extended.control_points()[3..]
            .iter()
            .all(|control| control.weight() == 1.0)
    );

    let separate = curve
        .try_separate_circular_extensions_by_length(
            CurveExtensionSide::End,
            2.0,
            Tolerance::DEFAULT,
        )
        .unwrap();
    assert_eq!(separate.len(), 1);
    assert_eq!(separate[0].degree(), 1);
    assert_eq!(separate[0].domain(), 0.0..=1.0);
    assert_point_near(
        separate[0].control_points()[1].point(),
        extended.control_points()[6].point(),
    );

    let spatial = NurbsCurve::try_new(
        3,
        vec![
            Point3::try_new(0.0, 0.0, 0.0).unwrap(),
            Point3::try_new(1.0, 3.0, 1.0).unwrap(),
            Point3::try_new(4.0, -2.0, 2.0).unwrap(),
            Point3::try_new(7.0, 1.0, 0.0).unwrap(),
        ],
        vec![0.0, 0.0, 0.0, 0.0, 1.0, 1.0, 1.0, 1.0],
    )
    .unwrap();
    let separate = spatial
        .try_separate_circular_extensions_by_length(
            CurveExtensionSide::End,
            2.0,
            Tolerance::DEFAULT,
        )
        .unwrap();
    assert_eq!(separate[0].degree(), 2);
    assert_eq!(separate[0].control_points().len(), 3);
}

#[test]
fn natural_extension_ignores_interior_bound_and_rejects_invalid_inputs() {
    let curve = NurbsCurve::try_new(
        1,
        vec![point(0.0, 0.0), point(10.0, 0.0)],
        vec![0.0, 0.0, 1.0, 1.0],
    )
    .unwrap();
    let extended = curve.try_extended_to(0.25..=2.0).unwrap();
    assert_eq!(extended.domain(), 0.0..=2.0);
    assert_eq!(extended.evaluate(2.0).unwrap(), point(20.0, 0.0));

    for interval in [0.0..=1.0, 0.75..=0.25, Real::NAN..=2.0] {
        assert_eq!(
            curve.try_extended_to(interval),
            Err(GeometryError::InvalidCurveExtensionInterval)
        );
    }

    let closed = NurbsCurve::try_control_point_curve_with_closure(
        1,
        vec![
            point(0.0, 0.0),
            point(2.0, 0.0),
            point(2.0, 2.0),
            point(0.0, 2.0),
        ],
        ControlPointCurveClosure::Sharp,
    )
    .unwrap();
    assert_eq!(
        closed.try_extended_to(-1.0..=5.0),
        Err(GeometryError::CurveExtensionMustBeOpen)
    );
    assert_eq!(
        closed.try_extended_by_length(CurveExtensionSide::End, 1.0, Tolerance::DEFAULT),
        Err(GeometryError::CurveExtensionMustBeOpen)
    );
    assert_eq!(
        closed.try_merged_naturally_by_length(CurveExtensionSide::End, 1.0, Tolerance::DEFAULT,),
        Err(GeometryError::CurveExtensionMustBeOpen)
    );
    assert_eq!(
        closed.try_extended_linearly_by_length(CurveExtensionSide::End, 1.0, Tolerance::DEFAULT,),
        Err(GeometryError::CurveExtensionMustBeOpen)
    );
    assert_eq!(
        closed.try_merged_linearly_by_length(CurveExtensionSide::End, 1.0, Tolerance::DEFAULT,),
        Err(GeometryError::CurveExtensionMustBeOpen)
    );
    assert_eq!(
        closed.try_extended_circularly_by_length(CurveExtensionSide::End, 1.0, Tolerance::DEFAULT,),
        Err(GeometryError::CurveExtensionMustBeOpen)
    );
    assert_eq!(
        closed.try_merged_circularly_by_length(CurveExtensionSide::End, 1.0, Tolerance::DEFAULT,),
        Err(GeometryError::CurveExtensionMustBeOpen)
    );
    assert_eq!(
        closed.try_joined_circularly_by_length(CurveExtensionSide::End, 1.0, Tolerance::DEFAULT,),
        Err(GeometryError::CurveExtensionMustBeOpen)
    );
    assert_eq!(
        closed.try_separate_circular_extensions_by_length(
            CurveExtensionSide::End,
            1.0,
            Tolerance::DEFAULT,
        ),
        Err(GeometryError::CurveExtensionMustBeOpen)
    );
    assert_eq!(
        closed.try_joined_linearly_by_length(CurveExtensionSide::End, 1.0, Tolerance::DEFAULT,),
        Err(GeometryError::CurveExtensionMustBeOpen)
    );
    assert_eq!(
        closed.try_joined_naturally_by_length(CurveExtensionSide::End, 1.0, Tolerance::DEFAULT,),
        Err(GeometryError::CurveExtensionMustBeOpen)
    );
    assert_eq!(
        closed.try_joined_smoothly_by_length(CurveExtensionSide::End, 1.0, Tolerance::DEFAULT,),
        Err(GeometryError::CurveExtensionMustBeOpen)
    );
    assert_eq!(
        closed.try_separate_linear_extensions_by_length(
            CurveExtensionSide::End,
            1.0,
            Tolerance::DEFAULT,
        ),
        Err(GeometryError::CurveExtensionMustBeOpen)
    );
    assert_eq!(
        closed.try_separate_natural_extensions_by_length(
            CurveExtensionSide::End,
            1.0,
            Tolerance::DEFAULT,
        ),
        Err(GeometryError::CurveExtensionMustBeOpen)
    );
    assert_eq!(
        closed.try_separate_smooth_extensions_by_length(
            CurveExtensionSide::End,
            1.0,
            Tolerance::DEFAULT,
        ),
        Err(GeometryError::CurveExtensionMustBeOpen)
    );
    for length in [0.0, -1.0, Real::NAN] {
        assert_eq!(
            curve.try_extended_by_length(CurveExtensionSide::End, length, Tolerance::DEFAULT),
            Err(GeometryError::InvalidCurveExtensionLength)
        );
        assert_eq!(
            curve.try_merged_naturally_by_length(
                CurveExtensionSide::End,
                length,
                Tolerance::DEFAULT,
            ),
            Err(GeometryError::InvalidCurveExtensionLength)
        );
        assert_eq!(
            curve.try_extended_linearly_by_length(
                CurveExtensionSide::End,
                length,
                Tolerance::DEFAULT,
            ),
            Err(GeometryError::InvalidCurveExtensionLength)
        );
        assert_eq!(
            curve.try_merged_linearly_by_length(
                CurveExtensionSide::End,
                length,
                Tolerance::DEFAULT,
            ),
            Err(GeometryError::InvalidCurveExtensionLength)
        );
        assert_eq!(
            curve.try_extended_circularly_by_length(
                CurveExtensionSide::End,
                length,
                Tolerance::DEFAULT,
            ),
            Err(GeometryError::InvalidCurveExtensionLength)
        );
        assert_eq!(
            curve.try_merged_circularly_by_length(
                CurveExtensionSide::End,
                length,
                Tolerance::DEFAULT,
            ),
            Err(GeometryError::InvalidCurveExtensionLength)
        );
        assert_eq!(
            curve.try_joined_circularly_by_length(
                CurveExtensionSide::End,
                length,
                Tolerance::DEFAULT,
            ),
            Err(GeometryError::InvalidCurveExtensionLength)
        );
        assert_eq!(
            curve.try_separate_circular_extensions_by_length(
                CurveExtensionSide::End,
                length,
                Tolerance::DEFAULT,
            ),
            Err(GeometryError::InvalidCurveExtensionLength)
        );
        assert_eq!(
            curve.try_joined_linearly_by_length(
                CurveExtensionSide::End,
                length,
                Tolerance::DEFAULT,
            ),
            Err(GeometryError::InvalidCurveExtensionLength)
        );
        assert_eq!(
            curve.try_joined_naturally_by_length(
                CurveExtensionSide::End,
                length,
                Tolerance::DEFAULT,
            ),
            Err(GeometryError::InvalidCurveExtensionLength)
        );
        assert_eq!(
            curve.try_joined_smoothly_by_length(
                CurveExtensionSide::End,
                length,
                Tolerance::DEFAULT,
            ),
            Err(GeometryError::InvalidCurveExtensionLength)
        );
        assert_eq!(
            curve.try_separate_linear_extensions_by_length(
                CurveExtensionSide::End,
                length,
                Tolerance::DEFAULT,
            ),
            Err(GeometryError::InvalidCurveExtensionLength)
        );
        assert_eq!(
            curve.try_separate_natural_extensions_by_length(
                CurveExtensionSide::End,
                length,
                Tolerance::DEFAULT,
            ),
            Err(GeometryError::InvalidCurveExtensionLength)
        );
        assert_eq!(
            curve.try_separate_smooth_extensions_by_length(
                CurveExtensionSide::End,
                length,
                Tolerance::DEFAULT,
            ),
            Err(GeometryError::InvalidCurveExtensionLength)
        );
    }
}
