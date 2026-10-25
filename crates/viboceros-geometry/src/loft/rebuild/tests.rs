use super::*;
use crate::{Circle3, Point3, UnitVector3};

fn circle(z: f64) -> NurbsCurve {
    Circle3::try_new(
        Point3::try_new(0., 0., z).unwrap(),
        1.,
        UnitVector3::try_new(0., 0., 1., Tolerance::DEFAULT).unwrap(),
        Tolerance::DEFAULT,
    )
    .unwrap()
    .to_nurbs()
    .unwrap()
}

#[test]
fn closed_rebuild_interpolates_exact_circle_stations_on_a_clamped_basis() {
    let source = circle(0.);
    let c =
        try_rebuild_open_curve(CurveRef::NurbsCurve(&source), 5, 3, Tolerance::DEFAULT).unwrap();
    assert_eq!(c.degree(), 3);
    assert_eq!(c.control_points().len(), 5);
    assert_eq!(c.knots(), [0., 0., 0., 0., 1., 2., 2., 2., 2.]);
    let controls = c.control_points();
    for (v, expected) in [
        (0, [1., 0., 0.]),
        (1, [25. / 24., 9. * 3_f64.sqrt() / 10., 0.]),
        (2, [-73. / 24., 0., 0.]),
        (3, [25. / 24., -9. * 3_f64.sqrt() / 10., 0.]),
        (4, [1., 0., 0.]),
    ] {
        for (a, b) in controls[v].point().to_array().into_iter().zip(expected) {
            assert!((a - b).abs() < 1e-10, "{a} != {b}");
        }
    }
    for t in [0., 1. / 3., 1., 5. / 3., 2.] {
        let theta = std::f64::consts::TAU * t / 2.;
        let p = c.evaluate(t).unwrap();
        assert!((p.x() - theta.cos()).abs() < 1e-10);
        assert!((p.y() - theta.sin()).abs() < 1e-10);
    }
}

#[test]
fn disabled_and_undersized_closed_requests_preserve_the_original_tensor() {
    let curves = [circle(0.), circle(2.), circle(5.)];
    let expected = try_loft_nurbs_curves(&curves, LoftStyle::Normal, false).unwrap();
    for count in [0, 1, 2] {
        let actual = try_loft_nurbs_curves_rebuilt(
            &curves,
            LoftStyle::Normal,
            false,
            LoftEndPoints::default(),
            count,
            Tolerance::DEFAULT,
        )
        .unwrap();
        assert_eq!(actual, expected);
    }
}

#[test]
fn closed_sections_request_one_extra_seam_control_and_no_periodic_constraints() {
    let curves = [circle(0.), circle(3.)];
    let before = curves.clone();
    for n in [3, 4, 6, 31] {
        let mut calls = 0;
        let sections = try_prepare_rebuilt_loft_sections_with(
            &curves,
            LoftStyle::Straight,
            false,
            LoftEndPoints::default(),
            n,
            Tolerance::DEFAULT,
            &mut |c, count, degree, tol| -> Result<_, GeometryError> {
                calls += 1;
                assert_eq!(count, n + 1);
                assert_eq!(degree, (n - 1).min(3));
                crate::try_rebuild_open_curve(CurveRef::NurbsCurve(c), count, degree, tol)
            },
        )
        .unwrap();
        assert_eq!(calls, 2);
        assert!(
            sections
                .iter()
                .all(|c| c.control_points().first().unwrap().point()
                    == c.control_points().last().unwrap().point())
        );
    }
    assert_eq!(curves, before);
}

#[test]
fn invalid_requests_never_reach_the_rebuild_backend() {
    let curve = circle(0.);
    for (curves, style, closed, ends, n) in [
        (
            vec![],
            LoftStyle::Normal,
            false,
            LoftEndPoints::default(),
            6,
        ),
        (
            vec![curve.clone()],
            LoftStyle::Normal,
            false,
            LoftEndPoints::default(),
            6,
        ),
        (
            vec![curve.clone(); 257],
            LoftStyle::Normal,
            false,
            LoftEndPoints::default(),
            6,
        ),
        (
            vec![curve.clone(); 3],
            LoftStyle::Normal,
            true,
            LoftEndPoints {
                start: Some(Point3::try_new(0., 0., 0.).unwrap()),
                end: None,
            },
            6,
        ),
        (
            vec![curve.clone(); 2],
            LoftStyle::Uniform,
            false,
            LoftEndPoints {
                start: Some(Point3::try_new(0., 0., 0.).unwrap()),
                end: None,
            },
            6,
        ),
        (
            vec![curve.clone(); 2],
            LoftStyle::Normal,
            false,
            LoftEndPoints::default(),
            513,
        ),
        (
            vec![curve.clone(); 2],
            LoftStyle::Normal,
            false,
            LoftEndPoints::default(),
            512,
        ),
    ] {
        let result = try_prepare_rebuilt_loft_sections_with(
            &curves,
            style,
            closed,
            ends,
            n,
            Tolerance::DEFAULT,
            &mut |_, _, _, _| -> Result<NurbsCurve, GeometryError> {
                panic!("invalid request reached the backend")
            },
        );
        assert!(result.is_err());
    }
}

#[test]
fn malformed_backend_layouts_are_rejected_without_mutating_sources() {
    let curves = [circle(0.), circle(2.)];
    let before = curves.clone();
    assert!(
        try_prepare_rebuilt_loft_sections_with(
            &curves,
            LoftStyle::Normal,
            false,
            LoftEndPoints::default(),
            6,
            Tolerance::DEFAULT,
            &mut |c, _, _, _| -> Result<_, GeometryError> { Ok(c.clone()) }
        )
        .is_err()
    );
    assert_eq!(curves, before);
}

#[test]
fn rebuilt_polygon_straight_lofts_use_the_constructed_cubic_chart() {
    let curves = [0., 2., 4.].map(|z| {
        let p = [[-1., -1.], [1., -1.], [1., 1.], [-1., 1.], [-1., -1.]];
        NurbsCurve::try_new(
            1,
            p.into_iter()
                .map(|p| Point3::try_new(p[0], p[1], z).unwrap())
                .collect(),
            vec![0., 0., 1., 2., 3., 4., 4.],
        )
        .unwrap()
    });
    let b = crate::Brep::try_loft_with_surface(
        &curves,
        LoftStyle::Straight,
        false,
        Tolerance::DEFAULT,
        &mut |c, s, closed| {
            try_loft_nurbs_curves_rebuilt(
                c,
                s,
                closed,
                LoftEndPoints::default(),
                6,
                Tolerance::DEFAULT,
            )
        },
    )
    .unwrap();
    for face in b.faces() {
        let s = face.surface();
        assert_eq!(s.degree_v(), 3);
        let u = s.parameter_at_u(0.45).unwrap();
        let v = s.parameter_at_v(0.37).unwrap();
        let p = s.evaluate(u, v).unwrap();
        let normal = s.normal_at(u, v).unwrap().as_vector();
        assert!(normal.x() * p.x() + normal.y() * p.y() > 0.);
    }
}
