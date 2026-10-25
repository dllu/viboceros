use super::*;
use crate::Point3;

fn curve(points: &[[f64; 3]], weights: &[f64], degree: usize, knots: Vec<f64>) -> NurbsCurve {
    NurbsCurve::try_new_rational(
        degree,
        points
            .iter()
            .zip(weights)
            .map(|(p, w)| WeightedPoint3::try_new(Point3::try_from(*p).unwrap(), *w).unwrap())
            .collect(),
        knots,
    )
    .unwrap()
}

fn uneven(weights: &[f64]) -> NurbsCurve {
    curve(
        &[[1., -2., 3.], [1.25, -1.5, 3.], [2., 0., 3.], [5., 6., 3.]],
        weights,
        3,
        vec![0., 0., 0., 0., 1., 1., 1., 1.],
    )
}

#[test]
fn rational_lines_have_uniform_cubic_speed_and_preserve_sources() {
    for weights in [[1., 2., 0.5, 1.], [-1., -2., -0.5, -1.]] {
        let source = uneven(&weights);
        let before = source.clone();
        let fitted =
            try_prepare_refitted_loft_sections(std::slice::from_ref(&source), 1e-12).unwrap();
        let fitted = &fitted[0];
        assert_eq!(fitted.degree(), 3);
        assert_eq!(fitted.control_points().len(), 4);
        assert!(fitted.control_points().iter().all(|c| c.weight() == 1.));
        for i in 0..=20 {
            let t = f64::from(i) / 20.;
            let actual = fitted.evaluate(t).unwrap();
            assert!(
                actual
                    .distance_to(Point3::try_new(1. + 4. * t, -2. + 8. * t, 3.).unwrap())
                    .unwrap()
                    < 4e-15
            );
        }
        assert!(
            source
                .evaluate(0.5)
                .unwrap()
                .distance_to(fitted.evaluate(0.5).unwrap())
                .unwrap()
                > 1.
        );
        assert_eq!(source, before);
    }
}

#[test]
fn continuous_piecewise_lines_and_stationary_ends_are_admitted() {
    let source = curve(
        &[[0., 0., 0.], [1., 0., 0.], [4., 0., 0.]],
        &[1.; 3],
        1,
        vec![-7., -7., 0.3, 13., 13.],
    );
    let fitted = try_prepare_refitted_loft_sections(&[source], 1e-12).unwrap();
    assert_eq!(fitted[0].evaluate(0.5).unwrap().x(), 2.);
    let source = curve(
        &[[0., 0., 0.], [0., 0., 0.], [4., 0., 0.], [4., 0., 0.]],
        &[1.; 4],
        3,
        vec![0., 0., 0., 0., 1., 1., 1., 1.],
    );
    assert!(try_prepare_refitted_loft_sections(&[source], 1e-12).is_ok());
}

#[test]
fn curves_reversals_jumps_unclamped_curves_and_mixed_weights_fail_explicitly() {
    let source = uneven(&[1.; 4]);
    let mut near = source
        .control_points()
        .iter()
        .map(|c| c.point().to_array())
        .collect::<Vec<_>>();
    near[1][2] += 1e-12;
    let knots = source.knots().to_vec();
    let mut reverse = near.clone();
    reverse[1] = [4., 4., 3.];
    reverse[2] = [2., 0., 3.];
    for source in [
        curve(&near, &[1.; 4], 3, knots.clone()),
        curve(&reverse, &[1.; 4], 3, knots),
        uneven(&[1., -2., 0.5, 1.]),
        curve(
            &[[0., 0., 0.], [1., 0., 0.], [2., 0., 0.], [3., 0., 0.]],
            &[1.; 4],
            1,
            vec![0., 0., 0.5, 0.5, 1., 1.],
        ),
        curve(
            &[[0., 0., 0.], [1., 0., 0.], [3., 0., 0.]],
            &[1.; 3],
            2,
            vec![-2., -1., 0., 1., 2., 3.],
        ),
        curve(
            &[[0., 0., 0.]; 4],
            &[1.; 4],
            3,
            vec![0., 0., 0., 0., 1., 1., 1., 1.],
        ),
    ] {
        assert!(matches!(
            try_prepare_refitted_loft_sections(std::slice::from_ref(&source), 0.1),
            Err(GeometryError::UnsupportedCurveApproximation { .. })
        ));
        assert_eq!(
            try_prepare_refitted_loft_sections(std::slice::from_ref(&source), 0.).unwrap(),
            [source]
        );
    }
}

#[test]
fn unrepresentable_handle_tolerances_and_resource_overflows_are_rejected() {
    let line = curve(
        &[[0., 0., 0.], [2., 0., 0.]],
        &[1.; 2],
        1,
        vec![0., 0., 1., 1.],
    );
    assert!(try_prepare_refitted_loft_sections(std::slice::from_ref(&line), 1e-300).is_err());
    for tolerance in [-1., f64::NAN, f64::INFINITY] {
        assert_eq!(
            try_prepare_refitted_loft_sections(&[], tolerance),
            Err(GeometryError::InvalidTolerance)
        );
    }
    assert!(matches!(
        try_prepare_refitted_loft_sections(&vec![line; 257], 0.001),
        Err(GeometryError::LoftResourceLimit { maximum: 256, .. })
    ));
}
