use super::*;
use crate::{Circle3, Point3, Vector3};

fn p(x: Real, y: Real) -> Point3 {
    Point3::try_new(x, y, 0.).unwrap()
}
fn tolerance() -> Tolerance {
    Tolerance::try_new(1e-12, 1e-12, 1e-10).unwrap()
}

#[test]
fn signed_distances_validate_domains_and_do_not_clamp_unavailable_lengths() {
    let c = NurbsCurve::try_new(1, vec![p(0., 0.), p(10., 0.)], vec![20., 20., 30., 30.]).unwrap();
    for (anchor, distance, expected) in [
        (24., 2., Some(26.)),
        (24., -2., Some(22.)),
        (24., 0., Some(24.)),
        (24., 6., Some(30.)),
        (24., -4., Some(20.)),
        (24., 7., None),
        (24., -5., None),
        (30., 1., None),
        (20., -1., None),
    ] {
        let actual = c
            .parameter_at_arc_length_from(anchor, distance, tolerance())
            .unwrap();
        match (actual, expected) {
            (Some(a), Some(e)) => assert!((a - e).abs() < 1e-12),
            (a, e) => assert_eq!(a, e),
        }
    }
    for anchor in [19., 31., Real::NAN, Real::INFINITY] {
        assert!(
            c.parameter_at_arc_length_from(anchor, 0., tolerance())
                .is_err()
        );
    }
    for distance in [Real::NAN, Real::INFINITY, Real::NEG_INFINITY] {
        assert!(
            c.parameter_at_arc_length_from(24., distance, tolerance())
                .is_err()
        );
    }
}

#[test]
fn quadratic_offsets_match_independent_analytic_arc_lengths_in_both_directions() {
    // C(t) = (30t,30t(1-t)); the Rhino constraint fixtures use this same parabola.
    let c = NurbsCurve::try_new(
        2,
        vec![p(0., 0.), p(15., 15.), p(30., 0.)],
        vec![0., 0., 0., 1., 1., 1.],
    )
    .unwrap();
    let primitive = |u: Real| (u * u.hypot(1.) + u.asinh()) * 0.5;
    for (anchor, distance) in [(0., 10.), (0.5, 5.), (0.9, -8.), (0.7, -19.)] {
        let actual = c
            .parameter_at_arc_length_from(anchor, distance, tolerance())
            .unwrap()
            .unwrap();
        let length = 15. * (primitive(1. - 2. * anchor) - primitive(1. - 2. * actual));
        assert!(
            (length - distance).abs() < 1e-11,
            "{anchor} + {distance}: {actual} -> {length}"
        );
        let reverse = c
            .reversed()
            .unwrap()
            .parameter_at_arc_length_from(-anchor, -distance, tolerance())
            .unwrap()
            .unwrap();
        assert!((actual + reverse).abs() < 1e-13);
    }
}

#[test]
fn local_offsets_survive_prefixes_larger_than_their_floating_point_resolution() {
    let c = NurbsCurve::try_new(
        1,
        vec![p(0., 0.), p(1e16, 0.), p(1e16, 1.), p(1e16, 2.)],
        vec![0., 0., 1., 2., 3., 3.],
    )
    .unwrap();
    for (anchor, distance, expected) in [(1., 0.25, 1.25), (2.5, -0.25, 2.25), (2.5, -1., 1.5)] {
        assert_eq!(
            c.parameter_at_arc_length_from(anchor, distance, tolerance())
                .unwrap(),
            Some(expected)
        );
    }
}

#[test]
fn rational_multispan_circle_offsets_keep_native_domains_and_do_not_wrap() {
    let c = Circle3::try_new(
        p(0., 0.),
        10.,
        Vector3::try_new(0., 0., 1.)
            .unwrap()
            .normalized_nonzero()
            .unwrap(),
        tolerance(),
    )
    .unwrap()
    .to_nurbs()
    .unwrap();
    for domain in [
        0.0..=1.0,
        100.0..=104.0,
        0.0..=1e-200,
        0.0..=1e200,
        -Real::MAX..=Real::MAX,
    ] {
        let mapped = c.try_reparameterized(domain.clone()).unwrap();
        let anchor = mapped.parameter_at(0.25).unwrap();
        for distance in [-5., 20.] {
            let actual = mapped
                .parameter_at_arc_length_from(anchor, distance, tolerance())
                .unwrap()
                .unwrap();
            let angle = std::f64::consts::FRAC_PI_2 + distance / 10.;
            let expected = p(10. * angle.cos(), 10. * angle.sin());
            assert!(
                mapped
                    .evaluate(actual)
                    .unwrap()
                    .distance_to(expected)
                    .unwrap()
                    < 1e-10,
                "{domain:?}"
            );
        }
        assert_eq!(
            mapped
                .parameter_at_arc_length_from(anchor, -20., tolerance())
                .unwrap(),
            None
        );
    }
}
