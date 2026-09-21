use super::*;

fn line(domain: [Real; 2]) -> NurbsCurve2 {
    NurbsCurve2::try_new_rational(
        1,
        vec![
            WeightedPoint2::try_new(Point2::try_new(1., 2.).unwrap(), 1.).unwrap(),
            WeightedPoint2::try_new(Point2::try_new(5., 7.).unwrap(), 3.).unwrap(),
        ],
        vec![domain[0], domain[0], domain[1], domain[1]],
    )
    .unwrap()
}

#[test]
fn lossless_frames_retain_rational_points_and_dimensionless_derivatives_across_exponents() {
    let tiny = Real::from_bits(1);
    for domain in [
        [0., 1.],
        [1e9, 1e9 + 4.],
        [-1e15, -1e15 + 4.],
        [0., tiny],
        [tiny, 2. * tiny],
        [-tiny, tiny],
        [0., 1e-300],
        [-1e300, 1e300],
        [-Real::MAX, Real::MAX],
    ] {
        let source = line(domain);
        let before = source.clone();
        let local = source.for_integration().unwrap();
        assert_eq!(local.control_points(), source.control_points());
        assert_eq!(local.degree(), source.degree());
        let width = local.domain().end() - local.domain().start();
        assert!((1. ..=4.).contains(&width));
        for f in [0., 0.125, 0.25, 0.5, 0.875, 1.] {
            let (point, derivative) = local
                .evaluate_with_derivative(local.parameter_at(f).unwrap())
                .unwrap();
            let divisor = 1. + 2. * f;
            assert!(
                (point.x() - (1. + 14. * f) / divisor).abs() < 2e-15,
                "{domain:?}: {point:?}"
            );
            assert!((point.y() - (2. + 19. * f) / divisor).abs() < 2e-15);
            assert!((derivative[0] * width - 12. / divisor.powi(2)).abs() < 1e-14);
            assert!((derivative[1] * width - 15. / divisor.powi(2)).abs() < 1e-14);
        }
        assert_eq!(source, before);
    }
    assert!(matches!(
        line([0., 1.]).for_integration().unwrap(),
        Cow::Borrowed(_)
    ));
}

#[test]
fn frames_preserve_exterior_knots_and_decline_unrepresentable_relative_ranges() {
    let mut source = line([1., 2.]);
    source.knots[0] = -0.1; // Subtracting the preferred origin is not exact.
    let local = source.for_integration().unwrap();
    assert_eq!(local.knots(), &[-0.05, 0.5, 1., 1.]);
    assert_eq!(local.control_points(), source.control_points());

    for knots in [
        vec![-1e308, 0., Real::from_bits(1), Real::from_bits(1)],
        vec![-Real::from_bits(1), 0., 1e308, 1e308],
    ] {
        let mut source = line([0., 1.]);
        source.knots = knots;
        let before = source.clone();
        assert!(matches!(
            source.for_integration(),
            Err(GeometryError::NumericalIntegrationDidNotConverge)
        ));
        assert_eq!(source, before);
    }
}

#[test]
fn multispan_frames_preserve_cubic_geometry_and_knot_multiplicity_without_fitting() {
    let base = NurbsCurve2::try_new_rational(
        3,
        [1., 2., 3., 0.25, 2., 4.]
            .into_iter()
            .enumerate()
            .map(|(i, w)| {
                WeightedPoint2::try_new(Point2::try_new(i as Real, (i % 3) as Real).unwrap(), w)
                    .unwrap()
            })
            .collect(),
        vec![0., 0., 0., 0., 0.375, 0.875, 1.125, 1.125, 1.125, 1.125],
    )
    .unwrap();
    for (origin, scale) in [
        (0., 2_f64.powi(-500)),
        (0., 2_f64.powi(500)),
        (1e15, 1.),
        (-1e15, 1.),
    ] {
        let source = NurbsCurve2::try_new_rational(
            3,
            base.control_points().to_vec(),
            base.knots().iter().map(|k| origin + scale * k).collect(),
        )
        .unwrap();
        let local = source.for_integration().unwrap();
        assert_eq!(local.degree(), base.degree());
        assert_eq!(local.control_points(), base.control_points());
        assert_eq!(local.knots().len(), base.knots().len());
        let start = *local.domain().start();
        for (&a, &b) in local.knots().iter().zip(base.knots()) {
            assert_eq!(a - start, b);
        }
        for i in 0..=64 {
            let f = i as Real / 64.;
            let (actual, derivative) = local
                .evaluate_with_derivative(local.parameter_at(f).unwrap())
                .unwrap();
            let (expected, expected_derivative) = base
                .evaluate_with_derivative(base.parameter_at(f).unwrap())
                .unwrap();
            assert!((actual.x() - expected.x()).abs() < 1e-13);
            assert!((actual.y() - expected.y()).abs() < 1e-13);
            for axis in 0..2 {
                assert!((derivative[axis] - expected_derivative[axis]).abs() < 1e-12);
            }
        }
    }
}
