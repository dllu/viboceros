use super::*;
fn p(x: Real, y: Real) -> Point3 {
    Point3::try_new(x, y, 0.).unwrap()
}
#[test]
fn arc_and_rational_source_return_first_sphere_crossing_in_original_chart() {
    let arc = crate::CircularArc3::try_from_three_points(
        p(1., 0.),
        p(2_f64.sqrt() / 2., 2_f64.sqrt() / 2.),
        p(0., 1.),
        Tolerance::DEFAULT,
    )
    .unwrap();
    let curve = CurveRef::Arc(&arc);
    for from_end in [false, true] {
        let hit = curve
            .point_at_chord_distance(0.2, from_end, Tolerance::DEFAULT)
            .unwrap()
            .unwrap();
        let angle = 2. * 0.1_f64.asin();
        assert!(
            (hit.parameter
                - if from_end {
                    std::f64::consts::FRAC_PI_2 - angle
                } else {
                    angle
                })
            .abs()
                < 1e-12
        );
        assert!(
            (hit.point
                .distance_to(if from_end {
                    curve.end_point().unwrap()
                } else {
                    curve.start_point().unwrap()
                })
                .unwrap()
                - 0.2)
                .abs()
                < 1e-12
        );
    }
    let nurbs = NurbsCurve::try_new(
        2,
        vec![p(0., 0.), p(1., 0.), p(2., 1.)],
        vec![0., 0., 0., 1., 1., 1.],
    )
    .unwrap();
    let curve = CurveRef::NurbsCurve(&nurbs);
    let hit = curve
        .point_at_chord_distance(0.2, true, Tolerance::DEFAULT)
        .unwrap()
        .unwrap();
    assert!((hit.parameter - 0.9280053900349274).abs() < 1e-12);
    assert!(
        hit.point
            .distance_to(curve.evaluate(hit.parameter).unwrap())
            .unwrap()
            < 1e-12
    );
    assert!(
        curve
            .point_at_chord_distance(10., true, Tolerance::DEFAULT)
            .unwrap()
            .is_none()
    );
}
#[test]
fn a_single_small_query_has_no_whole_curve_station_limit_and_poles_still_reject() {
    let line = crate::LineSegment::try_new(p(0., 0.), p(1e9, 0.), Tolerance::DEFAULT).unwrap();
    let hit = CurveRef::Line(&line)
        .point_at_chord_distance(1., false, Tolerance::DEFAULT)
        .unwrap()
        .unwrap();
    assert_eq!(hit.point, p(1., 0.));
    let pole = NurbsCurve::try_new_rational(
        2,
        [1., -1., 1.]
            .into_iter()
            .enumerate()
            .map(|(i, w)| crate::WeightedPoint3::try_new(p(i as Real, 0.), w).unwrap())
            .collect(),
        vec![0., 0., 0., 1., 1., 1.],
    )
    .unwrap();
    for from_end in [false, true] {
        assert_eq!(
            CurveRef::NurbsCurve(&pole).point_at_chord_distance(
                1000.,
                from_end,
                Tolerance::DEFAULT
            ),
            Err(GeometryError::ZeroWeightAtParameter)
        );
    }
    for invalid in [0., -1., Real::NAN, Real::INFINITY] {
        assert!(
            CurveRef::Line(&line)
                .point_at_chord_distance(invalid, false, Tolerance::DEFAULT)
                .is_err()
        );
    }
}
