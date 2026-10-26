use super::*;

#[test]
fn control_point_curve_matches_rhino_degree_lowering_and_domain() {
    let controls = vec![
        Point3::try_new(0.0, 0.0, 0.0).unwrap(),
        Point3::try_new(1.0, 2.0, 0.5).unwrap(),
        Point3::try_new(4.0, -1.0, 2.0).unwrap(),
        Point3::try_new(4.5, 3.0, -0.5).unwrap(),
        Point3::try_new(10.0, 0.0, 1.0).unwrap(),
    ];
    let curve = NurbsCurve::try_control_point_curve(3, controls.clone()).unwrap();
    let domain_end = 17.976_753_701_093_052;
    assert_eq!(curve.degree(), 3);
    assert_eq!(
        curve
            .control_points()
            .iter()
            .map(|control| control.point())
            .collect::<Vec<_>>(),
        controls
    );
    for (actual, expected) in curve.knots().iter().zip([
        0.0,
        0.0,
        0.0,
        0.0,
        domain_end / 2.0,
        domain_end,
        domain_end,
        domain_end,
        domain_end,
    ]) {
        assert!((*actual - expected).abs() <= 2.0e-14);
    }

    let quadratic = NurbsCurve::try_control_point_curve(
        5,
        vec![point(0.0, 0.0), point(2.0, 3.0), point(10.0, 0.0)],
    )
    .unwrap();
    assert_eq!(quadratic.degree(), 2);
    assert_eq!(quadratic.domain(), 0.0..=13.0_f64.sqrt() + 73.0_f64.sqrt());

    assert!(matches!(
        NurbsCurve::try_control_point_curve(3, vec![point(1.0, 1.0), point(1.0, 1.0)]),
        Err(GeometryError::Degenerate {
            context: "control-point curve"
        })
    ));
}

#[test]
fn smooth_control_point_curve_matches_rhino_periodic_layout() {
    let unique = vec![
        Point3::try_new(0.0, 0.0, 0.0).unwrap(),
        Point3::try_new(1.0, 2.0, 0.5).unwrap(),
        Point3::try_new(4.0, -1.0, 2.0).unwrap(),
        Point3::try_new(4.5, 3.0, -0.5).unwrap(),
        Point3::try_new(10.0, 0.0, 1.0).unwrap(),
        Point3::try_new(8.0, -4.0, 2.0).unwrap(),
        Point3::try_new(2.0, -3.0, -1.0).unwrap(),
        Point3::try_new(-2.0, 1.0, 0.25).unwrap(),
    ];
    let curve = NurbsCurve::try_control_point_curve_with_closure(
        4,
        unique.clone(),
        ControlPointCurveClosure::Smooth,
    )
    .unwrap();
    assert_eq!(curve.degree(), 4);
    assert!(curve.is_periodic());
    assert!(curve.is_closed().unwrap());
    let expected_controls = unique[7..]
        .iter()
        .chain(&unique)
        .chain(&unique[..3])
        .copied()
        .collect::<Vec<_>>();
    assert_eq!(
        curve
            .control_points()
            .iter()
            .map(|control| control.point())
            .collect::<Vec<_>>(),
        expected_controls
    );
    assert!((*curve.domain().end() - 46.426_262_339_780_31).abs() <= 2.0e-14);
    let short_knots = &curve.knots()[1..curve.knots().len() - 1];
    for (actual, expected) in short_knots.iter().zip([
        -17.409_848_377_417_617,
        -11.606_565_584_945_077,
        -5.803_282_792_472_538_5,
        0.0,
        5.803_282_792_472_538_5,
        11.606_565_584_945_077,
        17.409_848_377_417_617,
        23.213_131_169_890_154,
        29.016_413_962_362_69,
        34.819_696_754_835_235,
        40.622_979_547_307_77,
        46.426_262_339_780_31,
        52.229_545_132_252_845,
        58.032_827_924_725_38,
        63.836_110_717_197_926,
    ]) {
        assert!((*actual - expected).abs() <= 3.0e-14);
    }

    let lowered = NurbsCurve::try_control_point_curve_with_closure(
        11,
        unique.clone(),
        ControlPointCurveClosure::Smooth,
    )
    .unwrap();
    assert_eq!(lowered.degree(), 8);
    assert_eq!(lowered.control_points().len(), 16);
    assert_eq!(lowered.control_points()[0].point(), unique[5]);
    assert!(lowered.is_periodic());
    assert!((*lowered.domain().end() - 70.187_373_289_648_95).abs() <= 3.0e-14);
}

#[test]
fn closed_control_point_curves_match_rhino_degree_lowering_and_seams() {
    let points = vec![
        Point3::try_new(0.0, 0.0, 0.0).unwrap(),
        Point3::try_new(2.0, 3.0, 1.0).unwrap(),
        Point3::try_new(10.0, 0.0, 0.0).unwrap(),
    ];
    let smooth = NurbsCurve::try_control_point_curve_with_closure(
        5,
        points.clone(),
        ControlPointCurveClosure::Smooth,
    )
    .unwrap();
    assert_eq!(smooth.degree(), 3);
    assert!(smooth.is_periodic());
    assert_eq!(
        smooth
            .control_points()
            .iter()
            .map(|control| control.point())
            .collect::<Vec<_>>(),
        vec![
            points[2], points[0], points[1], points[2], points[0], points[1]
        ]
    );
    assert!((*smooth.domain().end() - 36.085_640_040_590_51).abs() <= 2.0e-14);

    let sharp = NurbsCurve::try_control_point_curve_with_closure(
        5,
        points.clone(),
        ControlPointCurveClosure::Sharp,
    )
    .unwrap();
    assert_eq!(sharp.degree(), 3);
    assert!(!sharp.is_periodic());
    assert!(sharp.is_closed().unwrap());
    assert_eq!(
        sharp
            .control_points()
            .iter()
            .map(|control| control.point())
            .collect::<Vec<_>>(),
        vec![points[0], points[1], points[2], points[0]]
    );
    assert!((*sharp.domain().end() - 22.343_982_653_816_568).abs() <= 2.0e-14);

    let linear = NurbsCurve::try_control_point_curve_with_closure(
        1,
        points.clone(),
        ControlPointCurveClosure::Smooth,
    )
    .unwrap();
    assert_eq!(linear.degree(), 1);
    assert!(!linear.is_periodic());
    assert!(linear.is_closed().unwrap());

    let mut explicitly_closed = points.clone();
    explicitly_closed.push(points[0]);
    let normalized = NurbsCurve::try_control_point_curve_with_closure(
        3,
        explicitly_closed,
        ControlPointCurveClosure::Smooth,
    )
    .unwrap();
    assert_eq!(normalized, smooth);

    assert_eq!(
        NurbsCurve::try_control_point_curve_with_closure(
            3,
            points[..2].to_vec(),
            ControlPointCurveClosure::Sharp,
        ),
        Err(GeometryError::InsufficientClosedControlPoints { actual: 2 })
    );
}

#[test]
fn control_polygon_uses_rhino_periodic_window_and_rejects_zero_segments() {
    let points = vec![
        point(0.0, 0.0),
        point(2.0, 0.0),
        point(3.0, 2.0),
        point(1.0, 4.0),
        point(-1.0, 2.0),
    ];
    let periodic = NurbsCurve::try_control_point_curve_with_closure(
        3,
        points.clone(),
        ControlPointCurveClosure::Smooth,
    )
    .unwrap();
    assert_eq!(
        periodic
            .control_polygon(Tolerance::DEFAULT)
            .unwrap()
            .vertices(),
        &[
            points[0], points[1], points[2], points[3], points[4], points[0]
        ]
    );

    let degree_two = NurbsCurve::try_new(
        2,
        vec![points[0], points[1], points[2], points[0], points[1]],
        vec![-2.0, -1.0, 0.0, 1.0, 2.0, 3.0, 4.0, 5.0],
    )
    .unwrap();
    assert_eq!(
        degree_two
            .control_polygon(Tolerance::DEFAULT)
            .unwrap()
            .vertices(),
        &[points[1], points[2], points[0], points[1]]
    );

    let repeated = NurbsCurve::try_new(
        3,
        vec![points[0], points[0], points[2], points[3], points[4]],
        vec![0.0, 0.0, 0.0, 0.0, 0.5, 1.0, 1.0, 1.0, 1.0],
    )
    .unwrap();
    assert_eq!(
        repeated.control_polygon(Tolerance::DEFAULT),
        Err(GeometryError::DegeneratePolylineSegment { segment: 0 })
    );
}

#[test]
fn control_point_insertion_matches_rhino_greville_rules() {
    let controls = [
        [0.0, 0.0, 0.0],
        [2.0, 4.0, 1.0],
        [5.0, -1.0, 2.0],
        [7.0, 3.0, -1.0],
        [9.0, 1.0, 0.0],
        [12.0, 5.0, 2.0],
        [15.0, -2.0, 1.0],
    ]
    .into_iter()
    .map(|point| Point3::try_from(point).unwrap())
    .collect::<Vec<_>>();
    let cubic = NurbsCurve::try_new(
        3,
        controls.clone(),
        vec![0.0, 0.0, 0.0, 0.0, 1.0, 3.0, 5.0, 8.0, 8.0, 8.0, 8.0],
    )
    .unwrap();

    let inserted = cubic.try_insert_control_point(2.25, false).unwrap();
    assert_eq!(inserted.degree(), 3);
    assert_eq!(
        inserted.knots(),
        &[0.0, 0.0, 0.0, 0.0, 1.0, 2.25, 3.0, 5.0, 8.0, 8.0, 8.0, 8.0,]
    );
    assert_point_near(
        inserted.control_points()[3].point(),
        Point3::try_new(6.1, 1.2, 0.35).unwrap(),
    );
    assert_eq!(inserted.control_points()[3].weight(), 1.0);
    assert_eq!(inserted.control_points()[4].point(), controls[3]);

    let midpoint = cubic.try_insert_control_point(2.25, true).unwrap();
    assert_eq!(
        midpoint.knots(),
        &[
            0.0,
            0.0,
            0.0,
            0.0,
            1.0,
            13.0 / 6.0,
            3.0,
            5.0,
            8.0,
            8.0,
            8.0,
            8.0,
        ]
    );
    assert_point_near(
        midpoint.control_points()[3].point(),
        Point3::try_new(6.0, 1.0, 0.5).unwrap(),
    );

    assert!(matches!(
        cubic.try_insert_control_point(0.0, false),
        Err(GeometryError::NoControlPointInsertionInterval { .. })
    ));
    let extended = cubic.try_insert_control_point(0.0, true).unwrap();
    assert_eq!(extended.knots()[4], 1.0 / 6.0);
    assert_point_near(
        extended.control_points()[1].point(),
        Point3::try_new(1.0, 2.0, 0.5).unwrap(),
    );

    let quadratic = NurbsCurve::try_new(
        2,
        [
            [0.0, 0.0, 0.0],
            [2.0, 4.0, 1.0],
            [5.0, -1.0, 2.0],
            [8.0, 3.0, -1.0],
            [11.0, 1.0, 0.0],
            [14.0, 5.0, 2.0],
            [17.0, -2.0, 1.0],
            [20.0, 2.0, -2.0],
        ]
        .into_iter()
        .map(|point| Point3::try_from(point).unwrap())
        .collect(),
        vec![0.0, 0.0, 0.0, 1.0, 2.0, 4.0, 7.0, 11.0, 14.0, 14.0, 14.0],
    )
    .unwrap();
    let inserted = quadratic.try_insert_control_point(3.0, false).unwrap();
    assert_eq!(
        inserted.control_points()[3].point(),
        quadratic.control_points()[3].point()
    );
    assert_eq!(
        inserted.control_points()[4].point(),
        quadratic.control_points()[3].point()
    );
    assert_eq!(
        inserted.knots(),
        &[
            0.0, 0.0, 0.0, 1.0, 2.0, 3.0, 4.0, 7.0, 11.0, 14.0, 14.0, 14.0,
        ]
    );
    let midpoint = quadratic.try_insert_control_point(3.0, true).unwrap();
    assert_eq!(midpoint.knots()[5], 2.25);
    assert_point_near(
        midpoint.control_points()[3].point(),
        Point3::try_new(6.5, 1.0, 0.5).unwrap(),
    );
}

#[test]
fn command_control_insertion_retains_spacing_and_nonuniform_charts() {
    let controls = [[0., 0., 0.], [1., 3., 0.], [3., -1., 0.], [5., 0., 0.]]
        .into_iter()
        .map(|p| Point3::try_from(p).unwrap())
        .collect();
    let source = NurbsCurve::try_clamped_uniform(3, controls)
        .unwrap()
        .try_reparameterized(-3.0..=17.0)
        .unwrap();
    let inserted = source.try_insert_command_control_point(5., false).unwrap();
    assert_eq!(inserted.domain(), 0.0..=40.0);
    assert_eq!(inserted.knots(), &[0., 0., 0., 0., 20., 40., 40., 40., 40.]);
    assert_point_near(
        inserted.control_points()[2].point(),
        Point3::try_new(1.4, 2.2, 0.).unwrap(),
    );
    let end = source.try_insert_command_control_point(-3., false).unwrap();
    assert_eq!(
        end.control_points()[0].point(),
        end.control_points()[1].point()
    );
    let nonuniform = source.try_insert_knot(1., 1).unwrap();
    let expected = nonuniform.try_insert_control_point(9., false).unwrap();
    assert_eq!(
        nonuniform
            .try_insert_command_control_point(9., false)
            .unwrap(),
        expected
    );
    let origin = 1e15;
    let translated = NurbsCurve::try_new(
        3,
        (0..6)
            .map(|i| Point3::try_new(i as Real, (i % 2) as Real, 0.).unwrap())
            .collect(),
        vec![
            origin,
            origin,
            origin,
            origin,
            origin + 1.,
            origin + 3.,
            origin + 4.,
            origin + 4.,
            origin + 4.,
            origin + 4.,
        ],
    )
    .unwrap();
    assert!(translated.command_uniform_step().is_none());
    assert_eq!(
        translated
            .try_insert_command_control_point(origin + 2., false)
            .unwrap(),
        translated
            .try_insert_control_point(origin + 2., false)
            .unwrap()
    );
}

#[test]
fn control_point_insertion_handles_rational_and_periodic_curves() {
    let rational = NurbsCurve::try_new_rational(
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
    .unwrap()
    .try_insert_control_point(2.0, false)
    .unwrap();
    assert_eq!(
        rational.knots(),
        &[-2.0, -2.0, -2.0, 1.0, 1.0, 2.0, 6.0, 6.0, 6.0]
    );
    assert_point_near(
        rational.control_points()[3].point(),
        Point3::try_new(7.2, 0.4, -0.4).unwrap(),
    );
    assert_eq!(rational.control_points()[3].weight(), 1.0);
    assert_eq!(
        rational
            .control_points()
            .iter()
            .map(|control| control.weight())
            .collect::<Vec<_>>(),
        vec![0.7, 1.6, 0.8, 1.0, 1.3, 0.9]
    );

    let unique = [
        point(0.0, 0.0),
        point(2.0, 0.0),
        point(2.0, 2.0),
        point(0.0, 2.0),
    ];
    let periodic = NurbsCurve::try_new(
        3,
        unique.iter().chain(&unique[..3]).copied().collect(),
        vec![0.0, 0.0, 1.0, 2.0, 3.0, 4.0, 5.0, 6.0, 7.0, 8.0, 8.0],
    )
    .unwrap()
    .try_insert_control_point(3.4, false)
    .unwrap();
    assert!(periodic.is_periodic());
    assert_eq!(
        periodic.knots(),
        &[
            -2.0, -2.0, -1.0, 0.0, 1.0, 2.0, 3.0, 4.0, 5.0, 6.0, 7.0, 7.0,
        ]
    );
    let expected_unique = [unique[0], unique[1], unique[2], point(1.2, 2.0), unique[3]];
    for (actual, expected) in periodic
        .control_points()
        .iter()
        .zip(expected_unique.iter().chain(&expected_unique[..3]))
    {
        assert_point_near(actual.point(), *expected);
    }
}

#[test]
fn control_point_removal_matches_rhino_knot_and_degree_rules() {
    let cubic_controls = [
        point(0.0, 0.0),
        point(2.0, 4.0),
        point(5.0, -1.0),
        point(7.0, 3.0),
        point(9.0, 1.0),
        point(12.0, 5.0),
        point(15.0, -2.0),
    ];
    let cubic = NurbsCurve::try_new(
        3,
        cubic_controls.to_vec(),
        vec![0.0, 0.0, 0.0, 0.0, 1.0, 3.0, 5.0, 8.0, 8.0, 8.0, 8.0],
    )
    .unwrap();
    for (index, expected_knots) in [
        vec![0.0, 0.0, 0.0, 0.0, 3.0, 5.0, 8.0, 8.0, 8.0, 8.0],
        vec![0.0, 0.0, 0.0, 0.0, 3.0, 5.0, 8.0, 8.0, 8.0, 8.0],
        vec![0.0, 0.0, 0.0, 0.0, 3.0, 5.0, 8.0, 8.0, 8.0, 8.0],
        vec![0.0, 0.0, 0.0, 0.0, 1.0, 5.0, 8.0, 8.0, 8.0, 8.0],
        vec![0.0, 0.0, 0.0, 0.0, 1.0, 3.0, 8.0, 8.0, 8.0, 8.0],
        vec![0.0, 0.0, 0.0, 0.0, 1.0, 3.0, 8.0, 8.0, 8.0, 8.0],
        vec![0.0, 0.0, 0.0, 0.0, 1.0, 3.0, 8.0, 8.0, 8.0, 8.0],
    ]
    .into_iter()
    .enumerate()
    {
        let removed = cubic.try_remove_control_point(index).unwrap();
        assert_eq!(removed.knots(), expected_knots);
        let expected_controls = cubic_controls
            .iter()
            .enumerate()
            .filter_map(|(control, point)| (control != index).then_some(*point))
            .collect::<Vec<_>>();
        assert_eq!(
            removed
                .control_points()
                .iter()
                .map(|control| control.point())
                .collect::<Vec<_>>(),
            expected_controls
        );
    }

    let quadratic = NurbsCurve::try_new(
        2,
        (0..8).map(|index| point(index as Real, 0.0)).collect(),
        vec![0.0, 0.0, 0.0, 1.0, 2.0, 4.0, 7.0, 11.0, 14.0, 14.0, 14.0],
    )
    .unwrap();
    assert_eq!(
        quadratic.try_remove_control_point(3).unwrap().knots(),
        &[0.0, 0.0, 0.0, 1.5, 4.0, 7.0, 11.0, 14.0, 14.0, 14.0]
    );
    assert_eq!(
        quadratic.try_remove_control_point(5).unwrap().knots(),
        &[0.0, 0.0, 0.0, 1.0, 2.0, 5.5, 11.0, 14.0, 14.0, 14.0]
    );

    let bezier = NurbsCurve::try_new(
        3,
        cubic_controls[..4].to_vec(),
        vec![0.0, 0.0, 0.0, 0.0, 7.0, 7.0, 7.0, 7.0],
    )
    .unwrap()
    .try_remove_control_point(1)
    .unwrap();
    assert_eq!(bezier.degree(), 2);
    assert_eq!(bezier.knots(), &[0.0, 0.0, 0.0, 7.0, 7.0, 7.0]);
    assert_eq!(
        bezier
            .control_points()
            .iter()
            .map(|control| control.point())
            .collect::<Vec<_>>(),
        vec![cubic_controls[0], cubic_controls[2], cubic_controls[3]]
    );
}

#[test]
fn control_point_removal_handles_rational_and_periodic_curves() {
    let rational = NurbsCurve::try_new_rational(
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
    .unwrap()
    .try_remove_control_point(2)
    .unwrap();
    assert_eq!(rational.knots(), &[-2.0, -2.0, -2.0, 1.0, 6.0, 6.0, 6.0]);
    assert_eq!(
        rational
            .control_points()
            .iter()
            .map(|control| control.weight())
            .collect::<Vec<_>>(),
        vec![1.0, 1.6, 1.3, 1.0]
    );

    let unique = [
        point(0.0, 0.0),
        point(2.0, 0.0),
        point(2.0, 2.0),
        point(0.0, 2.0),
    ];
    let periodic = NurbsCurve::try_new(
        3,
        unique.iter().chain(&unique[..3]).copied().collect(),
        vec![0.0, 0.0, 1.0, 2.0, 3.0, 4.0, 5.0, 6.0, 7.0, 8.0, 8.0],
    )
    .unwrap()
    .try_remove_control_point(1)
    .unwrap();
    assert!(periodic.is_periodic());
    assert_eq!(
        periodic.knots(),
        &[-2.0, -2.0, -1.0, 0.0, 1.0, 2.0, 3.0, 4.0, 5.0, 5.0]
    );
    let remaining = [unique[0], unique[2], unique[3]];
    assert_eq!(
        periodic
            .control_points()
            .iter()
            .map(|control| control.point())
            .collect::<Vec<_>>(),
        remaining
            .iter()
            .chain(&remaining)
            .copied()
            .collect::<Vec<_>>()
    );

    let minimal_quadratic = NurbsCurve::try_new(
        2,
        unique[..3].iter().chain(&unique[..2]).copied().collect(),
        vec![0.0, 0.0, 1.0, 2.0, 3.0, 4.0, 5.0, 5.0],
    )
    .unwrap()
    .try_remove_control_point(1)
    .unwrap();
    assert_eq!(minimal_quadratic.degree(), 2);
    assert!(minimal_quadratic.is_periodic());
    assert_eq!(
        minimal_quadratic.knots(),
        &[-1.0, -1.0, 0.0, 1.0, 2.0, 3.0, 4.0, 4.0]
    );
    assert_eq!(
        minimal_quadratic
            .control_points()
            .iter()
            .map(|control| control.point())
            .collect::<Vec<_>>(),
        unique[..3]
            .iter()
            .chain(&unique[..2])
            .copied()
            .collect::<Vec<_>>()
    );

    let minimal_quartic = NurbsCurve::try_new(
        4,
        unique.iter().chain(&unique).copied().collect(),
        vec![
            -3.0, -3.0, -2.0, -1.0, 0.0, 1.0, 2.0, 3.0, 4.0, 5.0, 6.0, 7.0, 7.0,
        ],
    )
    .unwrap()
    .try_remove_control_point(1)
    .unwrap();
    assert_eq!(minimal_quartic.degree(), 3);
    assert!(minimal_quartic.is_periodic());
    assert_eq!(
        minimal_quartic.knots(),
        &[-2.0, -2.0, -1.0, 0.0, 1.0, 2.0, 3.0, 4.0, 5.0, 5.0]
    );
    let remaining = [unique[0], unique[2], unique[3]];
    assert_eq!(
        minimal_quartic
            .control_points()
            .iter()
            .map(|control| control.point())
            .collect::<Vec<_>>(),
        remaining
            .iter()
            .chain(&remaining)
            .copied()
            .collect::<Vec<_>>()
    );

    assert!(matches!(
        periodic.try_remove_control_point(3),
        Err(GeometryError::ControlPointIndexOutOfRange { .. })
    ));
    let line = NurbsCurve::try_new(
        1,
        vec![point(0.0, 0.0), point(1.0, 0.0)],
        vec![0.0, 0.0, 1.0, 1.0],
    )
    .unwrap();
    assert!(matches!(
        line.try_remove_control_point(0),
        Err(GeometryError::InsufficientControlPoints { .. })
    ));
}
