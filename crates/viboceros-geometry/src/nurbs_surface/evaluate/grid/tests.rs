use super::*;
use crate::{AffineTransform3, WeightedPoint3};

fn point(p: [Real; 3]) -> Point3 {
    Point3::try_from(p).unwrap()
}

fn rational_patch() -> NurbsSurface {
    NurbsSurface::try_new_rational(
        2,
        2,
        3,
        3,
        (0..3)
            .flat_map(|j| {
                (0..3).map(move |i| {
                    WeightedPoint3::try_new(
                        point([i as Real, j as Real, (i * j) as Real]),
                        [1., 0.5, 2.][(i + j) % 3],
                    )
                    .unwrap()
                })
            })
            .collect(),
        vec![0., 0., 0., 1., 1., 1.],
        vec![0., 0., 0., 1., 1., 1.],
    )
    .unwrap()
}

fn assert_grid(surface: &NurbsSurface, us: &[Real], vs: &[Real]) -> usize {
    let before = surface.clone();
    let mut count = 0;
    surface.for_each_grid_point(us, vs, |i, j, actual| {
        assert_eq!((i, j), (count % us.len(), count / us.len()));
        match (actual, surface.evaluate(us[i], vs[j])) {
            (Ok(actual), Ok(expected)) => assert_eq!(
                actual.to_array().map(Real::to_bits),
                expected.to_array().map(Real::to_bits),
                "u={}, v={}",
                us[i],
                vs[j],
            ),
            (Err(actual), Err(expected)) => {
                assert_eq!(format!("{actual:?}"), format!("{expected:?}"))
            }
            (actual, expected) => panic!("u={}, v={}: {actual:?} != {expected:?}", us[i], vs[j]),
        }
        count += 1;
    });
    assert_eq!(count, us.len() * vs.len());
    assert_eq!(*surface, before);
    count
}

#[test]
fn grid_points_match_scalar_bits_across_degrees_spans_scales_and_weight_signs() {
    let mut state = 29_u64;
    let mut next = || {
        state = state.wrapping_mul(6364136223846793005).wrapping_add(1);
        ((state >> 32) & 255) as Real / 8. - 16.
    };
    let us = [
        1.,
        0.,
        0.5,
        0.125,
        1. / 3.,
        0.25,
        0.625,
        0.75,
        0.,
        0.375,
        0.5,
    ];
    let vs = [0., 0.75, 0.25, 1., 0.5, 0.375, 0.625, 0.5, 0.875];
    let mut count = 0;
    for degree_u in 1..=5 {
        for degree_v in 1..=4 {
            let (nu, nv) = (degree_u + 3, degree_v + 4);
            for scale in [2_f64.powi(-600), 1., 2_f64.powi(600)] {
                for mode in 0..3 {
                    let controls = (0..nu * nv)
                        .map(|index| {
                            let sign = if mode == 1 || (mode == 2 && index % 3 == 0) {
                                -1.
                            } else {
                                1.
                            };
                            let weight = if mode == 2 && index % 7 == 0 {
                                Real::MIN_POSITIVE
                            } else {
                                sign * (1 + index % 4) as Real * scale
                            };
                            WeightedPoint3::try_new(
                                point([next() * scale, next() * scale, next() * scale]),
                                weight,
                            )
                            .unwrap()
                        })
                        .collect();
                    let surface = NurbsSurface::try_new_rational(
                        degree_u,
                        degree_v,
                        nu,
                        nv,
                        controls,
                        crate::nurbs::clamped_uniform_knots(degree_u, nu).unwrap(),
                        crate::nurbs::clamped_uniform_knots(degree_v, nv).unwrap(),
                    )
                    .unwrap();
                    count += assert_grid(&surface, &us, &vs);
                }
            }
        }
    }
    assert_eq!(count, 17_820);
}

#[test]
fn grid_points_preserve_extreme_domains_translation_reversal_and_axis_swap() {
    let source = rational_patch()
        .try_insert_knot_u(0.5, 2)
        .unwrap()
        .try_insert_knot_v(0.25, 2)
        .unwrap();
    let translated = source
        .transformed(AffineTransform3::from_translation(
            Vector3::try_new(1e12, -2e12, 3e12).unwrap(),
        ))
        .unwrap();
    for surface in [
        source.clone(),
        translated,
        source.try_reversed_u().unwrap(),
        source.try_swapped_uv().unwrap(),
    ] {
        for (u_domain, v_domain) in [
            (0.0..=1.0, 0.0..=1.0),
            (-2e12..=6e12, 1e-12..=5e-12),
            (0.0..=1e-308, 0.0..=1e308),
            (-Real::MAX..=Real::MAX, 0.0..=1.0),
        ] {
            let surface = surface.try_reparameterized(u_domain, v_domain).unwrap();
            let fractions = [0., 0.25, 0.5, 0.625, 1., 0.375, 0.5];
            let us = fractions.map(|t| surface.parameter_at_u(t).unwrap());
            let vs = fractions.map(|t| surface.parameter_at_v(t).unwrap());
            assert_grid(&surface, &us, &vs);
        }
    }
}

#[test]
fn grid_points_preserve_discontinuous_knots_and_exact_interpolated_controls() {
    let knots = vec![0., 0., 0., 0.5, 0.5, 0.5, 1., 1., 1.];
    let controls = (0..6)
        .flat_map(|j| {
            (0..6).map(move |i| {
                WeightedPoint3::try_new(point([1e12 + i as Real, j as Real, (i * j) as Real]), 1.)
                    .unwrap()
            })
        })
        .collect();
    let surface =
        NurbsSurface::try_new_rational(2, 2, 6, 6, controls, knots.clone(), knots).unwrap();
    let parameters = [
        0.,
        0.25,
        0.5_f64.next_down(),
        0.5,
        0.5_f64.next_up(),
        0.75,
        1.,
    ];
    assert_grid(&surface, &parameters, &parameters);
    let mut center = None;
    surface.for_each_grid_point(&[0.5], &[0.5], |_, _, result| {
        center = Some(result.unwrap())
    });
    assert_eq!(center, Some(surface.control_points[3 * 6 + 3].point()));
}

#[test]
fn grid_points_retain_scalar_fallback_for_signed_weight_overflow_and_poles() {
    let make = |xs: [Real; 2], weights: [Real; 2]| {
        NurbsSurface::try_new_rational(
            1,
            1,
            2,
            2,
            [0., 1.]
                .into_iter()
                .flat_map(|y| {
                    xs.into_iter()
                        .zip(weights)
                        .map(move |(x, w)| WeightedPoint3::try_new(point([x, y, 0.]), w).unwrap())
                })
                .collect(),
            vec![0., 0., 1., 1.],
            vec![0., 0., 1., 1.],
        )
        .unwrap()
    };
    for surface in [
        make([-1e308, 1e308], [1., 1.]),
        make([1e308, 5e307], [1., -1.]),
        make([1., 2.], [Real::MIN_POSITIVE, Real::MAX]),
    ] {
        assert_grid(
            &surface,
            &[0., 0.25, 0.5, 0.58, 0.75, 1.],
            &[0., 0.25, 0.5, 1.],
        );
    }
    let signed = make([1e308, 5e307], [1., -1.]);
    let columns = signed.grid_columns(&[0.58], 1);
    // Signed nets now bypass rounded contractions before projection, including
    // cases where a cached projection would have returned a false finite pole.
    assert!(matches!(
        columns[0].as_ref().unwrap().contraction,
        Contraction::Exact(_)
    ));
    let mut actual = None;
    signed.for_each_grid_point(&[0.58], &[0.25], |_, _, result| {
        actual = Some(result.unwrap())
    });
    assert_eq!(actual, Some(signed.evaluate(0.58, 0.25).unwrap()));
}

#[test]
fn grid_points_validate_each_cell_and_preserve_empty_duplicate_and_unsorted_inputs() {
    let surface = rational_patch();
    let parameters = [
        Real::NAN,
        Real::INFINITY,
        -1.,
        0.5,
        0.,
        1.,
        2.,
        0.5,
        Real::NEG_INFINITY,
    ];
    assert_grid(&surface, &parameters, &parameters);
    assert_grid(&surface, &[], &parameters);
    assert_grid(&surface, &parameters, &[]);
    assert_grid(&surface, &[], &[]);
}

#[test]
fn exact_grid_retains_zero_weight_rows_until_the_final_tensor_projection() {
    let surface = NurbsSurface::try_new_rational(
        1,
        1,
        2,
        2,
        [
            (point([0., 0., 0.]), 1.),
            (point([1., 0., 0.]), -1.),
            (point([0., 1., 0.]), 1.),
            (point([1., 1., 0.]), 1.),
        ]
        .map(|(p, w)| WeightedPoint3::try_new(p, w).unwrap())
        .to_vec(),
        vec![0., 0., 1., 1.],
        vec![0., 0., 1., 1.],
    )
    .unwrap();
    // The first U-contracted row has W=0 at u=1/2, but a later V blend
    // produces a regular point. No intermediate row may be projected.
    let mut result = None;
    surface.for_each_grid_point(&[0.5], &[0.25], |_, _, value| result = Some(value));
    assert_eq!(result.unwrap().unwrap(), point([-1., 1., 0.]));
    assert_grid(&surface, &[0.5, 0.25, 0.5, 0.75], &[0.25, 0., 0.75, 0.25]);
}

#[test]
fn grid_and_scalar_recover_exact_corners_when_normalization_erases_their_weight() {
    let points = [[-0., -0., 0.], [4., 0., 1.], [0., 3., 2.], [4., 3., 5.]].map(point);
    for tiny_index in 0..4 {
        for sign in [-1., 1.] {
            let controls = points
                .into_iter()
                .enumerate()
                .map(|(i, p)| {
                    WeightedPoint3::try_new(
                        p,
                        sign * if i == tiny_index {
                            Real::MIN_POSITIVE
                        } else {
                            Real::MAX
                        },
                    )
                    .unwrap()
                })
                .collect();
            let surface = NurbsSurface::try_new_rational(
                1,
                1,
                2,
                2,
                controls,
                vec![0., 0., 1., 1.],
                vec![0., 0., 1., 1.],
            )
            .unwrap();
            let u = (tiny_index % 2) as Real;
            let v = (tiny_index / 2) as Real;
            assert_eq!(
                surface
                    .evaluate(u, v)
                    .unwrap()
                    .to_array()
                    .map(Real::to_bits),
                points[tiny_index].to_array().map(Real::to_bits)
            );
            // The true derivatives overflow for this nonconstant patch; point
            // recovery must not turn a requested differential jet into zeros.
            assert!(surface.evaluate_with_derivatives(u, v).is_err());
            assert_grid(&surface, &[0., 0.25, 0.5, 1.], &[0., 0.5, 1.]);
        }
    }
}

#[test]
fn grid_points_match_an_independent_polynomial_tensor_formula() {
    let surface = NurbsSurface::try_clamped_uniform(
        2,
        2,
        3,
        3,
        (0..3)
            .flat_map(|j| {
                (0..3).map(move |i| {
                    point([
                        i as Real / 2.,
                        j as Real / 2.,
                        Real::from(i == 2) + (i * j) as Real / 4. + 2. * Real::from(j == 2),
                    ])
                })
            })
            .collect(),
    )
    .unwrap();
    let parameters = [0., 0.125, 0.25, 0.5, 0.75, 1.];
    surface.for_each_grid_point(&parameters, &parameters, |i, j, result| {
        let (u, v) = (parameters[i], parameters[j]);
        assert_eq!(result.unwrap(), point([u, v, u * u + u * v + 2. * v * v]));
    });
}

#[test]
#[ignore = "manual release microbenchmark; not a timing assertion"]
fn benchmark_surface_grid_evaluation() {
    use std::{hint::black_box, time::Instant};
    let source = rational_patch();
    let surface = source
        .try_insert_knot_u(0.5, 1)
        .unwrap()
        .try_insert_knot_v(0.5, 1)
        .unwrap();
    let parameters: Vec<_> = (0..33).map(|i| Real::from(i) / 32.).collect();
    for surface in [&source, &surface] {
        for _round in 0..3 {
            let mut sums = [0.; 2];
            for (method, sum) in sums.iter_mut().enumerate() {
                let start = Instant::now();
                for _ in 0..200 {
                    if method == 0 {
                        for &v in &parameters {
                            for &u in &parameters {
                                *sum += black_box(
                                    surface.evaluate(black_box(u), black_box(v)).unwrap(),
                                )
                                .z();
                            }
                        }
                    } else {
                        surface.for_each_grid_point(
                            black_box(&parameters),
                            black_box(&parameters),
                            |_, _, result| {
                                *sum += black_box(result.unwrap()).z();
                            },
                        );
                    }
                }
                println!(
                    "grid benchmark spans={} method={method} elapsed_ns={}",
                    surface.spans_u().count(),
                    start.elapsed().as_nanos()
                );
            }
            assert_eq!(sums[0].to_bits(), sums[1].to_bits());
        }
    }
}
