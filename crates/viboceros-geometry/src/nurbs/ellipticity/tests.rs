use super::*;
use crate::{Ellipse3, WeightedPoint3};
use std::f64::consts::FRAC_1_SQRT_2;

fn p(x: Real, y: Real, z: Real) -> Point3 {
    Point3::try_new(x, y, z).unwrap()
}

fn ellipse(center: Point3, tilted: bool) -> NurbsCurve {
    let (x, y) = if tilted {
        ([3., 4., 0.], [-4., 3., 5.])
    } else {
        ([1., 0., 0.], [0., 1., 0.])
    };
    Ellipse3::try_new(
        center,
        2.,
        1.,
        Vector3::try_from(x).unwrap().normalized_nonzero().unwrap(),
        Vector3::try_from(y).unwrap().normalized_nonzero().unwrap(),
        Tolerance::DEFAULT,
    )
    .unwrap()
    .to_nurbs()
    .unwrap()
}

fn assert_center(curve: &NurbsCurve, expected: Point3) {
    let before = curve.clone();
    let actual = curve
        .elliptical_center(Tolerance::DEFAULT)
        .unwrap()
        .unwrap_or_else(|| panic!("missing ellipse: {curve:?}"));
    assert!(
        actual.distance_to(expected).unwrap() < 1e-9,
        "{actual:?} vs {expected:?}"
    );
    assert_eq!(curve, &before);
}

#[test]
fn ellipses_retain_centers_under_elevation_refinement_reversal_gauges_and_domains() {
    for tilted in [false, true] {
        let center = p(4., -4., 7.);
        let base = ellipse(center, tilted);
        for source in [base.clone(), base.try_bezier_spans().unwrap().remove(0)] {
            for degree in [2, 5, 12] {
                let source = source.try_change_degree(degree, false).unwrap();
                let source = source
                    .try_insert_knot(source.parameter_at(0.25).unwrap(), 1)
                    .unwrap();
                for gauge in [1., -8., 1e-200, -1e200] {
                    let scaled = NurbsCurve::try_new_rational(
                        degree,
                        source
                            .control_points()
                            .iter()
                            .map(|c| {
                                WeightedPoint3::try_new(c.point(), c.weight() * gauge).unwrap()
                            })
                            .collect(),
                        source.knots().to_vec(),
                    )
                    .unwrap();
                    for domain in [0. ..=1., 1e12..=1e12 + 8., 0. ..=1e-170, 0. ..=1e170] {
                        let curve = scaled.try_reparameterized(domain).unwrap();
                        assert_center(&curve, center);
                        assert_center(&curve.reversed().unwrap(), center);
                        assert!(curve.circular_radius(Tolerance::DEFAULT).unwrap().is_none());
                    }
                }
            }
        }
    }
}

#[test]
fn translated_ellipses_and_quadratic_control_triangle_have_independent_centers() {
    let center = p(1e12, -1e12, 1e12);
    assert_center(&ellipse(center, false), center);
    // The retained Rhino "noncircle" fixture is a genuine elliptic arc, not a
    // malformed circle. For w0=w2=1 and w1²=1/2 its center is P0+P2-P1.
    let curve = NurbsCurve::try_new_rational(
        2,
        vec![
            WeightedPoint3::try_new(p(2., -4., 0.), 1.).unwrap(),
            WeightedPoint3::try_new(p(2.25, -2., 0.), FRAC_1_SQRT_2).unwrap(),
            WeightedPoint3::try_new(p(4., -2., 0.), 1.).unwrap(),
        ],
        vec![0., 0., 0., 1., 1., 1.],
    )
    .unwrap();
    assert_center(&curve, p(3.75, -4., 0.));
    assert!(curve.circular_center(Tolerance::DEFAULT).unwrap().is_none());
}

#[test]
fn nonlinear_quartic_parameterization_with_stationary_endpoint_is_elliptic() {
    // Quadratic ellipse composed with t²: H0,H0,(2H0+H1)/3,H1,H2.
    let w = FRAC_1_SQRT_2;
    let curve = NurbsCurve::try_new_rational(
        4,
        vec![
            WeightedPoint3::try_new(p(2., -4., 7.), 1.).unwrap(),
            WeightedPoint3::try_new(p(2., -4., 7.), 1.).unwrap(),
            WeightedPoint3::try_new(p(2., (-8. - 3. * w) / (2. + w), 7.), (2. + w) / 3.).unwrap(),
            WeightedPoint3::try_new(p(2., -3., 7.), w).unwrap(),
            WeightedPoint3::try_new(p(4., -3., 7.), 1.).unwrap(),
        ],
        vec![0., 0., 0., 0., 0., 1., 1., 1., 1., 1.],
    )
    .unwrap();
    assert_eq!(curve.derivative_at(0.).unwrap().length().unwrap(), 0.);
    assert_center(&curve, p(4., -4., 7.));
}

#[test]
fn rejects_parabolic_hyperbolic_linear_mixed_sign_and_nearly_linear_loci() {
    for w in [1., 2., -FRAC_1_SQRT_2, 1. - 1e-12] {
        let curve = NurbsCurve::try_new_rational(
            2,
            vec![
                WeightedPoint3::try_new(p(0., 0., 0.), 1.).unwrap(),
                WeightedPoint3::try_new(p(0., 1., 0.), w).unwrap(),
                WeightedPoint3::try_new(p(1., 1., 0.), 1.).unwrap(),
            ],
            vec![0., 0., 0., 1., 1., 1.],
        )
        .unwrap();
        assert!(
            curve
                .elliptical_center(Tolerance::DEFAULT)
                .unwrap()
                .is_none(),
            "weight {w}"
        );
    }
    let line = NurbsCurve::try_new(
        2,
        vec![p(0., 0., 0.), p(1., 1., 0.), p(2., 2., 0.)],
        vec![0., 0., 0., 1., 1., 1.],
    )
    .unwrap();
    assert!(
        line.elliptical_center(Tolerance::DEFAULT)
            .unwrap()
            .is_none()
    );
    let curve = ellipse(p(0., 0., 0.), false);
    let tiny_arc = curve.try_trimmed(0. ..=1e-5).unwrap();
    assert!(
        tiny_arc
            .elliptical_center(Tolerance::DEFAULT)
            .unwrap()
            .is_none()
    );
}

#[test]
fn every_span_and_plane_are_checked_not_only_the_proposal_span() {
    let curve = ellipse(p(4., -4., 7.), false);
    for changed in [p(4., -5.2, 7.), p(4., -5., 7.2)] {
        let mut controls = curve.control_points().to_vec();
        let last_quarter = controls.len() - 2;
        controls[last_quarter] =
            WeightedPoint3::try_new(changed, controls[last_quarter].weight()).unwrap();
        let bent = NurbsCurve::try_new_rational(2, controls, curve.knots().to_vec()).unwrap();
        assert!(
            bent.elliptical_center(Tolerance::DEFAULT)
                .unwrap()
                .is_none()
        );
    }
}

#[test]
fn positive_and_negative_independent_span_gauges_are_equivalent() {
    let curve = ellipse(p(0., 0., 0.), false);
    let spans = curve.try_bezier_spans().unwrap();
    let controls = spans
        .iter()
        .enumerate()
        .flat_map(|(i, span)| {
            span.control_points().iter().map(move |c| {
                WeightedPoint3::try_new(
                    c.point(),
                    c.weight() * if i % 2 == 0 { 1e-200 } else { -1e200 },
                )
                .unwrap()
            })
        })
        .collect();
    let knots = (0..=4).flat_map(|i| [i as Real; 3]).collect();
    let disconnected = NurbsCurve::try_new_rational(2, controls, knots).unwrap();
    assert_center(&disconnected, p(0., 0., 0.));
}

#[test]
fn projective_reparameterization_with_unequal_endpoint_weights_keeps_center() {
    let center = p(4., -4., 7.);
    let source = ellipse(center, true).try_bezier_spans().unwrap().remove(0);
    for ratio in [0.25_f64, 0.5, 2., 4.] {
        let controls = source
            .control_points()
            .iter()
            .enumerate()
            .map(|(i, c)| {
                WeightedPoint3::try_new(c.point(), c.weight() * ratio.powi(i as i32)).unwrap()
            })
            .collect();
        let curve = NurbsCurve::try_new_rational(2, controls, source.knots().to_vec()).unwrap();
        assert_center(&curve, center);
    }
}

#[test]
fn ellipse_recognition_is_covariant_with_model_and_tolerance_scale() {
    let base = ellipse(p(4., -4., 7.), true);
    for scale in [1e-100, 1., 1e100] {
        let controls = base
            .control_points()
            .iter()
            .map(|c| {
                WeightedPoint3::try_new(
                    Point3::try_from(c.point().to_array().map(|v| v * scale)).unwrap(),
                    c.weight(),
                )
                .unwrap()
            })
            .collect();
        let curve = NurbsCurve::try_new_rational(2, controls, base.knots().to_vec()).unwrap();
        let tolerance = Tolerance::try_new(1e-9 * scale, 1e-12, 1e-10).unwrap();
        let center = curve.elliptical_center(tolerance).unwrap().unwrap();
        for (actual, expected) in center.to_array().into_iter().zip([4., -4., 7.]) {
            assert!((actual / scale - expected).abs() < 1e-10);
        }
    }
}

#[test]
fn independently_derived_quadratic_centers_and_circles_agree() {
    use super::super::exact::{rational, scalar};
    for weight in [0.4, 0.5, 0.7, 0.8] {
        let points = [p(0., 0., 2.), p(0., 1., 2.), p(2., 1., 2.)];
        let curve = NurbsCurve::try_new_rational(
            2,
            points
                .into_iter()
                .zip([1., weight, 1.])
                .map(|(p, w)| WeightedPoint3::try_new(p, w).unwrap())
                .collect(),
            vec![0., 0., 0., 1., 1., 1.],
        )
        .unwrap();
        // Exact rational arithmetic over the binary64 inputs supplies an
        // independent center oracle, without fitting or sampled evaluation.
        let k = rational(weight) * rational(weight) * rational(2.);
        let expected = Point3::try_from(std::array::from_fn(|i| {
            scalar(
                &((rational(points[0].to_array()[i]) + rational(points[2].to_array()[i])
                    - &k * rational(points[1].to_array()[i]))
                    / (rational(2.) - &k)),
            )
            .unwrap()
        }))
        .unwrap();
        assert_center(&curve, expected);
    }
    let center = p(4., -4., 7.);
    let circle = crate::Circle3::try_new(
        center,
        2.,
        Vector3::try_new(0., 0., 1.)
            .unwrap()
            .normalized_nonzero()
            .unwrap(),
        Tolerance::DEFAULT,
    )
    .unwrap()
    .to_nurbs()
    .unwrap();
    for degree in [2, 5, 12] {
        let curve = circle.try_change_degree(degree, false).unwrap();
        assert_center(&curve, center);
        assert!(
            curve
                .circular_center(Tolerance::DEFAULT)
                .unwrap()
                .unwrap()
                .distance_to(center)
                .unwrap()
                < 1e-9
        );
    }
}
