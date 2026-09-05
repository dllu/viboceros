use super::*;
use crate::ParameterSide::{Left, Right};

pub(super) fn check_cubic_lift_jets(source: &NurbsSurface, fitted: &NurbsSurface) {
    for i in 0..=4 {
        for j in 0..=4 {
            let u = source.parameter_at_u(i as Real / 4.0).unwrap();
            let v = source.parameter_at_v(j as Real / 4.0).unwrap();
            for su in [Left, Right] {
                for sv in [Left, Right] {
                    let s = source
                        .evaluate_with_second_derivatives_on_sides(u, v, su, sv)
                        .unwrap();
                    let f = fitted
                        .evaluate_with_second_derivatives_on_sides(u, v, su, sv)
                        .unwrap();
                    let ax = 2.0 * s.point.x() + 0.25 * s.point.y();
                    let ay = 0.25 * s.point.x() + 3.0 * s.point.y().powi(2);
                    let first = |d: Vector3| [d.x(), d.y(), d.z() + ax * d.x() + ay * d.y()];
                    let second = |a: Vector3, b: Vector3, d: Vector3| {
                        let mut value = first(d);
                        value[2] += 2.0 * a.x() * b.x()
                            + 0.25 * (a.x() * b.y() + a.y() * b.x())
                            + 6.0 * s.point.y() * a.y() * b.y();
                        value
                    };
                    for (a, b) in [
                        (f.derivative_u, first(s.derivative_u)),
                        (f.derivative_v, first(s.derivative_v)),
                        (
                            f.derivative_uu,
                            second(s.derivative_u, s.derivative_u, s.derivative_uu),
                        ),
                        (
                            f.derivative_uv,
                            second(s.derivative_u, s.derivative_v, s.derivative_uv),
                        ),
                        (
                            f.derivative_vv,
                            second(s.derivative_v, s.derivative_v, s.derivative_vv),
                        ),
                    ] {
                        for (a, b) in a.to_array().into_iter().zip(b) {
                            assert!((a - b).abs() < 5e-11, "jet {a} != {b}");
                        }
                    }
                }
            }
        }
    }
}

fn weighted_patch(weights: [Real; 4]) -> NurbsSurface {
    let source = unit_patch();
    NurbsSurface::try_new_rational(
        1,
        1,
        2,
        2,
        source
            .control_points()
            .iter()
            .zip(weights)
            .map(|(p, w)| WeightedPoint3::try_new(p.point(), w).unwrap())
            .collect(),
        source.knots_u().to_vec(),
        source.knots_v().to_vec(),
    )
    .unwrap()
}

fn weight_field(source: &NurbsSurface) -> NurbsSurface {
    NurbsSurface::try_new_rational(
        source.degree_u(),
        source.degree_v(),
        source.control_point_count_u(),
        source.control_point_count_v(),
        source
            .control_points()
            .iter()
            .map(|cp| WeightedPoint3::try_new(p(cp.weight(), 0.0, 0.0), 1.0).unwrap())
            .collect(),
        source.knots_u().to_vec(),
        source.knots_v().to_vec(),
    )
    .unwrap()
}

#[test]
fn composition_keeps_cubed_denominator_and_ignores_common_weight_sign_and_scale() {
    for (index, scale) in [1.0, -1.0, 1e-200, -1e200].into_iter().enumerate() {
        let source = weighted_patch([1.0, 2.0, 3.0, 5.0].map(|w| w * scale))
            .try_change_degree(1 + index % 3, 3 - index % 3, false)
            .unwrap();
        let fitted = Cubic
            .morph_nurbs_surface(&source, Tolerance::DEFAULT)
            .unwrap();
        assert_eq!(fitted.degree_u(), source.degree_u() * 3);
        assert_eq!(fitted.degree_v(), source.degree_v() * 3);
        check_image(&source, &fitted, &Cubic, 2e-12);
        let expected_weights = denominator::source_weights(&source).unwrap();
        let actual_weights = weight_field(&fitted);
        for j in 0..=12 {
            for i in 0..=12 {
                let u = source.parameter_at_u(i as Real / 12.0).unwrap();
                let v = source.parameter_at_v(j as Real / 12.0).unwrap();
                let expected = expected_weights.evaluate(u, v).unwrap().x().powi(3);
                let actual = actual_weights.evaluate(u, v).unwrap().x();
                assert!((expected - actual).abs() <= 2e-13);
            }
        }
    }
}

#[test]
fn rational_composition_retains_four_independent_limits_at_crossing_full_order_knots() {
    let source = NurbsSurface::try_new_rational(
        2,
        2,
        6,
        6,
        [0.0, 1.0, 2.0, 5.0, 6.0, 7.0]
            .into_iter()
            .enumerate()
            .flat_map(|(j, y)| {
                [0.0, 0.5, 1.0, 3.0, 3.5, 4.0]
                    .into_iter()
                    .enumerate()
                    .map(move |(i, x)| {
                        WeightedPoint3::try_new(p(x, y, 0.0), 1.0 + (i * 7 + j) as Real * 0.01)
                            .unwrap()
                    })
            })
            .collect(),
        vec![0.0, 0.0, 0.0, 1.0, 1.0, 1.0, 2.0, 2.0, 2.0],
        vec![0.0, 0.0, 0.0, 1.0, 1.0, 1.0, 2.0, 2.0, 2.0],
    )
    .unwrap();
    let fitted = Cubic
        .morph_nurbs_surface(&source, Tolerance::DEFAULT)
        .unwrap();
    assert_eq!([fitted.degree_u(), fitted.degree_v()], [6, 6]);
    assert_eq!(fitted.knots_u().iter().filter(|&&k| k == 1.0).count(), 7);
    for su in [Left, Right] {
        for sv in [Left, Right] {
            let expected = Cubic
                .morph_point(source.evaluate_on_sides(1.0, 1.0, su, sv).unwrap())
                .unwrap();
            let actual = fitted.evaluate_on_sides(1.0, 1.0, su, sv).unwrap();
            assert!(actual.distance_to(expected).unwrap() <= 1e-11);
        }
    }
    check_image(&source, &fitted, &Cubic, 1e-11);
}

#[test]
fn noncubic_map_rejects_the_composition_candidate_and_uses_adaptive_fitting() {
    struct Wave;
    impl PointMorph for Wave {
        fn morph_point(&self, p: Point3) -> Result<Point3, GeometryError> {
            Point3::try_new(p.x(), p.y(), (8.0 * p.x()).sin())
        }
    }
    let source = weighted_patch([1.0, 2.0, 1.0, 2.0])
        .try_change_degree(2, 1, false)
        .unwrap();
    let candidate = tensor::rational_candidate(
        &mut |[u, v], [su, sv]| Wave.morph_point(source.evaluate_on_sides(u, v, su, sv)?),
        &source,
        MAX_MORPH_SURFACE_AXIS_CONTROLS,
    )
    .unwrap()
    .unwrap();
    let exact = Wave
        .morph_point(source.evaluate(-1.3, 3.2).unwrap())
        .unwrap();
    assert!(
        candidate
            .evaluate(-1.3, 3.2)
            .unwrap()
            .distance_to(exact)
            .unwrap()
            > 1e-4
    );
    let fitted = Wave
        .morph_nurbs_surface(&source, Tolerance::try_new(1e-6, 1e-12, 1e-10).unwrap())
        .unwrap();
    assert_eq!([fitted.degree_u(), fitted.degree_v()], [3, 3]);
    assert!(fitted.control_points().iter().all(|c| c.weight() == 1.0));
    check_image(&source, &fitted, &Wave, 1e-6);
}

#[test]
fn unsuitable_or_oversized_composition_spaces_do_not_sample_the_map() {
    let weighted = weighted_patch([1.0, 2.0, 3.0, 5.0]);
    for source in [
        unit_patch(),
        weighted_patch([1.0, -0.1, 2.0, 3.0]),
        weighted_patch([Real::from_bits(1), 1e300, 1.0, 1.0]),
        weighted.try_change_degree(4, 4, false).unwrap(),
    ] {
        assert!(
            tensor::rational_candidate(
                &mut |_, _| panic!("not a supported candidate space"),
                &source,
                MAX_MORPH_SURFACE_AXIS_CONTROLS
            )
            .unwrap()
            .is_none()
        );
    }
    assert!(
        tensor::rational_candidate(
            &mut |_, _| panic!("candidate exceeds axis control limit"),
            &weighted,
            3
        )
        .unwrap()
        .is_none()
    );
}

#[test]
fn candidate_mapping_errors_propagate_without_a_retry() {
    let mut calls = 0;
    let result = tensor::rational_candidate(
        &mut |_, _| {
            calls += 1;
            Err(GeometryError::Degenerate {
                context: "test map failure",
            })
        },
        &weighted_patch([1.0, 2.0, 3.0, 5.0]),
        MAX_MORPH_SURFACE_AXIS_CONTROLS,
    );
    assert!(matches!(
        result,
        Err(GeometryError::Degenerate {
            context: "test map failure"
        })
    ));
    assert_eq!(calls, 1);
}

#[test]
fn rational_interpolation_keeps_constant_large_coordinate_targets_exact() {
    let expected = p(1e12, -2e12, 3e12);
    let fitted = tensor::rational_candidate(
        &mut |_, _| Ok(expected),
        &weighted_patch([1.0, 2.0, 3.0, 5.0])
            .try_change_degree(2, 3, false)
            .unwrap(),
        MAX_MORPH_SURFACE_AXIS_CONTROLS,
    )
    .unwrap()
    .unwrap();
    assert!(
        fitted
            .control_points()
            .iter()
            .all(|c| c.point() == expected)
    );
}
