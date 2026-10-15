use super::*;
use crate::WeightedPoint3;

fn p(x: f64, y: f64) -> Point3 {
    Point3::try_new(x, y, 0.).unwrap()
}
fn tolerance(absolute: f64) -> Tolerance {
    Tolerance::try_new(absolute, 1e-12, 1e-10).unwrap()
}
fn check(curve: &NurbsCurve, error: f64) -> Polyline3 {
    let original = curve.clone();
    let polyline = curve.try_approximate_polyline(tolerance(error)).unwrap();
    assert_eq!(curve, &original);
    assert_eq!(polyline.domain(), curve.domain());
    for pair in polyline.parameters().windows(2) {
        for i in 0..=12 {
            let t = pair[0] + (pair[1] - pair[0]) * i as f64 / 12.;
            let a = curve.evaluate(t).unwrap();
            let b = polyline.evaluate(t).unwrap();
            assert!(a.distance_to(b).unwrap() <= error * (1. + 1e-9));
        }
    }
    polyline
}

#[test]
fn straight_spans_keep_native_parameters_and_kinks() {
    let curve = NurbsCurve::try_new(
        1,
        vec![p(0., 0.), p(1., 0.), p(1., 2.)],
        vec![10., 10., 13., 21., 21.],
    )
    .unwrap();
    let polyline = check(&curve, 1e-6);
    assert_eq!(polyline.parameters(), &[10., 13., 21.]);
    assert_eq!(polyline.vertices(), &[p(0., 0.), p(1., 0.), p(1., 2.)]);
}

#[test]
fn rational_quarter_circle_supports_both_weight_gauges() {
    for gauge in [1., -1., 1e-250, -1e250] {
        let curve = NurbsCurve::try_new_rational(
            2,
            [
                (p(1., 0.), 1.),
                (p(1., 1.), std::f64::consts::FRAC_1_SQRT_2),
                (p(0., 1.), 1.),
            ]
            .into_iter()
            .map(|(p, w)| WeightedPoint3::try_new(p, w * gauge).unwrap())
            .collect(),
            vec![-3., -3., -3., 7., 7., 7.],
        )
        .unwrap();
        let polyline = check(&curve, 1e-3);
        assert!(polyline.vertices().len() > 16);
        assert_eq!(polyline.vertices()[0], p(1., 0.));
        assert_eq!(*polyline.vertices().last().unwrap(), p(0., 1.));
    }
}

#[test]
fn collinear_reversal_is_not_replaced_by_the_endpoint_chord() {
    let curve = NurbsCurve::try_new(
        3,
        vec![p(0., 0.), p(5., 0.), p(-4., 0.), p(1., 0.)],
        vec![0., 0., 0., 0., 1., 1., 1., 1.],
    )
    .unwrap();
    let polyline = check(&curve, 1e-3);
    assert!(polyline.vertices().iter().any(|p| p.x() > 1.));
    assert!(polyline.vertices().iter().any(|p| p.x() < 0.));
}

#[test]
fn a_rational_line_retains_its_nonuniform_parameter_speed() {
    let curve = NurbsCurve::try_new_rational(
        1,
        vec![
            WeightedPoint3::try_new(p(0., 0.), 1.).unwrap(),
            WeightedPoint3::try_new(p(1., 0.), 100.).unwrap(),
        ],
        vec![0., 0., 1., 1.],
    )
    .unwrap();
    let polyline = check(&curve, 1e-3);
    assert!(polyline.vertices().len() > 2);
    assert!(polyline.evaluate(0.5).unwrap().x() > 0.98);
}

#[test]
fn unsupported_degree_declines_before_subdivision() {
    let curve = NurbsCurve::try_new(
        17,
        (0..18).map(|i| p(i as f64, (i % 2) as f64)).collect(),
        [vec![0.; 18], vec![1.; 18]].concat(),
    )
    .unwrap();
    assert!(matches!(
        curve.try_approximate_polyline(tolerance(1e-3)),
        Err(GeometryError::UnsupportedCurveApproximation {
            context: "degree exceeds 16"
        })
    ));
}

#[test]
fn closed_curve_with_coincident_endpoint_and_midpoint_is_retained() {
    let curve = NurbsCurve::try_new(
        3,
        vec![p(0., 0.), p(2., 3.), p(-2., -3.), p(0., 0.)],
        vec![0., 0., 0., 0., 1., 1., 1., 1.],
    )
    .unwrap();
    let polyline = check(&curve, 1e-3);
    assert!(polyline.vertices().len() > 10);
    assert_eq!(polyline.vertices().first(), polyline.vertices().last());
    assert!(polyline.vertices().iter().any(|p| p.y() > 0.5));
    assert!(polyline.vertices().iter().any(|p| p.y() < -0.5));
}

#[test]
fn unclamped_multi_span_curve_keeps_internal_knots() {
    let curve = NurbsCurve::try_new(
        2,
        vec![p(0., 1.), p(1., 3.), p(2., -1.), p(3., 2.)],
        vec![-3., -2., -1., 0.1, 3., 4., 5.],
    )
    .unwrap();
    let polyline = check(&curve, 1e-3);
    assert!(polyline.parameters().contains(&0.1));
}

#[test]
fn mixed_weights_constant_spans_and_discontinuous_knots_decline() {
    let mixed = NurbsCurve::try_new_rational(
        1,
        vec![
            WeightedPoint3::try_new(p(0., 0.), 1.).unwrap(),
            WeightedPoint3::try_new(p(1., 0.), -1.).unwrap(),
        ],
        vec![0., 0., 1., 1.],
    )
    .unwrap();
    let constant = NurbsCurve::try_new(1, vec![p(1., 0.); 2], vec![0., 0., 1., 1.]).unwrap();
    let discontinuous = NurbsCurve::try_new(
        1,
        vec![p(0., 0.), p(1., 0.), p(1., 1.), p(2., 1.)],
        vec![0., 0., 0.5, 0.5, 1., 1.],
    )
    .unwrap();
    for curve in [mixed, constant, discontinuous] {
        assert!(matches!(
            curve.try_approximate_polyline(tolerance(1e-3)),
            Err(GeometryError::UnsupportedCurveApproximation { .. })
        ));
    }
}

#[test]
fn unsplittable_native_parameter_interval_declines_instead_of_accepting_a_chord() {
    let a = 1.;
    let b = 1_f64.next_up();
    let curve = NurbsCurve::try_new(
        2,
        vec![p(0., 0.), p(1., 2.), p(2., 0.)],
        vec![a, a, a, b, b, b],
    )
    .unwrap();
    assert_eq!(
        curve.try_approximate_polyline(tolerance(1e-3)),
        Err(GeometryError::CurveApproximationLimit)
    );
}

#[test]
fn a_rounded_native_midpoint_restricts_at_its_actual_parameter() {
    let a = 1.;
    let middle = 1_f64.next_up().next_up();
    let b = middle.next_up();
    let curve = NurbsCurve::try_new(
        2,
        vec![p(0., 0.), p(1., 2.), p(2., 0.)],
        vec![a, a, a, b, b, b],
    )
    .unwrap();
    let output = curve.try_approximate_polyline(tolerance(0.2)).unwrap();
    let index = output
        .parameters()
        .iter()
        .position(|&t| t == middle)
        .unwrap();
    assert!(
        output.vertices()[index]
            .distance_to(p(4. / 3., 8. / 9.))
            .unwrap()
            < 1e-14
    );
}

#[test]
fn large_coordinates_account_for_endpoint_rounding() {
    let base = 2_f64.powi(50);
    let curve = NurbsCurve::try_new(
        2,
        vec![p(base, 0.), p(base + 1., 2.), p(base + 2., 0.)],
        vec![0., 0., 0., 1., 1., 1.],
    )
    .unwrap();
    let polyline = curve.try_approximate_polyline(tolerance(0.3)).unwrap();
    assert!(polyline.vertices().len() > 2);
    assert_eq!(
        curve.try_approximate_polyline(tolerance(1e-6)),
        Err(GeometryError::CurveApproximationLimit)
    );
}
