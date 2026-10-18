#![cfg(feature = "native")]
use viboceros_geometry::{NurbsCurve, NurbsSurface, Point3, SurfaceKnotDirection, WeightedPoint3};
use viboceros_smlib::{curve_bezier_spans, surface_single_span_patches};

fn controls(weights: &[f64]) -> Vec<WeightedPoint3> {
    weights
        .iter()
        .enumerate()
        .map(|(i, &w)| {
            WeightedPoint3::try_new(
                Point3::try_new((i % 4) as f64, (i / 4) as f64, (i % 3) as f64).unwrap(),
                w,
            )
            .unwrap()
        })
        .collect()
}

fn compare_curve(source: &NurbsCurve) {
    let before = source.clone();
    let actual = curve_bezier_spans(source).unwrap();
    let expected = source.try_bezier_spans().unwrap();
    assert_eq!(actual.len(), expected.len());
    for (a, e) in actual.iter().zip(&expected) {
        assert_eq!(a.knots(), e.knots());
        assert_eq!(a.control_points().len(), e.control_points().len());
        for (x, y) in a.control_points().iter().zip(e.control_points()) {
            assert!(x.point().distance_to(y.point()).unwrap() < 1e-10);
            assert!((x.weight() / y.weight() - 1.).abs() < 1e-12);
        }
        for i in 0..=32 {
            let t = a.parameter_at(i as f64 / 32.).unwrap();
            assert!(
                a.evaluate(t)
                    .unwrap()
                    .distance_to(e.evaluate(t).unwrap())
                    .unwrap()
                    < 1e-10
            );
        }
    }
    assert_eq!(*source, before);
}

#[test]
fn native_curve_decomposition_preserves_charts_gauges_full_order_breaks_and_inactive_controls() {
    for chart in [2_f64.powi(-360), 1., 2_f64.powi(360)] {
        for gauge in [1e-300, 1., 1e300, -1e300] {
            for (weights, knots) in [
                (vec![1., 0.7, 1.4, 0.8], vec![-3., -2., -1., 1., 4., 5., 6.]),
                (
                    vec![1., 0.7, 1.4, 0.8, 1.2, 1.],
                    vec![0., 0., 0., 0.5, 0.5, 0.5, 1., 1., 1.],
                ),
                (
                    vec![1., 0.7, 1.4, 0.8, 1.2],
                    vec![-2., 0., 0., 0., 0.5, 1., 2., 3.],
                ),
            ] {
                let source = NurbsCurve::try_new_rational(
                    2,
                    controls(&weights.iter().map(|w| w * gauge).collect::<Vec<_>>()),
                    knots.iter().map(|k| k * chart).collect(),
                )
                .unwrap();
                compare_curve(&source);
                compare_curve(&source.reversed().unwrap());
            }
        }
    }
}

fn compare_surface(source: &NurbsSurface, direction: SurfaceKnotDirection) {
    let before = source.clone();
    let actual = surface_single_span_patches(source, direction).unwrap();
    let expected = source.try_single_span_patches(direction).unwrap();
    assert_eq!(actual.len(), expected.len());
    for (a, e) in actual.iter().zip(&expected) {
        assert_eq!(a.knots_u(), e.knots_u());
        assert_eq!(a.knots_v(), e.knots_v());
        assert_eq!(a.control_points().len(), e.control_points().len());
        for (x, y) in a.control_points().iter().zip(e.control_points()) {
            assert!(x.point().distance_to(y.point()).unwrap() < 1e-10);
            assert!((x.weight() / y.weight() - 1.).abs() < 1e-12);
        }
        for i in 0..=8 {
            for j in 0..=8 {
                let u = a.parameter_at_u(i as f64 / 8.).unwrap();
                let v = a.parameter_at_v(j as f64 / 8.).unwrap();
                assert!(
                    a.evaluate(u, v)
                        .unwrap()
                        .distance_to(e.evaluate(u, v).unwrap())
                        .unwrap()
                        < 1e-10
                );
            }
        }
    }
    assert_eq!(*source, before);
}

#[test]
fn native_tensor_decomposition_preserves_order_and_unselected_full_bases() {
    for chart in [2_f64.powi(-360), 1., 2_f64.powi(360)] {
        for gauge in [1e-300, 1., 1e300, -1e300] {
            let source = NurbsSurface::try_new_rational(
                2,
                2,
                4,
                4,
                controls(
                    &(0..16)
                        .map(|i| (0.7 + (i % 5) as f64 / 4.) * gauge)
                        .collect::<Vec<_>>(),
                ),
                [-3., -2., -1., 1., 4., 5., 6.].map(|k| k * chart).to_vec(),
                [8., 9., 10., 13., 18., 19., 20.]
                    .map(|k| k * chart)
                    .to_vec(),
            )
            .unwrap();
            for direction in [
                SurfaceKnotDirection::U,
                SurfaceKnotDirection::V,
                SurfaceKnotDirection::Both,
            ] {
                compare_surface(&source, direction);
            }
        }
    }
}

#[test]
fn native_tensor_decomposition_keeps_intermediate_zero_weights_homogeneous() {
    let source = NurbsSurface::try_new_rational(
        2,
        2,
        3,
        3,
        controls(&[1., -1., 3., 3., 3., 3., 3., 3., 3.]),
        vec![-2., -1., 0., 1., 2., 3.],
        vec![-2., -1., 0., 1., 2., 3.],
    )
    .unwrap();
    compare_surface(&source, SurfaceKnotDirection::Both);
    assert!(surface_single_span_patches(&source, SurfaceKnotDirection::U).is_err());
}

#[test]
fn full_order_tensor_breaks_use_native_tensor_decomposition_without_buffer_underflow() {
    let source = NurbsSurface::try_new_rational(
        2,
        2,
        6,
        6,
        controls(
            &(0..36)
                .map(|i| 0.7 + (i % 5) as f64 / 4.)
                .collect::<Vec<_>>(),
        ),
        vec![0., 0., 0., 0.5, 0.5, 0.5, 1., 1., 1.],
        vec![0., 0., 0., 0.5, 0.5, 0.5, 1., 1., 1.],
    )
    .unwrap();
    for direction in [
        SurfaceKnotDirection::U,
        SurfaceKnotDirection::V,
        SurfaceKnotDirection::Both,
    ] {
        compare_surface(&source, direction);
    }
}

#[test]
fn directional_strips_retain_extreme_independent_control_line_gauges() {
    let source = NurbsSurface::try_new_rational(
        2,
        1,
        3,
        2,
        (0..6)
            .map(|i| {
                WeightedPoint3::try_new(
                    Point3::try_new((i % 3) as f64 * 2., (i / 3) as f64, 0.).unwrap(),
                    if i < 3 { 1e-280 } else { 1e280 },
                )
                .unwrap()
            })
            .collect(),
        vec![-2., -1., 0., 1., 2., 3.],
        vec![10., 10., 18., 18.],
    )
    .unwrap();
    let actual = surface_single_span_patches(&source, SurfaceKnotDirection::U).unwrap();
    assert_eq!(actual.len(), 1);
    for (i, p) in actual[0].control_points().iter().enumerate() {
        assert_eq!(p.weight(), if i < 3 { 1e-280 } else { 1e280 });
        assert_eq!(
            p.point().to_array(),
            [1. + (i % 3) as f64, (i / 3) as f64, 0.]
        );
    }
    assert_eq!(actual[0].knots_v(), source.knots_v());
}

#[test]
fn local_origins_preserve_large_world_placement_exactly() {
    let source = NurbsCurve::try_new_rational(
        2,
        (0..3)
            .map(|i| {
                WeightedPoint3::try_new(Point3::try_new(1e15 + i as f64, i as f64, 0.).unwrap(), 1.)
                    .unwrap()
            })
            .collect(),
        vec![-2., -1., 0., 1., 2., 3.],
    )
    .unwrap();
    let actual = curve_bezier_spans(&source).unwrap();
    assert_eq!(actual.len(), 1);
    assert_eq!(actual[0].control_points()[0].point().x(), 1e15 + 0.5);
    assert_eq!(actual[0].control_points()[1].point().x(), 1e15 + 1.);
    assert_eq!(actual[0].control_points()[2].point().x(), 1e15 + 1.5);
}

#[test]
fn native_decomposition_retains_narrow_positive_interior_spans() {
    for chart in [2_f64.powi(-360), 1., 2_f64.powi(360)] {
        let source = NurbsCurve::try_new_rational(
            2,
            controls(&[1., 0.7, 1.4, 0.8, 1.]),
            [0., 0., 0., 0.5, 0.5 + 1e-14, 1., 1., 1.]
                .map(|k| k * chart)
                .to_vec(),
        )
        .unwrap();
        compare_curve(&source);
    }
}

#[test]
fn distant_inactive_controls_do_not_choose_the_extraction_origin() {
    for knots in [
        vec![-2., 0., 0., 0., 0.5, 1., 2., 3.],
        vec![-2., -1., 0., 0., 0.5, 1., 2., 3.],
    ] {
        let source = NurbsCurve::try_new_rational(
            2,
            [
                Point3::try_new(1e308, 1e308, 1e308).unwrap(),
                Point3::try_new(0., 0., 0.).unwrap(),
                Point3::try_new(1., 3., 1.).unwrap(),
                Point3::try_new(4., -1., 1.).unwrap(),
                Point3::try_new(6., 2., 0.).unwrap(),
            ]
            .into_iter()
            .map(|p| WeightedPoint3::try_new(p, 1.).unwrap())
            .collect(),
            knots,
        )
        .unwrap();
        compare_curve(&source);
    }
}

#[test]
fn coordinate_conditioning_preserves_small_controls_beside_a_large_active_control() {
    let source = NurbsCurve::try_new_rational(
        2,
        [
            ([1e308, 0., 0.], 1e-300),
            ([1., 4., 0.], 1.),
            ([2., 5., 0.], 1.),
        ]
        .into_iter()
        .map(|(p, w)| WeightedPoint3::try_new(Point3::try_from(p).unwrap(), w).unwrap())
        .collect(),
        vec![0., 0., 0., 1., 1., 1.],
    )
    .unwrap();
    let actual = curve_bezier_spans(&source).unwrap();
    assert_eq!(
        actual[0].control_points()[1..],
        source.control_points()[1..]
    );
    assert!((actual[0].control_points()[0].point().x() / 1e308 - 1.).abs() < 1e-12);
    assert_eq!(actual[0].control_points()[0].weight(), 1e-300);
}
