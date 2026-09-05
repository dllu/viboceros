use super::*;
use crate::{
    Circle3, CircularArc3, Curve3, CurveSegment3, Ellipse3, LineSegment, NurbsCurve, Point3,
    PolyCurve3, Polyline3, Tolerance, UnitVector3,
};
use std::f64::consts::PI;

fn point(x: Real, y: Real) -> Point3 {
    Point3::try_new(x, y, 0.0).unwrap()
}

fn axis(x: Real, y: Real, z: Real) -> UnitVector3 {
    UnitVector3::try_new(x, y, z, Tolerance::DEFAULT).unwrap()
}

fn circle() -> Circle3 {
    Circle3::try_from_center_point(
        point(0.0, 0.0),
        point(5.0, 0.0),
        axis(0.0, 0.0, 1.0),
        Tolerance::DEFAULT,
    )
    .unwrap()
}

fn check_correspondence(curve: CurveRef<'_>) {
    let rational = curve.to_nurbs().unwrap();
    let domain = curve.domain();
    let epsilon = (domain.end() - domain.start()) * 2e-14;
    for i in 0..=257 {
        let t = curve.parameter_at(i as Real / 257.0).unwrap();
        let n = curve.nurbs_parameter(t).unwrap();
        let back = curve.parameter_from_nurbs(n).unwrap();
        assert!((back - t).abs() <= epsilon, "{t} -> {n} -> {back}");
        assert!(
            curve
                .evaluate(t)
                .unwrap()
                .distance_to(rational.evaluate(n).unwrap())
                .unwrap()
                < 2e-12
        );
        let c = curve.parameter_from_nurbs(t).unwrap();
        let back = curve.nurbs_parameter(c).unwrap();
        assert!((back - t).abs() <= epsilon, "{t} -> {c} -> {back}");
        assert!(
            curve
                .evaluate(c)
                .unwrap()
                .distance_to(rational.evaluate(t).unwrap())
                .unwrap()
                < 2e-12
        );
    }
    for endpoint in [*domain.start(), *domain.end()] {
        assert_eq!(curve.nurbs_parameter(endpoint).unwrap(), endpoint);
        assert_eq!(curve.parameter_from_nurbs(endpoint).unwrap(), endpoint);
    }
}

#[test]
fn circular_mapping_matches_rational_locus_in_both_directions() {
    for sweep in [
        1e-10,
        0.1,
        PI * 0.5,
        PI * 0.5 + 1e-8,
        PI * 0.7,
        PI,
        PI * 1.3,
        TAU,
    ] {
        let arc = CircularArc3::try_from_circle_sweep(circle(), sweep).unwrap();
        for domain in [0.0..=1.0, -7.0..=13.0, 1e-200..=9e-200, -1e200..=3e200] {
            let arc = Curve3::Arc(arc.try_reparameterized(domain).unwrap());
            check_correspondence(arc.as_ref());
            check_correspondence(arc.reversed(Tolerance::DEFAULT).unwrap().as_ref());
        }
    }
    check_correspondence(CurveRef::Circle(&circle()));
}

#[test]
fn quarter_knots_and_their_neighbors_do_not_change_spans() {
    let circle = circle().try_reparameterized(-7.0..=13.0).unwrap();
    let view = CurveRef::Circle(&circle);
    for knot in [-2.0_f64, 3.0, 8.0] {
        assert_eq!(view.nurbs_parameter(knot).unwrap(), knot);
        assert_eq!(view.parameter_from_nurbs(knot).unwrap(), knot);
        for neighbor in [knot.next_down(), knot.next_up()] {
            let n = view.nurbs_parameter(neighbor).unwrap();
            assert!((n - neighbor).abs() < 1e-14);
            view.parameter_from_nurbs(neighbor).unwrap();
        }
    }
    let t = -6.0;
    assert!((view.nurbs_parameter(t).unwrap() - t).abs() > 0.01);
}

#[test]
fn parameter_equivalent_families_are_exact_identities() {
    let line = LineSegment::try_new(point(0.0, 0.0), point(3.0, 1.0), Tolerance::DEFAULT).unwrap();
    let ellipse = Ellipse3::try_new(
        point(0.0, 0.0),
        5.0,
        2.0,
        axis(1.0, 0.0, 0.0),
        axis(0.0, 1.0, 0.0),
        Tolerance::DEFAULT,
    )
    .unwrap();
    let polyline = Polyline3::try_new(
        vec![point(0.0, 0.0), point(3.0, 1.0), point(2.0, 4.0)],
        Tolerance::DEFAULT,
    )
    .unwrap();
    let nurbs =
        NurbsCurve::try_clamped_uniform(2, vec![point(0.0, 0.0), point(3.0, 1.0), point(2.0, 4.0)])
            .unwrap();
    for curve in [
        Curve3::Line(line),
        Curve3::Ellipse(ellipse),
        Curve3::Polyline(polyline),
        Curve3::NurbsCurve(nurbs),
    ] {
        let curve = curve.try_reparameterized(-7.0..=13.0).unwrap();
        check_correspondence(curve.as_ref());
        for t in [-7.0, -3.125, 0.0, 3.1, 13.0] {
            assert_eq!(curve.as_ref().nurbs_parameter(t).unwrap(), t);
            assert_eq!(curve.as_ref().parameter_from_nurbs(t).unwrap(), t);
        }
    }
}

#[test]
fn composite_mapping_preserves_coincident_branches_and_leaf_domains() {
    let first = CircularArc3::try_from_circle_sweep(circle(), PI)
        .unwrap()
        .try_reparameterized(-8.0..=-3.0)
        .unwrap();
    let second =
        CircularArc3::try_from_circle_sweep(circle().try_change_closed_seam(5.0 * PI).unwrap(), PI)
            .unwrap()
            .try_reparameterized(31.0..=36.0)
            .unwrap();
    let curve = PolyCurve3::try_new(vec![
        CurveSegment3::Arc(first),
        CurveSegment3::Arc(second),
        CurveSegment3::Arc(first),
        CurveSegment3::Arc(second),
    ])
    .unwrap()
    .try_reparameterized(-7.0..=13.0)
    .unwrap();
    let view = CurveRef::PolyCurve(&curve);
    check_correspondence(view);
    let a = view.nurbs_parameter(-6.0).unwrap();
    let b = view.nurbs_parameter(4.0).unwrap();
    assert!((b - a - 10.0).abs() < 1e-13);
    assert!(
        view.evaluate(-6.0)
            .unwrap()
            .distance_to(view.evaluate(4.0).unwrap())
            .unwrap()
            < 1e-13
    );
}

#[test]
fn invalid_parameters_are_rejected_even_for_identity_maps() {
    let curve = circle().to_nurbs().unwrap();
    for view in [CurveRef::Circle(&circle()), CurveRef::NurbsCurve(&curve)] {
        for parameter in [Real::NAN, Real::INFINITY, -1.0, 100.0] {
            assert!(view.nurbs_parameter(parameter).is_err());
            assert!(view.parameter_from_nurbs(parameter).is_err());
        }
    }
}
