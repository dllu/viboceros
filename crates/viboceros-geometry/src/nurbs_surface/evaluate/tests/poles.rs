use super::*;
use crate::{GeometryError, ParameterSide, SurfaceJet2};

fn patch(weights: [f64; 2], end: f64, constant: bool) -> NurbsSurface {
    NurbsSurface::try_new_rational(
        1,
        1,
        2,
        2,
        [0., 1.]
            .into_iter()
            .flat_map(|y| {
                weights.into_iter().enumerate().map(move |(i, w)| {
                    let point = if constant {
                        p(3., -4., 5.)
                    } else {
                        p(i as f64, y, 0.)
                    };
                    WeightedPoint3::try_new(point, w).unwrap()
                })
            })
            .collect(),
        vec![0., 0., end, end],
        vec![0., 0., 1., 1.],
    )
    .unwrap()
}

fn pole<T: std::fmt::Debug>(result: Result<T, GeometryError>) {
    assert!(
        matches!(result, Err(GeometryError::ZeroWeightAtParameter)),
        "{result:?}"
    );
}

fn zero_jet(jet: SurfaceJet2) {
    assert_eq!(jet.point, p(3., -4., 5.));
    for d in [
        jet.derivative_u,
        jet.derivative_v,
        jet.derivative_uu,
        jet.derivative_uv,
        jet.derivative_vv,
    ] {
        assert_eq!(d.to_array(), [0.; 3]);
    }
}

#[test]
fn mixed_weight_surface_poles_cannot_hide_in_rounded_interpolation() {
    // W(u,v) = 1 - 2u on [0, 1.5] × [0, 1]. Computing alpha=u/1.5
    // first rounds the pole's 1/3 and can leave a nonzero floating-point W.
    for sign in [-1., 1.] {
        for constant in [false, true] {
            let source = patch([sign, -2. * sign], 1.5, constant);
            for swapped in [false, true] {
                let surface = if swapped {
                    source.try_swapped_uv().unwrap()
                } else {
                    source.clone()
                };
                let [u, v] = if swapped { [0.25, 0.5] } else { [0.5, 0.25] };
                pole(surface.evaluate(u, v));
                pole(surface.evaluate_with_derivatives(u, v));
                pole(surface.evaluate_with_second_derivatives(u, v));
                pole(surface.evaluate_extended(u, v));
                pole(surface.evaluate_extended_with_derivatives(u, v));
                pole(surface.evaluate_extended_with_second_derivatives(u, v));
                pole(surface.normal_at(u, v, Tolerance::DEFAULT));
                for su in [ParameterSide::Left, ParameterSide::Right] {
                    for sv in [ParameterSide::Left, ParameterSide::Right] {
                        pole(surface.evaluate_on_sides(u, v, su, sv));
                        pole(surface.evaluate_with_derivatives_on_sides(u, v, su, sv));
                        pole(surface.evaluate_with_second_derivatives_on_sides(u, v, su, sv));
                    }
                }
                let mut visited = false;
                surface.for_each_grid_point(&[u], &[v], |i, j, result| {
                    assert_eq!((i, j), (0, 0));
                    pole(result);
                    visited = true;
                });
                assert!(visited);
            }
        }
    }
}

#[test]
fn mixed_weight_surface_jets_are_finite_on_both_sides_of_a_pole() {
    // x = -4u / (3(1-2u)), x' = -4 / (3(1-2u)^2),
    // x'' = -16 / (3(1-2u)^3); y=v. This is independent of de Boor.
    for sign in [-1., 1.] {
        let source = patch([sign, -2. * sign], 1.5, false);
        for swapped in [false, true] {
            let surface = if swapped {
                source.try_swapped_uv().unwrap()
            } else {
                source.clone()
            };
            for (t, x, xx) in [(0.25, -2. / 3., -128. / 3.), (0.75, 2., 128. / 3.)] {
                let [u, v] = if swapped { [0.25, t] } else { [t, 0.25] };
                let jet = surface.evaluate_with_second_derivatives(u, v).unwrap();
                assert_eq!(jet.point, p(x, 0.25, 0.));
                let (d, dd, other, other_dd) = if swapped {
                    (
                        jet.derivative_v,
                        jet.derivative_vv,
                        jet.derivative_u,
                        jet.derivative_uu,
                    )
                } else {
                    (
                        jet.derivative_u,
                        jet.derivative_uu,
                        jet.derivative_v,
                        jet.derivative_vv,
                    )
                };
                assert_eq!(d.to_array(), [-16. / 3., 0., 0.]);
                assert_eq!(dd.to_array(), [xx, 0., 0.]);
                assert_eq!(other.to_array(), [0., 1., 0.]);
                assert_eq!(other_dd.to_array(), [0.; 3]);
                assert_eq!(jet.derivative_uv.to_array(), [0.; 3]);
            }
            // Even adjacent representable native stations are not poles.
            for t in [0.5_f64.next_down(), 0.5_f64.next_up()] {
                let [u, v] = if swapped { [0.25, t] } else { [t, 0.25] };
                assert!(surface.evaluate_with_second_derivatives(u, v).is_ok());
            }
        }
    }
}

#[test]
fn common_sign_surface_continuation_rejects_exact_poles() {
    // W(u,v)=1+2u: all-positive controls do not imply a positive
    // denominator outside the domain. The pole's alpha is -1/3.
    for sign in [-1., 1.] {
        for constant in [false, true] {
            let source = patch([sign, 4. * sign], 1.5, constant);
            for swapped in [false, true] {
                let surface = if swapped {
                    source.try_swapped_uv().unwrap()
                } else {
                    source.clone()
                };
                let [u, v] = if swapped { [0.25, -0.5] } else { [-0.5, 0.25] };
                pole(surface.evaluate_extended(u, v));
                pole(surface.evaluate_extended_with_derivatives(u, v));
                pole(surface.evaluate_extended_with_second_derivatives(u, v));
                for t in [(-0.5_f64).next_down(), (-0.5_f64).next_up()] {
                    let [u, v] = if swapped { [0.25, t] } else { [t, 0.25] };
                    let jet = surface
                        .evaluate_extended_with_second_derivatives(u, v)
                        .unwrap();
                    if constant {
                        zero_jet(jet);
                    }
                }
            }
        }
    }
}

#[test]
fn constant_surface_jets_recover_after_intermediate_weight_derivative_overflow() {
    let tiny = f64::from_bits(1);
    for sign in [-1., 1.] {
        let source = patch([sign, 2. * sign], tiny, true);
        assert!(!source.evaluation_controls([1, 1]).unwrap().needs_exact);
        for surface in [source.clone(), source.try_swapped_uv().unwrap()] {
            for u in [0., tiny] {
                for v in [0., tiny] {
                    zero_jet(surface.evaluate_with_second_derivatives(u, v).unwrap());
                }
            }
        }
    }
}

#[test]
fn common_sign_surface_nets_keep_fast_controls_including_negative_gauges() {
    for sign in [-1., 1.] {
        let surface = patch([sign, 2. * sign], 1.5, false);
        assert!(!surface.evaluation_controls([1, 1]).unwrap().needs_exact);
        for (u, v) in [(0., 0.), (0.25, 0.375), (1.5, 1.)] {
            // The extended entry point inside the domain uses the same fast path.
            assert_eq!(surface.evaluate(u, v), surface.evaluate_extended(u, v));
            assert_eq!(
                surface.evaluate_with_second_derivatives(u, v),
                surface.evaluate_extended_with_second_derivatives(u, v)
            );
        }
    }
}

#[test]
fn mixed_weight_surface_four_sided_limits_and_grid_keep_the_selected_span() {
    use ParameterSide::{Left, Right};
    let surface = NurbsSurface::try_new_rational(
        1,
        1,
        4,
        4,
        (0..4)
            .flat_map(|j| {
                (0..4).map(move |i| {
                    WeightedPoint3::try_new(p((i / 2) as f64, (j / 2) as f64, 3.), [1., -2.][i % 2])
                        .unwrap()
                })
            })
            .collect(),
        vec![0., 0., 1., 1., 2., 2.],
        vec![0., 0., 1., 1., 2., 2.],
    )
    .unwrap();
    let before = surface.clone();
    for (su, x) in [(Left, 0.), (Right, 1.)] {
        for (sv, y) in [(Left, 0.), (Right, 1.)] {
            let jet = surface
                .evaluate_with_second_derivatives_on_sides(1., 1., su, sv)
                .unwrap();
            assert_eq!(jet.point, p(x, y, 3.));
            for d in [
                jet.derivative_u,
                jet.derivative_v,
                jet.derivative_uu,
                jet.derivative_uv,
                jet.derivative_vv,
            ] {
                assert_eq!(d.to_array(), [0.; 3]);
            }
        }
    }
    let samples = [2., 0.25, 1., 1.75, 0., 1.];
    let mut count = 0;
    surface.for_each_grid_point(&samples, &samples, |i, j, result| {
        assert_eq!((i, j), (count % samples.len(), count / samples.len()));
        assert_eq!(
            result.unwrap(),
            p(f64::from(samples[i] >= 1.), f64::from(samples[j] >= 1.), 3.)
        );
        count += 1;
    });
    assert_eq!(count, samples.len().pow(2));
    assert_eq!(surface, before);
}

#[test]
fn equal_weight_constant_surface_continuation_survives_unbounded_blend_factors() {
    let surface = patch([1., 1.], f64::from_bits(1), true);
    for u in [-f64::MAX, f64::MAX] {
        zero_jet(
            surface
                .evaluate_extended_with_second_derivatives(u, u)
                .unwrap(),
        );
    }
}

#[test]
fn polynomial_surface_continuation_preserves_constant_coordinates_at_huge_parameters() {
    for sign in [-1., 1.] {
        let surface = patch([sign, sign], 1.5, false);
        for alpha in [-2_f64.powi(54), 2_f64.powi(54)] {
            // In a weighted-sum blend, 1-alpha loses its unit term and the
            // supposedly constant Y and W coordinates can become zero.
            let jet = surface
                .evaluate_extended_with_second_derivatives(1.5 * alpha, 0.25)
                .unwrap();
            assert_eq!(jet.point, p(alpha, 0.25, 0.));
            assert_eq!(jet.derivative_u.to_array(), [2. / 3., 0., 0.]);
            assert_eq!(jet.derivative_v.to_array(), [0., 1., 0.]);
            for d in [jet.derivative_uu, jet.derivative_uv, jet.derivative_vv] {
                assert_eq!(d.to_array(), [0.; 3]);
            }
        }
    }
}

#[test]
fn polynomial_surface_continuation_matches_an_independent_quadratic_jet() {
    for weight in [1., -1., f64::MAX, -f64::from_bits(1)] {
        let surface = NurbsSurface::try_new_rational(
            2,
            2,
            3,
            3,
            (0..3)
                .flat_map(|j| {
                    (0..3).map(move |i| {
                        WeightedPoint3::try_new(
                            p(
                                i as f64 / 2.,
                                j as f64 / 2.,
                                f64::from(i == 2) + (i * j) as f64 / 4. + 2. * f64::from(j == 2),
                            ),
                            weight,
                        )
                        .unwrap()
                    })
                })
                .collect(),
            vec![0., 0., 0., 1., 1., 1.],
            vec![0., 0., 0., 1., 1., 1.],
        )
        .unwrap();
        for (u, v) in [(-1.25, 1.5), (0.25, -1.5), (1.75, 0.375)] {
            let jet = surface
                .evaluate_extended_with_second_derivatives(u, v)
                .unwrap();
            assert_eq!(jet.point, p(u, v, u * u + u * v + 2. * v * v));
            assert_eq!(jet.derivative_u.to_array(), [1., 0., 2. * u + v]);
            assert_eq!(jet.derivative_v.to_array(), [0., 1., u + 4. * v]);
            assert_eq!(jet.derivative_uu.to_array(), [0., 0., 2.]);
            assert_eq!(jet.derivative_uv.to_array(), [0., 0., 1.]);
            assert_eq!(jet.derivative_vv.to_array(), [0., 0., 4.]);
        }
    }
}

#[test]
fn polynomial_continuation_requires_exactly_equal_weights_in_only_the_active_patch() {
    assert!(
        patch([1., 1_f64.next_up()], 1.5, false)
            .constant_span_weight([1, 1])
            .is_none()
    );
    let surface = NurbsSurface::try_new_rational(
        1,
        1,
        4,
        2,
        (0..2)
            .flat_map(|j| {
                (0..4).map(move |i| {
                    WeightedPoint3::try_new(p(i as f64, j as f64, 0.), if i < 2 { 1. } else { -1. })
                        .unwrap()
                })
            })
            .collect(),
        vec![0., 0., 1., 1., 2., 2.],
        vec![0., 0., 1., 1.],
    )
    .unwrap();
    assert_eq!(surface.constant_span_weight([1, 1]), Some(1.));
    assert_eq!(surface.constant_span_weight([3, 1]), Some(-1.));
}
