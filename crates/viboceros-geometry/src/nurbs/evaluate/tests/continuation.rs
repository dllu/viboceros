use super::*;

#[test]
fn continuation_uses_endpoint_polynomials_and_keeps_interior_right_limits() {
    // Two independent quadratic Beziers, with a full-multiplicity interior knot.
    let curve = NurbsCurve::try_new(
        2,
        vec![
            point(0., 0., 0.),
            point(0.5, 0., 0.),
            point(1., 1., 0.),
            point(10., 1., 0.),
            point(11., 1., 0.),
            point(12., 3., 0.),
        ],
        vec![0., 0., 0., 1., 1., 1., 3., 3., 3.],
    )
    .unwrap();
    for (t, expected, first, second) in [
        (-1., [-1., 1., 0.], [1., -2., 0.], [0., 2., 0.]),
        (4., [13., 5.5, 0.], [1., 3., 0.], [0., 1., 0.]),
    ] {
        let (p, d, dd) = curve.evaluate_extended_with_second_derivative(t).unwrap();
        assert_eq!(p.to_array(), expected);
        assert_eq!(d.to_array(), first);
        assert_eq!(dd.to_array(), second);
        assert!(curve.evaluate_with_second_derivative(t).is_err());
    }
    for t in [0., 0.25, 1., 2., 3.] {
        assert_eq!(
            curve.evaluate_extended_with_second_derivative(t),
            curve.evaluate_with_second_derivative(t)
        );
    }
    assert_eq!(
        curve
            .evaluate_extended_with_second_derivative(1.)
            .unwrap()
            .0,
        point(10., 1., 0.)
    );
}

#[test]
fn continuation_skips_empty_endpoint_spans() {
    let curve = NurbsCurve::try_new(
        1,
        vec![
            point(9., 0., 0.),
            point(0., 0., 0.),
            point(2., 0., 0.),
            point(8., 0., 0.),
        ],
        vec![-1., 0., 0., 1., 1., 2.],
    )
    .unwrap();
    for t in [-1., 2.] {
        let (p, d, dd) = curve.evaluate_extended_with_second_derivative(t).unwrap();
        assert_eq!(p, point(2. * t, 0., 0.));
        assert_eq!(d.to_array(), [2., 0., 0.]);
        assert_eq!(dd.to_array(), [0.; 3]);
    }
}

#[test]
fn rational_continuation_retains_quotient_jets_and_reports_exterior_poles() {
    for gauge in [1., -1., 2_f64.powi(-600), 2_f64.powi(600)] {
        let curve = rational_line(0., gauge);
        for t in [-2_f64, -0.5, 2., 4.] {
            let (p, d, dd) = curve.evaluate_extended_with_second_derivative(t).unwrap();
            for (actual, expected) in [
                (p.x(), 8. * t / (1. + t)),
                (d.x(), 8. / (1. + t).powi(2)),
                (dd.x(), -16. / (1. + t).powi(3)),
            ] {
                assert!((actual - expected).abs() <= expected.abs() * 2e-14);
            }
            assert_eq!([p.y(), p.z(), d.y(), d.z(), dd.y(), dd.z()], [0.; 6]);
        }
        assert_eq!(
            curve.evaluate_extended_with_second_derivative(-1.),
            Err(GeometryError::ZeroWeightAtParameter)
        );
        for t in [Real::NAN, Real::INFINITY, Real::NEG_INFINITY] {
            assert!(matches!(
                curve.evaluate_extended_with_second_derivative(t),
                Err(GeometryError::NonFinite { .. })
            ));
        }
    }
}
