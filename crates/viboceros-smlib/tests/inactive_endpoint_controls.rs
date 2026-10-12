#![cfg(feature = "native")]
use viboceros_geometry::{NurbsCurve, NurbsSurface, Point3, SurfaceKnotDirection, WeightedPoint3};
use viboceros_smlib::{
    change_curve_degree, change_surface_degree, insert_curve_knot, insert_surface_knot,
};

fn source(start_side: bool, gauge: f64, chart: f64) -> NurbsCurve {
    let mut controls = [
        [99., 99., 99.],
        [0., 0., 0.],
        [1., 3., 1.],
        [4., -1., 1.],
        [6., 2., 0.],
    ]
    .into_iter()
    .zip([1., 0.7, 1.5, 0.8, 1.])
    .map(|(p, w)| WeightedPoint3::try_new(Point3::try_from(p).unwrap(), w * gauge).unwrap())
    .collect::<Vec<_>>();
    let mut knots = [-2., 0., 0., 0., 0.5, 1., 2., 3.]
        .map(|k| k * chart)
        .to_vec();
    if !start_side {
        controls.reverse();
        knots = knots.into_iter().rev().map(|k| -k).collect();
    }
    NurbsCurve::try_new_rational(2, controls, knots).unwrap()
}

fn validate_curve(curve: &NurbsCurve) {
    assert_eq!(
        NurbsCurve::try_new_rational(
            curve.degree(),
            curve.control_points().to_vec(),
            curve.knots().to_vec(),
        )
        .unwrap(),
        *curve
    );
}

#[test]
fn full_order_endpoint_groups_are_extracted_before_curve_degree_changes() {
    for start_side in [true, false] {
        for gauge in [1e-300, 1., 1e300, -1e300] {
            for chart in [2_f64.powi(-360), 1., 2_f64.powi(360)] {
                let source = source(start_side, gauge, chart);
                let before = source.clone();
                let endpoint = if start_side {
                    *source.domain().start()
                } else {
                    *source.domain().end()
                };
                let clamped = insert_curve_knot(&source, endpoint, 3).unwrap();
                assert_eq!(clamped, source.try_insert_knot(endpoint, 3).unwrap());
                assert_eq!(insert_curve_knot(&clamped, endpoint, 3).unwrap(), clamped);
                let retained = if start_side {
                    &source.control_points()[1..]
                } else {
                    &source.control_points()[..4]
                };
                assert_eq!(clamped.control_points(), retained);
                for (degree, deformable) in [(3, false), (3, true), (1, false)] {
                    let actual = change_curve_degree(&source, degree, deformable).unwrap();
                    let expected = source.try_change_degree(degree, deformable).unwrap();
                    assert_eq!(actual.knots(), expected.knots());
                    validate_curve(&actual);
                    for i in 0..=64 {
                        let t = source.parameter_at(i as f64 / 64.).unwrap();
                        assert!(
                            source
                                .evaluate(t)
                                .unwrap()
                                .distance_to(clamped.evaluate(t).unwrap())
                                .unwrap()
                                < 1e-10
                        );
                        assert!(
                            actual
                                .evaluate(t)
                                .unwrap()
                                .distance_to(expected.evaluate(t).unwrap())
                                .unwrap()
                                < 1e-10
                        );
                    }
                }
                assert_eq!(source, before);
            }
        }
    }
}

#[test]
fn insertion_and_noops_keep_full_order_exterior_groups_valid() {
    for start_side in [true, false] {
        let source = source(start_side, 1., 1.);
        for (parameter, target) in [
            (*source.domain().start(), 1),
            (*source.domain().end(), 1),
            (source.parameter_at(0.5).unwrap(), 1),
            (source.parameter_at(0.25).unwrap(), 2),
        ] {
            let actual = insert_curve_knot(&source, parameter, target).unwrap();
            let expected = source.try_insert_knot(parameter, target).unwrap();
            validate_curve(&actual);
            assert_eq!(actual.knots(), expected.knots());
            for i in 0..=64 {
                let t = source.parameter_at(i as f64 / 64.).unwrap();
                assert!(
                    source
                        .evaluate(t)
                        .unwrap()
                        .distance_to(actual.evaluate(t).unwrap())
                        .unwrap()
                        < 1e-10
                );
            }
        }
    }
}

#[test]
fn full_order_endpoint_groups_are_extracted_in_both_tensor_axes() {
    for axis in [SurfaceKnotDirection::U, SurfaceKnotDirection::V] {
        for start_side in [true, false] {
            for gauge in [1e-300, 1., 1e300, -1e300] {
                let curve = source(start_side, gauge, 1.);
                let mut controls = Vec::new();
                let u_axis = axis == SurfaceKnotDirection::U;
                let counts = if u_axis { (5, 2) } else { (2, 5) };
                for j in 0..counts.1 {
                    for i in 0..counts.0 {
                        let (index, other) = if u_axis { (i, j) } else { (j, i) };
                        let p = curve.control_points()[index];
                        controls.push(
                            WeightedPoint3::try_new(
                                Point3::try_new(
                                    p.point().x(),
                                    p.point().y(),
                                    p.point().z() + other as f64 * 3.,
                                )
                                .unwrap(),
                                p.weight() * if other == 0 { 0.8 } else { 1.2 },
                            )
                            .unwrap(),
                        );
                    }
                }
                let linear = vec![0., 0., 1., 1.];
                let (degrees, knots) = if u_axis {
                    ((2, 1), (curve.knots().to_vec(), linear))
                } else {
                    ((1, 2), (linear, curve.knots().to_vec()))
                };
                let source = NurbsSurface::try_new_rational(
                    degrees.0, degrees.1, counts.0, counts.1, controls, knots.0, knots.1,
                )
                .unwrap();
                let before = source.clone();
                let endpoint = if start_side {
                    *curve.domain().start()
                } else {
                    *curve.domain().end()
                };
                let clamped = insert_surface_knot(&source, axis, endpoint, 3).unwrap();
                assert_eq!(clamped.control_points().len(), 8);
                assert_eq!(
                    insert_surface_knot(&clamped, axis, endpoint, 3).unwrap(),
                    clamped
                );
                let requested = if u_axis { (3, 1) } else { (1, 3) };
                let actual =
                    change_surface_degree(&source, requested.0, requested.1, false).unwrap();
                let expected = source
                    .try_change_degree(requested.0, requested.1, false)
                    .unwrap();
                assert_eq!(actual.knots_u(), expected.knots_u());
                assert_eq!(actual.knots_v(), expected.knots_v());
                assert_eq!(
                    NurbsSurface::try_new_rational(
                        actual.degree_u(),
                        actual.degree_v(),
                        actual.control_point_count_u(),
                        actual.control_point_count_v(),
                        actual.control_points().to_vec(),
                        actual.knots_u().to_vec(),
                        actual.knots_v().to_vec(),
                    )
                    .unwrap(),
                    actual
                );
                for i in 0..=16 {
                    for j in 0..=16 {
                        let u = source.parameter_at_u(i as f64 / 16.).unwrap();
                        let v = source.parameter_at_v(j as f64 / 16.).unwrap();
                        let point = source.evaluate(u, v).unwrap();
                        for result in [&clamped, &actual, &expected] {
                            assert!(
                                point.distance_to(result.evaluate(u, v).unwrap()).unwrap() < 1e-10
                            );
                        }
                    }
                }
                assert_eq!(source, before);
            }
        }
    }
}

#[test]
fn inactive_prefixes_and_suffixes_are_removed_at_native_degree_boundaries() {
    for degree in [1, 3, 17] {
        let mut controls = vec![
            WeightedPoint3::try_new(Point3::try_new(99., 99., 99.).unwrap(), 1.)
                .unwrap();
            degree
        ];
        controls.extend((0..=degree).map(|i| {
            WeightedPoint3::try_new(
                Point3::try_new(i as f64, (i % 3) as f64, (i % 2) as f64).unwrap(),
                1. + i as f64 / 10.,
            )
            .unwrap()
        }));
        let mut knots = (0..degree)
            .map(|i| i as f64 - degree as f64)
            .collect::<Vec<_>>();
        knots.extend(std::iter::repeat_n(0., degree + 1));
        knots.extend(std::iter::repeat_n(1., degree + 1));
        let original = NurbsCurve::try_new_rational(degree, controls, knots).unwrap();
        for (curve, endpoint, retained) in [
            (
                original.clone(),
                0.,
                original.control_points()[degree..].to_vec(),
            ),
            (
                original.reversed().unwrap(),
                0.,
                original.control_points()[degree..]
                    .iter()
                    .copied()
                    .rev()
                    .collect(),
            ),
        ] {
            let result = insert_curve_knot(&curve, endpoint, degree + 1).unwrap();
            assert_eq!(result.control_points(), retained);
            assert_eq!(result.control_points().len(), degree + 1);
            validate_curve(&result);
            assert_eq!(
                insert_curve_knot(&result, endpoint, degree + 1).unwrap(),
                result
            );
            for i in 0..=32 {
                let t = curve.parameter_at(i as f64 / 32.).unwrap();
                assert!(
                    curve
                        .evaluate(t)
                        .unwrap()
                        .distance_to(result.evaluate(t).unwrap())
                        .unwrap()
                        < 1e-10
                );
            }
        }
    }
}
