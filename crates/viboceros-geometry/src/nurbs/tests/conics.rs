use super::*;

#[test]
fn conic_matches_rhino_rho_weight_and_normalized_layout() {
    let start = point(0.0, 0.0);
    let apex = point(5.0, 5.0);
    let end = point(10.0, 0.0);

    for (rho, expected_weight, rational) in [
        (0.25, 1.0 / 3.0, true),
        (0.5, 1.0, false),
        (0.75, 3.0, true),
    ] {
        let curve = NurbsCurve::try_conic(start, apex, end, rho).unwrap();
        assert_eq!(curve.degree(), 2);
        assert_eq!(curve.is_rational(), rational);
        assert_eq!(curve.domain(), 0.0..=1.0);
        assert_eq!(curve.knots(), &[0.0, 0.0, 0.0, 1.0, 1.0, 1.0]);
        assert_eq!(curve.control_points()[0].point(), start);
        assert_eq!(curve.control_points()[0].weight(), 1.0);
        assert_eq!(curve.control_points()[1].point(), apex);
        assert_eq!(curve.control_points()[1].weight(), expected_weight);
        assert_eq!(curve.control_points()[2].point(), end);
        assert_eq!(curve.control_points()[2].weight(), 1.0);
    }
}

#[test]
fn conic_through_point_projects_to_plane_and_recovers_weight() {
    let start = point(0.0, 0.0);
    let apex = point(5.0, 5.0);
    let end = point(10.0, 0.0);
    let through = Point3::try_new(5.0, 2.0, 37.0).unwrap();
    let curve =
        NurbsCurve::try_conic_through_point(start, apex, end, through, Tolerance::DEFAULT).unwrap();
    assert!((curve.control_points()[1].weight() - 2.0 / 3.0).abs() < 1.0e-15);
    assert_point_near(curve.evaluate(0.5).unwrap(), point(5.0, 2.0));

    let start = Point3::try_new(1.0, 2.0, 3.0).unwrap();
    let apex = Point3::try_new(4.0, 9.0, 7.0).unwrap();
    let end = Point3::try_new(9.0, 4.0, 5.0).unwrap();
    let through = Point3::try_new(
        3.666_666_666_666_666_5,
        5.047_619_047_619_047_4,
        4.904_761_904_761_905,
    )
    .unwrap();
    let curve =
        NurbsCurve::try_conic_through_point(start, apex, end, through, Tolerance::DEFAULT).unwrap();
    assert!((curve.control_points()[1].weight() - 2.0 / 3.0).abs() < 1.0e-14);
    assert_point_near(curve.evaluate(0.4).unwrap(), through);

    let closed_cusp = NurbsCurve::try_conic_through_point(
        point(0.0, 0.0),
        point(5.0, 5.0),
        point(0.0, 0.0),
        point(2.0, 2.0),
        Tolerance::DEFAULT,
    )
    .unwrap();
    assert!((closed_cusp.control_points()[1].weight() - 2.0 / 3.0).abs() < 1.0e-15);
    assert_point_near(closed_cusp.evaluate(0.5).unwrap(), point(2.0, 2.0));
}

#[test]
fn conic_accepts_degenerate_rho_controls_and_rejects_invalid_through_points() {
    let start = point(0.0, 0.0);
    let apex = point(5.0, 5.0);
    let end = point(10.0, 0.0);
    for rho in [-1.0, 0.0, 1.0, Real::INFINITY, Real::NAN] {
        assert!(NurbsCurve::try_conic(start, apex, end, rho).is_err());
    }
    let collinear = NurbsCurve::try_conic(start, point(5.0, 0.0), end, 0.5).unwrap();
    assert_eq!(collinear.control_points()[1].point(), point(5.0, 0.0));
    let coincident = NurbsCurve::try_conic(start, start, start, 0.5).unwrap();
    assert!(
        coincident
            .control_points()
            .iter()
            .all(|control| control.point() == start)
    );
    for through in [
        point(5.0, 0.0),
        point(5.0, 5.0),
        point(5.0, 6.0),
        point(-1.0, 1.0),
    ] {
        assert!(
            NurbsCurve::try_conic_through_point(start, apex, end, through, Tolerance::DEFAULT,)
                .is_err()
        );
    }
}

#[test]
fn parabola_matches_rhino_full_and_half_quadratic_layouts() {
    let frame = Frame3::try_from_directions(
        Point3::try_new(1.0, 2.0, 3.0).unwrap(),
        Vector3::try_new(0.0, 1.0, 0.0).unwrap(),
        Vector3::try_new(-1.0, 0.0, 0.0).unwrap(),
        Tolerance::DEFAULT,
    )
    .unwrap();

    let full = NurbsCurve::try_parabola(frame, 2.0, 1.0, false).unwrap();
    assert_eq!(full.degree(), 2);
    assert!(!full.is_rational());
    assert_eq!(full.domain(), 0.0..=1.0);
    assert_eq!(full.knots(), &[0.0, 0.0, 0.0, 1.0, 1.0, 1.0]);
    assert_eq!(
        full.control_points()
            .iter()
            .map(|control| control.point())
            .collect::<Vec<_>>(),
        vec![
            Point3::try_new(1.0, 0.0, 4.0).unwrap(),
            Point3::try_new(1.0, 2.0, 2.0).unwrap(),
            Point3::try_new(1.0, 4.0, 4.0).unwrap(),
        ]
    );
    assert_eq!(full.evaluate(0.5).unwrap(), frame.origin());

    let half = NurbsCurve::try_parabola(frame, 2.0, 1.0, true).unwrap();
    assert_eq!(
        half.control_points()
            .iter()
            .map(|control| control.point())
            .collect::<Vec<_>>(),
        vec![
            Point3::try_new(1.0, 2.0, 3.0).unwrap(),
            Point3::try_new(1.0, 3.0, 3.0).unwrap(),
            Point3::try_new(1.0, 4.0, 4.0).unwrap(),
        ]
    );
    for parameter in [0.0, 0.125, 0.5, 1.0] {
        assert_point_near(
            half.evaluate(parameter).unwrap(),
            Point3::try_new(1.0, 2.0 + 2.0 * parameter, 3.0 + parameter * parameter).unwrap(),
        );
    }

    assert!(matches!(
        NurbsCurve::try_parabola(frame, 0.0, 1.0, false),
        Err(GeometryError::Degenerate {
            context: "parabola"
        })
    ));
    assert!(matches!(
        NurbsCurve::try_parabola(frame, 1.0, -1.0, true),
        Err(GeometryError::Degenerate {
            context: "parabola"
        })
    ));
    assert!(matches!(
        NurbsCurve::try_parabola(frame, f64::INFINITY, 1.0, false),
        Err(GeometryError::NonFinite { .. })
    ));
}

#[test]
fn three_point_vertex_parabola_matches_rhino_and_reverses_exactly() {
    let vertex = Point3::try_new(1.0, 2.0, 3.0).unwrap();
    let start = Point3::try_new(-2.0, 5.0, 7.0).unwrap();
    let end = Point3::try_new(8.0, 4.0, 6.0).unwrap();
    let curve =
        NurbsCurve::try_parabola_from_vertex(vertex, start, end, Tolerance::DEFAULT).unwrap();
    assert_eq!(curve.degree(), 2);
    assert!(!curve.is_rational());
    assert_eq!(curve.domain(), 0.0..=1.0);
    assert_eq!(curve.knots(), &[0.0, 0.0, 0.0, 1.0, 1.0, 1.0]);
    assert_eq!(curve.control_points()[0].point(), start);
    assert_point_near(
        curve.control_points()[1].point(),
        Point3::try_new(
            -0.011_269_980_002_004_854,
            -0.655_652_364_729_298_2,
            -0.676_681_758_332_815_5,
        )
        .unwrap(),
    );
    assert_eq!(curve.control_points()[2].point(), end);
    assert_point_near(curve.evaluate(0.448_996_842_378_081_96).unwrap(), vertex);

    let reversed =
        NurbsCurve::try_parabola_from_vertex(vertex, end, start, Tolerance::DEFAULT).unwrap();
    for (actual, expected) in reversed
        .control_points()
        .iter()
        .zip(curve.control_points().iter().rev())
    {
        assert_point_near(actual.point(), expected.point());
    }
}

#[test]
fn three_point_focus_parabola_matches_rhino_and_recovers_focus() {
    let focus = Point3::try_new(1.0, 2.0, 3.0).unwrap();
    let start = Point3::try_new(-2.0, 5.0, 7.0).unwrap();
    let end = Point3::try_new(8.0, 4.0, 6.0).unwrap();
    let curve = NurbsCurve::try_parabola_from_focus(focus, start, end, Tolerance::DEFAULT).unwrap();
    assert_eq!(curve.control_points()[0].point(), start);
    assert_point_near(
        curve.control_points()[1].point(),
        Point3::try_new(
            -0.856_025_073_925_939_4,
            -1.808_620_322_768_226_7,
            -2.287_962_111_716_675_3,
        )
        .unwrap(),
    );
    assert_eq!(curve.control_points()[2].point(), end);
    assert_point_near(curve.try_parabola_focus(Tolerance::DEFAULT).unwrap(), focus);

    let reversed =
        NurbsCurve::try_parabola_from_focus(focus, end, start, Tolerance::DEFAULT).unwrap();
    for (actual, expected) in reversed
        .control_points()
        .iter()
        .zip(curve.control_points().iter().rev())
    {
        assert_point_near(actual.point(), expected.point());
    }

    let asymmetric = NurbsCurve::try_parabola_from_focus(
        Point3::try_new(0.0, 0.0, 1.0).unwrap(),
        Point3::try_new(-1.0, 0.0, 0.25).unwrap(),
        Point3::try_new(3.0, 0.0, 2.25).unwrap(),
        Tolerance::DEFAULT,
    )
    .unwrap();
    assert_point_near(
        asymmetric.control_points()[1].point(),
        Point3::try_new(-1.0, 0.0, 2.75).unwrap(),
    );
}

#[test]
fn through_point_parabola_honors_the_opening_direction() {
    let start = Point3::try_new(-1.0, 0.0, 0.25).unwrap();
    let through = Point3::try_new(1.0, 0.0, 0.25).unwrap();
    let end = Point3::try_new(3.0, 0.0, 2.25).unwrap();
    let vertical = NurbsCurve::try_parabola_through_point(
        start,
        through,
        end,
        Vector3::try_new(0.0, 0.0, 1.0).unwrap(),
        Tolerance::DEFAULT,
    )
    .unwrap();
    assert_point_near(
        vertical.control_points()[1].point(),
        Point3::try_new(1.0, 0.0, -0.75).unwrap(),
    );
    assert_point_near(vertical.evaluate(0.5).unwrap(), through);
    assert_point_near(
        vertical.try_parabola_focus(Tolerance::DEFAULT).unwrap(),
        Point3::try_new(0.0, 0.0, 1.0).unwrap(),
    );

    let oblique = NurbsCurve::try_parabola_through_point(
        start,
        through,
        end,
        Vector3::try_new(-1.0, 0.0, 0.75).unwrap(),
        Tolerance::DEFAULT,
    )
    .unwrap();
    assert_point_near(
        oblique.control_points()[1].point(),
        Point3::try_new(2.904_761_904_761_904_7, 0.0, -0.178_571_428_571_428_66).unwrap(),
    );
    assert_point_near(oblique.evaluate(0.3).unwrap(), through);
}

#[test]
fn three_point_parabola_rejects_degenerate_constraints() {
    let origin = Point3::try_new(0.0, 0.0, 0.0).unwrap();
    let x = Point3::try_new(1.0, 0.0, 0.0).unwrap();
    let negative_x = Point3::try_new(-1.0, 0.0, 0.0).unwrap();
    assert!(NurbsCurve::try_parabola_from_vertex(origin, origin, x, Tolerance::DEFAULT).is_err());
    assert!(
        NurbsCurve::try_parabola_from_vertex(origin, negative_x, x, Tolerance::DEFAULT).is_err()
    );
    assert!(
        NurbsCurve::try_parabola_from_focus(origin, negative_x, x, Tolerance::DEFAULT).is_err()
    );
    assert!(
        NurbsCurve::try_parabola_through_point(
            origin,
            x,
            Point3::try_new(2.0, 0.0, 0.0).unwrap(),
            Vector3::try_new(0.0, 0.0, 0.0).unwrap(),
            Tolerance::DEFAULT,
        )
        .is_err()
    );
    assert!(
        NurbsCurve::try_parabola_through_point(
            origin,
            Point3::try_new(1.0, 1.0, -1.0).unwrap(),
            Point3::try_new(2.0, 0.0, 0.0).unwrap(),
            Vector3::try_new(0.0, 0.0, 1.0).unwrap(),
            Tolerance::DEFAULT,
        )
        .is_err()
    );
    assert!(
        NurbsCurve::try_parabola_through_point(
            origin,
            Point3::try_new(3.0, 0.0, -1.0).unwrap(),
            Point3::try_new(2.0, 0.0, 0.0).unwrap(),
            Vector3::try_new(0.0, 0.0, 1.0).unwrap(),
            Tolerance::DEFAULT,
        )
        .is_err()
    );
}

#[test]
fn hyperbola_matches_rhino_rational_quadratic_layout() {
    let frame = Frame3::try_from_directions(
        Point3::try_new(0.0, 0.0, 0.0).unwrap(),
        Vector3::try_new(1.0, 0.0, 0.0).unwrap(),
        Vector3::try_new(0.0, 1.0, 0.0).unwrap(),
        Tolerance::DEFAULT,
    )
    .unwrap();
    let curve = NurbsCurve::try_hyperbola(frame, 3.0, 4.0, 3.75).unwrap();
    assert_eq!(curve.degree(), 2);
    assert!(curve.is_rational());
    assert_eq!(curve.domain(), 0.0..=1.0);
    assert_eq!(curve.knots(), &[0.0, 0.0, 0.0, 1.0, 1.0, 1.0]);
    assert_eq!(
        curve.control_points(),
        &[
            WeightedPoint3::try_new(Point3::try_new(3.75, -3.0, 0.0).unwrap(), 1.0).unwrap(),
            WeightedPoint3::try_new(Point3::try_new(2.4, 0.0, 0.0).unwrap(), 1.25).unwrap(),
            WeightedPoint3::try_new(Point3::try_new(3.75, 3.0, 0.0).unwrap(), 1.0).unwrap(),
        ]
    );
    assert_point_near(
        curve.evaluate(0.5).unwrap(),
        Point3::try_new(3.0, 0.0, 0.0).unwrap(),
    );

    for parameter in [0.0, 0.125, 0.5, 0.875, 1.0] {
        let point = curve.evaluate(parameter).unwrap();
        let equation = point.x() * point.x() / 9.0 - point.y() * point.y() / 16.0;
        assert!((equation - 1.0).abs() < 1.0e-12);
    }
}

#[test]
fn hyperbola_supports_arbitrary_frames_and_rejects_bad_dimensions() {
    let frame = Frame3::try_from_directions(
        Point3::try_new(1.0, 2.0, 3.0).unwrap(),
        Vector3::try_new(3.0, 4.0, 0.0).unwrap(),
        Vector3::try_new(-4.0, 3.0, 0.0).unwrap(),
        Tolerance::DEFAULT,
    )
    .unwrap();
    let curve = NurbsCurve::try_hyperbola(frame, 3.0, 4.0, 3.75).unwrap();
    assert_point_near(
        curve.control_points()[0].point(),
        Point3::try_new(5.65, 3.2, 3.0).unwrap(),
    );
    assert_point_near(
        curve.control_points()[1].point(),
        Point3::try_new(2.44, 3.92, 3.0).unwrap(),
    );
    assert_point_near(
        curve.control_points()[2].point(),
        Point3::try_new(0.85, 6.8, 3.0).unwrap(),
    );

    for dimensions in [
        [0.0, 4.0, 5.0],
        [3.0, 0.0, 5.0],
        [3.0, 4.0, 3.0],
        [3.0, 4.0, 2.0],
        [3.0, 4.0, Real::INFINITY],
    ] {
        assert!(
            NurbsCurve::try_hyperbola(frame, dimensions[0], dimensions[1], dimensions[2]).is_err()
        );
    }
}
