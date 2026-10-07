use super::*;
use crate::{Circle3, LineSegment, Point3, Polyline3, Vector3};

fn p(x: Real, y: Real) -> Point3 {
    Point3::try_new(x, y, 0.).unwrap()
}
fn tol() -> Tolerance {
    Tolerance::try_new(1e-12, 1e-12, 1e-10).unwrap()
}

#[test]
fn signed_length_subcurves_preserve_analytic_lines_and_native_endpoints() {
    let c = Curve3::Line(
        LineSegment::try_new(p(0., 0.), p(10., 0.), tol())
            .unwrap()
            .try_reparameterized(20.0..=30.0)
            .unwrap(),
    );
    for (anchor, length, start, end) in [
        (24., 2., p(4., 0.), p(6., 0.)),
        (24., -2., p(4., 0.), p(2., 0.)),
        (24., 6., p(4., 0.), p(10., 0.)),
    ] {
        let result = c
            .try_subcurve_at_arc_length(anchor, length, tol())
            .unwrap()
            .unwrap();
        assert!(matches!(result, Curve3::Line(_)));
        assert!(
            result
                .as_ref()
                .start_point()
                .unwrap()
                .distance_to(start)
                .unwrap()
                < 1e-12
        );
        assert!(
            result
                .as_ref()
                .end_point()
                .unwrap()
                .distance_to(end)
                .unwrap()
                < 1e-12
        );
    }
    for (anchor, length) in [(24., 7.), (24., -5.), (30., 1.), (20., -1.)] {
        assert!(
            c.try_subcurve_at_arc_length(anchor, length, tol())
                .unwrap()
                .is_none()
        );
    }
    for (anchor, length) in [(19., 1.), (24., 0.), (24., Real::NAN), (Real::INFINITY, 1.)] {
        assert!(c.try_subcurve_at_arc_length(anchor, length, tol()).is_err());
    }
}

#[test]
fn closed_signed_length_subcurves_cross_seams_and_limit_one_traversal() {
    let c = Curve3::Polyline(
        Polyline3::try_with_parameters(
            vec![p(0., 0.), p(4., 0.), p(4., 6.), p(0., 6.), p(0., 0.)],
            vec![0., 1., 2., 3., 4.],
            tol(),
        )
        .unwrap(),
    );
    for (length, end) in [
        (8., p(3.2, 0.)),
        (-8., p(4., 3.2)),
        (20., p(0., 4.8)),
        (-20., p(0., 4.8)),
    ] {
        let result = c
            .try_subcurve_at_arc_length(3.2, length, tol())
            .unwrap()
            .unwrap();
        assert!(
            result
                .as_ref()
                .start_point()
                .unwrap()
                .distance_to(p(0., 4.8))
                .unwrap()
                < 1e-12
        );
        assert!(
            result
                .as_ref()
                .end_point()
                .unwrap()
                .distance_to(end)
                .unwrap()
                < 1e-12
        );
    }
    assert!(
        c.try_subcurve_at_arc_length(3.2, 20.1, tol())
            .unwrap()
            .is_none()
    );
}

#[test]
fn circle_length_subcurves_keep_analytic_arcs_and_independent_angle_witnesses() {
    let c = Curve3::Circle(
        Circle3::try_new(
            p(0., 0.),
            2.,
            Vector3::try_new(0., 0., 1.)
                .unwrap()
                .normalized_nonzero()
                .unwrap(),
            tol(),
        )
        .unwrap()
        .try_reparameterized(10.0..=18.0)
        .unwrap(),
    );
    let anchor = 17.;
    let angle = std::f64::consts::TAU * 7. / 8.;
    for length in [3., -3.] {
        let result = c
            .try_subcurve_at_arc_length(anchor, length, tol())
            .unwrap()
            .unwrap();
        assert!(matches!(result, Curve3::Arc(_)));
        let theta = angle + length / 2.;
        assert!(
            result
                .as_ref()
                .end_point()
                .unwrap()
                .distance_to(p(2. * theta.cos(), 2. * theta.sin()))
                .unwrap()
                < 1e-11
        );
    }
}

#[test]
fn subcurve_length_after_large_prefix_retains_short_local_spans() {
    let c = Curve3::NurbsCurve(
        crate::NurbsCurve::try_new(
            1,
            vec![p(0., 0.), p(1e16, 0.), p(1e16, 1.), p(1e16, 2.)],
            vec![0., 0., 1., 2., 3., 3.],
        )
        .unwrap(),
    );
    for (anchor, length, end) in [(1., 0.25, p(1e16, 0.25)), (2.5, -0.25, p(1e16, 1.25))] {
        let result = c
            .try_subcurve_at_arc_length(anchor, length, tol())
            .unwrap()
            .unwrap();
        assert_eq!(result.as_ref().end_point().unwrap(), end);
    }
}
