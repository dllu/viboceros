#![cfg(feature = "native")]
use viboceros_geometry::{
    GeometryError, NurbsCurve, NurbsSurface, Point3, SurfaceKnotDirection, WeightedPoint3,
};
use viboceros_smlib::{Error, make_curve_periodic, make_surface_periodic};

fn control(point: [f64; 3], weight: f64) -> WeightedPoint3 {
    WeightedPoint3::try_new(Point3::try_from(point).unwrap(), weight).unwrap()
}
fn rational_loop(gauge: f64, coordinate_scale: f64) -> NurbsCurve {
    NurbsCurve::try_new_rational(
        2,
        [
            ([-1., 0., 0.], 0.75),
            ([2., -2., 0.], 1.5),
            ([4., 2., 1.], 0.6),
            ([0., 4., 0.], 1.8),
            ([-1., 0., 0.], 0.75),
        ]
        .map(|(p, w)| control([p[0], p[1], p[2] * coordinate_scale], w * gauge))
        .to_vec(),
        vec![0., 0., 0., 2., 5., 8., 8., 8.],
    )
    .unwrap()
}
fn compare(a: &NurbsCurve, b: &NurbsCurve) {
    assert_eq!(a.degree(), b.degree());
    assert_eq!(a.knots(), b.knots());
    assert_eq!(a.control_points().len(), b.control_points().len());
    for (a, b) in a.control_points().iter().zip(b.control_points()) {
        assert!((a.weight() - b.weight()).abs() < 1e-9);
        assert!(a.point().distance_to(b.point()).unwrap() < 1e-8);
    }
}

#[test]
fn rational_periodic_constraints_match_independent_quadratic_coefficients() {
    // These coefficients are the independently expanded quadratic source
    // basis at the three target Greville sites; no evaluator supplies them.
    let stations = [
        (1., [0.25, 0.65, 0.1, 0., 0.]),
        (3.5, [0., 0.15, 0.725, 0.125, 0.]),
        (6.5, [0., 0., 0.125, 0.625, 0.25]),
    ];
    let reference = rational_loop(1., 1.);
    for gauge in [1., -1., 1e280, -1e280, 1e-280, -1e-280] {
        for scale in [1., 1e-280] {
            let source = rational_loop(gauge, scale);
            let before = source.clone();
            let output = make_curve_periodic(&source, true).unwrap();
            assert_eq!(source, before);
            assert!(output.is_periodic());
            assert_eq!(output.knots(), &[-3., -3., 0., 2., 5., 8., 10., 10.]);
            assert_eq!(output.control_points()[..2], output.control_points()[3..]);
            for (t, basis) in stations {
                let mut expected = [0.; 3];
                let mut weight = 0.;
                for (b, c) in basis.into_iter().zip(reference.control_points()) {
                    weight += b * c.weight();
                    for (value, coordinate) in expected.iter_mut().zip(c.point().to_array()) {
                        *value += b * c.weight() * coordinate;
                    }
                }
                let actual = output.evaluate(t).unwrap();
                for (axis, (a, e)) in actual.to_array().into_iter().zip(expected).enumerate() {
                    let coordinate_scale = if axis == 2 { scale } else { 1. };
                    assert!((a / coordinate_scale - e / weight).abs() < 1e-12);
                }
            }
            let start = output.evaluate_with_derivative(0.).unwrap();
            let end = output.evaluate_with_derivative(8.).unwrap();
            for (axis, (a, b)) in start
                .1
                .to_array()
                .into_iter()
                .zip(end.1.to_array())
                .enumerate()
            {
                let coordinate_scale = if axis == 2 { scale } else { 1. };
                assert!((a / coordinate_scale - b / coordinate_scale).abs() < 1e-12);
            }
        }
    }
}

#[test]
fn nonuniform_periodic_phase_and_supported_degrees_keep_complete_nets() {
    for degree in 2..=17 {
        let count = 2 * degree + 3;
        let points = (0..count)
            .map(|i| {
                let angle = std::f64::consts::TAU * (i % (count - 1)) as f64 / (count - 1) as f64;
                Point3::try_new(angle.cos(), angle.sin(), 0.1 * (2. * angle).sin()).unwrap()
            })
            .collect();
        let spans = count - degree;
        let breaks = (0..=spans)
            .map(|i| (i as f64).powi(2) / spans as f64)
            .collect::<Vec<_>>();
        let knots = [vec![0.; degree], breaks, vec![spans as f64; degree]].concat();
        let source = NurbsCurve::try_new(degree, points, knots).unwrap();
        for smooth in [false, true] {
            let output = make_curve_periodic(&source, smooth).unwrap();
            compare(&output, &source.try_make_periodic(smooth).unwrap());
            assert!(output.is_periodic());
        }
    }
    // The first in-domain Greville site is 3.033..., far from the start.
    let source = NurbsCurve::try_new(
        3,
        [
            [1., 0., 0.],
            [4., -1., 0.],
            [6., 2., 1.],
            [4., 5., 0.],
            [0., 4., -1.],
            [-2., 1., 0.],
            [1., 0., 0.],
        ]
        .map(|p| Point3::try_from(p).unwrap())
        .to_vec(),
        vec![0., 0., 0., 0., 0.1, 9., 9.5, 10., 10., 10., 10.],
    )
    .unwrap();
    let output = make_curve_periodic(&source, true).unwrap();
    compare(&output, &source.try_make_periodic(true).unwrap());
    for t in [9.1 / 3., 6.2, 9.5, 29.6 / 3.] {
        assert!(
            output
                .evaluate(t)
                .unwrap()
                .distance_to(source.evaluate(t).unwrap())
                .unwrap()
                < 1e-11
        );
    }
}

#[test]
fn large_periodic_bands_preserve_complete_uniform_and_nonuniform_nets() {
    for degree in [2, 3, 7, 11, 17] {
        for unique in [64, 256] {
            let count = unique + degree;
            let points = (0..count)
                .map(|i| {
                    let t = std::f64::consts::TAU * (i % (count - 1)) as f64 / (count - 1) as f64;
                    Point3::try_new(t.cos(), t.sin(), 0.1 * (2. * t).sin()).unwrap()
                })
                .collect::<Vec<_>>();
            for nonuniform in [false, true] {
                let breaks = (0..=unique)
                    .map(|i| {
                        if nonuniform {
                            (i as f64).powi(2) / unique as f64
                        } else {
                            i as f64
                        }
                    })
                    .collect::<Vec<_>>();
                let knots = [vec![0.; degree], breaks, vec![unique as f64; degree]].concat();
                let source = NurbsCurve::try_new(degree, points.clone(), knots).unwrap();
                let output = make_curve_periodic(&source, true).unwrap();
                compare(&output, &source.try_make_periodic(true).unwrap());
                assert!(output.is_periodic());
            }
        }
    }
}

#[test]
fn both_surface_axes_interpolate_separable_rational_constraints() {
    let x = [-1., 2., 4., 0., -1.];
    let y = [0., 2., -1., 3., 0.];
    let weights = [0.75, 1.5, 0.6, 1.8, 0.75];
    let controls = (0..5)
        .flat_map(|v| {
            (0..5).map(move |u| control([x[u], y[v], 0.2 * x[u] * y[v]], weights[u] * weights[v]))
        })
        .collect();
    let source = NurbsSurface::try_new_rational(
        2,
        2,
        5,
        5,
        controls,
        vec![0., 0., 0., 2., 5., 8., 8., 8.],
        vec![0., 0., 0., 2., 5., 8., 8., 8.],
    )
    .unwrap();
    let before = source.clone();
    let sites = [
        (1., [0.25, 0.65, 0.1, 0., 0.]),
        (3.5, [0., 0.15, 0.725, 0.125, 0.]),
        (6.5, [0., 0., 0.125, 0.625, 0.25]),
    ];
    for direction in [
        SurfaceKnotDirection::U,
        SurfaceKnotDirection::V,
        SurfaceKnotDirection::Both,
    ] {
        let result = make_surface_periodic(&source, direction, true).unwrap();
        let portable = source.try_make_periodic(direction, true).unwrap();
        assert_eq!(result.knots_u(), portable.knots_u());
        assert_eq!(result.knots_v(), portable.knots_v());
        for (a, b) in result
            .control_points()
            .iter()
            .zip(portable.control_points())
        {
            assert!(a.point().distance_to(b.point()).unwrap() < 1e-12);
            assert!((a.weight() - b.weight()).abs() < 1e-12);
        }
        for (u, bu) in sites {
            for (v, bv) in sites {
                let rational = |coords: [f64; 5], basis: [f64; 5]| {
                    let denominator: f64 = basis.iter().zip(weights).map(|(b, w)| b * w).sum();
                    basis
                        .iter()
                        .zip(weights)
                        .zip(coords)
                        .map(|((b, w), x)| b * w * x)
                        .sum::<f64>()
                        / denominator
                };
                let (px, py) = (rational(x, bu), rational(y, bv));
                let expected = Point3::try_new(px, py, 0.2 * px * py).unwrap();
                assert!(
                    result
                        .evaluate(u, v)
                        .unwrap()
                        .distance_to(expected)
                        .unwrap()
                        < 1e-12
                );
            }
        }
    }
    assert_eq!(source, before);
}

#[test]
fn periodic_admission_and_portable_high_degree_fallback_are_preserved() {
    let short = NurbsCurve::try_new(
        2,
        [[0., 0., 0.], [5., -2., 0.], [4., 6., 1.], [0., 0., 0.]]
            .map(|p| Point3::try_from(p).unwrap())
            .to_vec(),
        vec![0., 0., 0., 1., 2., 2., 2.],
    )
    .unwrap();
    assert!(matches!(
        make_curve_periodic(&short, true),
        Err(Error::Geometry(
            GeometryError::InsufficientSmoothPeriodicControlPoints { .. }
        ))
    ));
    let open = NurbsCurve::try_new(
        2,
        [[0., 0., 0.], [1., 1., 0.], [2., 0., 0.]]
            .map(|p| Point3::try_from(p).unwrap())
            .to_vec(),
        vec![0., 0., 0., 1., 1., 1.],
    )
    .unwrap();
    assert!(matches!(
        make_curve_periodic(&open, true),
        Err(Error::Geometry(GeometryError::PeriodicCurveMustBeClosed))
    ));
    let source = rational_loop(1., 1.).try_change_degree(18, false).unwrap();
    assert_eq!(
        make_curve_periodic(&source, true).unwrap(),
        source.try_make_periodic(true).unwrap()
    );
    let periodic = make_curve_periodic(&rational_loop(1., 1.), true).unwrap();
    assert_eq!(make_curve_periodic(&periodic, true).unwrap(), periodic);
}
