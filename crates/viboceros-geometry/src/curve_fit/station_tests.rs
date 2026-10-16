use super::*;
use crate::{LineSegment, ParameterSide, Polyline3};

fn p(x: Real, y: Real, z: Real) -> Point3 {
    Point3::try_new(x, y, z).unwrap()
}

#[test]
fn common_station_fit_recovers_a_quadratic_clock_and_its_endpoint_derivatives() {
    let source = LineSegment::try_new(p(0., 0., 0.), p(8., 0., 0.), Tolerance::DEFAULT).unwrap();
    for (quadratic, times) in [
        (0., &[1., 0.][..]),
        (0.4, &[1., 0.5, 0.][..]),
        (0.4, &[0.8, 1., 0.2, 0., 0.5][..]),
    ] {
        let x = |t: Real| 8. * ((1. - quadratic) * t + quadratic * t * t);
        let stations = times
            .iter()
            .copied()
            .map(|t| CurveFitStation {
                source_parameter: x(t),
                fit_parameter: t,
                point: p(x(t), 0., 0.),
            })
            .collect::<Vec<_>>();
        let fit = try_fit_cubic_curve_at_stations(
            CurveRef::Line(&source),
            &stations,
            1e-8,
            0.,
            Tolerance::DEFAULT,
        )
        .unwrap();
        assert_eq!(fit.curve.domain(), 0. ..=1.);
        assert_eq!(fit.constraint_parameters, times);
        for i in 0..=128 {
            let t = i as Real / 128.;
            assert!(
                fit.curve
                    .evaluate(t)
                    .unwrap()
                    .distance_to(p(x(t), 0., 0.))
                    .unwrap()
                    < 2e-13
            );
        }
        for (t, expected) in [(0., 8. * (1. - quadratic)), (1., 8. * (1. + quadratic))] {
            let (_, tangent) = fit.curve.evaluate_with_derivative(t).unwrap();
            assert!((tangent.x() - expected).abs() < 2e-13);
            assert_eq!(tangent.y(), 0.);
            assert_eq!(tangent.z(), 0.);
        }
    }
}

#[test]
fn common_station_fit_preserves_a_retimed_source_kink_and_one_sided_speeds() {
    let source = Polyline3::try_new(
        vec![p(0., 0., 0.), p(2., 0., 0.), p(2., 3., 0.)],
        Tolerance::DEFAULT,
    )
    .unwrap();
    let stations =
        [(0., 0.), (1., 0.4), (2., 1.)].map(|(source_parameter, fit_parameter)| CurveFitStation {
            source_parameter,
            fit_parameter,
            point: CurveRef::Polyline(&source)
                .evaluate(source_parameter)
                .unwrap(),
        });
    let fit = try_fit_cubic_curve_at_stations(
        CurveRef::Polyline(&source),
        &stations,
        1e-8,
        0.1,
        Tolerance::DEFAULT,
    )
    .unwrap();
    assert_eq!(fit.curve.knots().iter().filter(|&&k| k == 0.4).count(), 3);
    for t in [0., 0.2, 0.4, 0.7, 1.] {
        let expected = if t <= 0.4 {
            p(5. * t, 0., 0.)
        } else {
            p(2., 5. * t - 2., 0.)
        };
        assert!(
            fit.curve
                .evaluate(t)
                .unwrap()
                .distance_to(expected)
                .unwrap()
                < 2e-13
        );
    }
    for (side, expected) in [
        (ParameterSide::Left, [5., 0., 0.]),
        (ParameterSide::Right, [0., 5., 0.]),
    ] {
        let actual = fit
            .curve
            .evaluate_with_derivative_on_side(0.4, side)
            .unwrap()
            .1
            .to_array();
        assert!(
            actual
                .into_iter()
                .zip(expected)
                .all(|(a, b)| (a - b).abs() < 2e-13)
        );
    }
}

#[test]
fn common_station_fit_keeps_spatial_constraints_in_an_independent_source_domain() {
    let source = NurbsCurve::try_new(
        3,
        vec![p(0., 0., 0.), p(1., 2., 3.), p(3., -1., 1.), p(4., 1., 2.)],
        vec![0., 0., 0., 0., 1., 1., 1., 1.],
    )
    .unwrap()
    .reversed()
    .unwrap()
    .try_reparameterized(-13. ..=29.)
    .unwrap();
    let before = source.clone();
    let stations =
        [(0., 0.), (0.3, 0.35), (0.7, 0.7), (1., 1.)].map(|(fraction, fit_parameter)| {
            let source_parameter = source.parameter_at(fraction).unwrap();
            CurveFitStation {
                source_parameter,
                fit_parameter,
                point: source.evaluate(source_parameter).unwrap(),
            }
        });
    let fit = try_fit_cubic_curve_at_stations(
        CurveRef::NurbsCurve(&source),
        &stations,
        1e-6,
        0.,
        Tolerance::DEFAULT,
    )
    .unwrap();
    for station in stations {
        assert!(
            fit.curve
                .evaluate(station.fit_parameter)
                .unwrap()
                .distance_to(station.point)
                .unwrap()
                < 1e-11
        );
    }
    assert!(
        fit.curve
            .knots()
            .chunk_by(|a, b| a == b)
            .filter(|g| g[0] > 0. && g[0] < 1.)
            .all(|g| g.len() == 1)
    );
    for i in 0..=97 {
        let t = i as Real / 97.;
        let point = fit.curve.evaluate(t).unwrap();
        let parameter = source.closest_parameter(point, Tolerance::DEFAULT).unwrap();
        assert!(
            source
                .evaluate(parameter)
                .unwrap()
                .distance_to(point)
                .unwrap()
                < 1.1e-6
        );
    }
    assert_eq!(source, before);
}

#[test]
fn common_station_fit_rejects_incomplete_invalid_and_unprovably_monotone_maps() {
    let source = LineSegment::try_new(p(0., 0., 0.), p(1., 0., 0.), Tolerance::DEFAULT).unwrap();
    let station = |(s, t)| CurveFitStation {
        source_parameter: s,
        fit_parameter: t,
        point: p(s, 0., 0.),
    };
    for pairs in [
        vec![],
        vec![(0., 0.)],
        vec![(0.1, 0.), (1., 1.)],
        vec![(0., 0.01), (1., 1.)],
        vec![(0., 0.), (1., 0.99)],
        vec![(0., 0.), (0.4, 0.8), (0.7, 0.4), (1., 1.)],
        vec![(0., 0.), (0.4, 0.5), (0.7, 0.5), (1., 1.)],
        vec![(0., 0.), (0.9, 0.1), (1., 1.)],
        vec![(0., 0.), (0.5, Real::NAN), (1., 1.)],
        vec![(0., 0.), (0.5, Real::INFINITY), (1., 1.)],
    ] {
        let stations = pairs.into_iter().map(station).collect::<Vec<_>>();
        assert!(matches!(
            try_fit_cubic_curve_at_stations(
                CurveRef::Line(&source),
                &stations,
                1e-6,
                0.,
                Tolerance::DEFAULT
            ),
            Err(GeometryError::InvalidCurveFitConstraints { .. })
        ));
    }
    assert!(matches!(
        try_fit_cubic_curve_at_stations(
            CurveRef::Line(&source),
            &vec![station((0., 0.)); 65],
            1e-6,
            0.,
            Tolerance::DEFAULT
        ),
        Err(GeometryError::InvalidCurveFitConstraints { .. })
    ));
}

#[test]
fn common_station_fit_declines_an_oversized_source_span_list_before_map_inversion() {
    let source = Polyline3::try_new(
        (0..5000).map(|i| p(i as Real, 0., 0.)).collect(),
        Tolerance::DEFAULT,
    )
    .unwrap();
    let stations = [0., 4999.].map(|source_parameter| CurveFitStation {
        source_parameter,
        fit_parameter: source_parameter / 4999.,
        point: p(source_parameter, 0., 0.),
    });
    assert!(matches!(
        try_fit_cubic_curve_at_stations(
            CurveRef::Polyline(&source),
            &stations,
            1e-6,
            0.1,
            Tolerance::DEFAULT
        ),
        Err(GeometryError::TooManyCurveFitControlPoints {
            maximum: MAX_CURVE_FIT_CONTROL_POINTS
        })
    ));
}
