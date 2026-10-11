#![cfg(feature = "native")]
use viboceros_geometry::{NurbsCurve, Point3, WeightedPoint3};
use viboceros_smlib::KernelCurve;

fn curve(degree: usize, points: &[[f64; 3]], weights: &[f64], knots: &[f64]) -> NurbsCurve {
    NurbsCurve::try_new_rational(
        degree,
        points
            .iter()
            .zip(weights)
            .map(|(p, &w)| {
                WeightedPoint3::try_new(Point3::try_new(p[0], p[1], p[2]).unwrap(), w).unwrap()
            })
            .collect(),
        knots.to_vec(),
    )
    .unwrap()
}

fn check_import(source: &NurbsCurve, modeling: bool) {
    let before = source.clone();
    let native = if modeling {
        KernelCurve::from_nurbs_for_modeling(source)
    } else {
        KernelCurve::from_nurbs(source)
    }
    .unwrap();
    let output = native.to_nurbs().unwrap();
    assert_eq!(output.domain(), source.domain());
    assert_eq!(output.degree(), source.degree());
    let p = output.degree();
    assert!(
        output.knots()[..=p]
            .iter()
            .all(|&t| t == *output.domain().start())
    );
    assert!(
        output.knots()[output.control_points().len()..]
            .iter()
            .all(|&t| t == *output.domain().end())
    );
    for i in 0..=128 {
        let t = source.parameter_at(i as f64 / 128.).unwrap();
        let expected = source.evaluate(t).unwrap();
        assert!(native.evaluate(t).unwrap().distance_to(expected).unwrap() < 1e-11);
        assert!(output.evaluate(t).unwrap().distance_to(expected).unwrap() < 1e-11);
    }
    assert_eq!(source, &before);
}

#[test]
fn native_canonical_clamping_preserves_unclamped_and_partial_end_intervals() {
    let cases = [
        curve(
            1,
            &[[0., 0., 0.], [2., 0., 0.]],
            &[1., 1.],
            &[-1., 0., 1., 2.],
        ),
        curve(
            2,
            &[[0., 0., 0.], [1., 2., 0.], [3., 2., 1.], [4., 0., 0.]],
            &[1., 0.75, 0.5, 1.],
            &[-3., -2., -1., 0., 1., 2., 3.],
        ),
        curve(
            3,
            &[
                [0., 0., 0.],
                [1., 2., 0.],
                [2., 2., 1.],
                [3., 0., 0.],
                [4., 1., 0.],
            ],
            &[1., 0.8, 0.6, 0.9, 1.],
            &[-3., -2., -1., 0., 1., 2., 2., 2., 2.],
        ),
        curve(
            2,
            &[
                [0., 0., 0.],
                [1., 2., 0.],
                [2., 1., 1.],
                [3., 1., 0.],
                [4., 0., 0.],
            ],
            &[1., 0.8, 0.6, 0.9, 1.],
            &[-5., -5., -1., 0., 0., 2., 5., 5.],
        ),
    ];
    for source in cases {
        check_import(&source, false);
        check_import(&source, true);
    }
}

#[test]
fn periodic_and_negative_gauge_imports_preserve_parameterized_geometry() {
    let points = [
        [-1., 0., 0.],
        [0., 1., 0.],
        [1., 0., 0.],
        [0., -1., 0.],
        [-1., 0., 0.],
        [0., 1., 0.],
    ];
    for gauge in [1., -1.] {
        let source = curve(
            2,
            &points,
            &[gauge; 6],
            &[-2., -1., 0., 1., 2., 3., 4., 5., 6.],
        );
        assert!(source.is_closed().unwrap());
        check_import(&source, true);
        let native = KernelCurve::from_nurbs_for_modeling(&source)
            .unwrap()
            .to_nurbs()
            .unwrap();
        assert!(native.is_closed().unwrap());
    }
}

#[test]
fn native_clamping_preserves_extreme_parameter_scales() {
    for exponent in [-360, 0, 360] {
        let scale = 2f64.powi(exponent);
        let knots = [-3., -2., -1., 0., 1., 2., 3.].map(|k| k * scale);
        let source = curve(
            2,
            &[[0., 0., 0.], [1., 2., 0.], [3., 2., 1.], [4., 0., 0.]],
            &[1., 0.75, 0.5, 1.],
            &knots,
        );
        check_import(&source, false);
    }
}

#[test]
fn constructor_knot_fixup_does_not_flatten_a_short_parameter_span() {
    let close = 0.5 + 1e-14;
    let source = curve(
        2,
        &[
            [0., 0., 0.],
            [1., 0., 0.],
            [2., 0., 0.],
            [2., 10., 0.],
            [3., 0., 0.],
            [4., 0., 0.],
            [5., 0., 0.],
        ],
        &[1., 0.75, 1., 0.5, 1., 0.75, 1.],
        &[0., 0., 0., 0.5, 0.5, close, close, 1., 1., 1.],
    );
    let native = KernelCurve::from_nurbs(&source).unwrap();
    let output = native.to_nurbs().unwrap();
    assert_eq!(output.knots(), source.knots());
    for t in [0.5, (0.5 + close) * 0.5, close] {
        let expected = source.evaluate(t).unwrap();
        assert!(native.evaluate(t).unwrap().distance_to(expected).unwrap() < 1e-11);
        assert!(output.evaluate(t).unwrap().distance_to(expected).unwrap() < 1e-11);
    }
}

#[test]
fn canonical_degree_and_default_weight_limits_are_explicit() {
    for degree in [17, 18, 32, 33] {
        let points = (0..=degree).map(|i| [i as f64, 0., 0.]).collect::<Vec<_>>();
        let mut knots = vec![0.; degree + 1];
        knots.extend(vec![1.; degree + 1]);
        let source = curve(degree, &points, &vec![1.; degree + 1], &knots);
        if degree == 17 {
            check_import(&source, false);
        } else {
            assert!(KernelCurve::from_nurbs(&source).is_err());
        }
    }
    for weight in [1e-308, 1e-4, 1e-3, 100., 101., 1e308] {
        for sign in [1., -1.] {
            let source = curve(
                1,
                &[[0., 0., 0.], [2., 1., 0.]],
                &[weight * sign; 2],
                &[0., 0., 1., 1.],
            );
            check_import(&source, true);
        }
    }
    for weight in [1e-4, 1e-3, 100., 101.] {
        let source = curve(
            1,
            &[[0., 0., 0.], [2., 1., 0.]],
            &[weight, 1.],
            &[0., 0., 1., 1.],
        );
        if (1e-3..=100.).contains(&weight) {
            check_import(&source, false);
        } else {
            assert!(KernelCurve::from_nurbs(&source).is_err());
        }
    }
    let discontinuous = curve(
        1,
        &[[0., 0., 0.], [1., 0., 0.], [4., 0., 0.], [5., 0., 0.]],
        &[1.; 4],
        &[0., 0., 1., 1., 2., 2.],
    );
    assert!(KernelCurve::from_nurbs(&discontinuous).is_err());
}
