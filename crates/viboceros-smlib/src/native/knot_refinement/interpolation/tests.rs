use super::*;

#[test]
fn native_blend_hermite_rows_reproduce_independent_monomials_and_tiny_channels() {
    use viboceros_geometry::{SurfaceBlendContinuity as G, SurfaceBlendInterpolation};
    for first in [G::Position, G::Tangency, G::Curvature] {
        for second in [G::Position, G::Tangency, G::Curvature] {
            let p = first.order() + second.order() + 1;
            for count in [1, 513, 4096] {
                let scales = [0., 1e-280, 1., 1e280];
                let mut targets = Vec::new();
                for (end, order) in (0..=first.order())
                    .map(|order| (false, order))
                    .chain((0..=second.order()).rev().map(|order| (true, order)))
                {
                    // Polynomial channels contain v^p and linear functions.
                    // Their Bernstein controls follow linear precision and
                    // v^p=B[p,p]; no portable Hermite constructor is called.
                    let monomial = match order {
                        0 => f64::from(end),
                        1 if end || p == 1 => p as f64,
                        2 if end || p == 2 => (p * (p - 1)) as f64,
                        _ => 0.,
                    };
                    let linear = match order {
                        0 => f64::from(end),
                        1 => 1.,
                        _ => 0.,
                    };
                    let constant = f64::from(order == 0);
                    for u in 0..count {
                        let scale = scales[u % scales.len()];
                        targets.push([
                            scale * monomial,
                            scale * (2. * constant + 3. * linear + monomial),
                            -scale * (linear + monomial),
                            constant + 0.25 * linear + 0.125 * monomial,
                        ]);
                    }
                }
                let output = interpolate_surface_blend(&SurfaceBlendInterpolation {
                    continuity: [first, second],
                    edge_control_count: count,
                    targets: &targets,
                })
                .unwrap();
                assert_eq!(output.len(), (p + 1) * count);
                for row in 0..=p {
                    for u in 0..count {
                        let scale = scales[u % scales.len()];
                        let t = row as f64 / p as f64;
                        let monomial = f64::from(row == p);
                        let expected = [
                            scale * monomial,
                            scale * (2. + 3. * t + monomial),
                            -scale * (t + monomial),
                            1. + 0.25 * t + 0.125 * monomial,
                        ];
                        for k in 0..4 {
                            let magnitude = if k == 3 { 1. } else { scale };
                            assert!(
                                (output[row * count + u][k] - expected[k]).abs()
                                    <= 1e-12 * magnitude,
                                "{first:?}/{second:?} count={count} row={row} u={u} k={k}"
                            );
                        }
                    }
                }
            }
        }
    }
}

#[test]
fn equal_station_rows_require_independent_fixed_constraints() {
    // Two separated cubic Bezier intervals represent X=6t and X=7+6t.
    // Linear precision gives independent controls [0,1,2,3,10,11,12,13].
    let knots = [0., 0., 0., 0., 0.5, 0.5, 0.5, 0.5, 1., 1., 1., 1.];
    let parameters = [0., 1. / 6., 1. / 3., 0.5, 0.5, 2. / 3., 5. / 6., 1.];
    let targets = [0., 1., 2., 3., 10., 11., 12., 13.]
        .into_iter()
        .flat_map(|x| [x, 0., 0., 1.])
        .collect::<Vec<_>>();
    let system = viboceros_geometry::NurbsInterpolationSystem {
        degree: 3,
        knots: &knots,
        parameters: &parameters,
        periodic: false,
        columns: 4,
        targets: &targets,
    };
    let mask = [1, 0, 0, 1, 1, 0, 0, 1];
    let output = solve_with_fixed_rows(&system, Some(&mask)).unwrap();
    for (a, b) in output.iter().zip(&targets) {
        assert!((a - b).abs() < 1e-12);
    }
    let mut invalid = mask;
    invalid[4] = 0;
    assert!(matches!(
        solve_with_fixed_rows(&system, Some(&invalid)),
        Err(Error::Kernel(_))
    ));
    assert_eq!(system.targets, targets);
}
#[test]
fn closed_grid_band_retries_native_pivoting_for_an_invertible_zero_diagonal() {
    // Ordered integer stations shifted by one give the exact cyclic
    // permutation A[i,(i+1)%64]=1. Every diagonal entry is zero, so native
    // unpivoted band LU must reject this invertible system. The existing
    // native pivoted producer must recover every tiny and ordinary channel.
    let n = 64;
    let knots = [
        vec![0., 0.],
        (1..n).map(|i| i as f64).collect(),
        vec![n as f64; n.min(2)],
    ]
    .concat();
    let parameters = (1..=n).map(|i| i as f64).collect::<Vec<_>>();
    let expected = (0..n)
        .flat_map(|i| [i as f64, -(i as f64), (i + 1) as f64 * 1e-280, 1.])
        .collect::<Vec<_>>();
    let targets = (0..n)
        .flat_map(|i| {
            expected[((i + 1) % n) * 4..((i + 1) % n) * 4 + 4]
                .iter()
                .copied()
        })
        .collect::<Vec<_>>();
    let job = viboceros_geometry::PointGridInterpolationSystem {
        degree: 1,
        knots: &knots,
        parameters: &parameters,
        closed: true,
        columns: 4,
        targets: &targets,
    };
    let output = solve_point_grid(&job).unwrap();
    assert_eq!(output.controls, expected);
    assert_eq!(output.amplification, vec![1.; n]);
    assert_eq!(job.targets, targets);
}

#[test]
fn nonuniform_periodic_band_retries_pivoting_and_accepts_a_final_endpoint_site() {
    // A one-station phase shift gives an independently invertible cyclic
    // permutation with every diagonal zero. The last actual site is the
    // domain end; the appended interval endpoint is metadata, not a row.
    let n = 64;
    let knots = [
        vec![0., 0.],
        (1..n).map(|i| i as f64).collect(),
        vec![n as f64; 2],
    ]
    .concat();
    let parameters = [
        (1..=n).map(|i| i as f64).collect::<Vec<_>>(),
        vec![n as f64],
    ]
    .concat();
    let expected = (0..n)
        .flat_map(|i| [i as f64, -(i as f64), (i + 1) as f64 * 1e-280, 1.])
        .collect::<Vec<_>>();
    let targets = (0..n)
        .flat_map(|i| {
            expected[((i + 1) % n) * 4..((i + 1) % n) * 4 + 4]
                .iter()
                .copied()
        })
        .collect::<Vec<_>>();
    let job = viboceros_geometry::NurbsInterpolationSystem {
        degree: 1,
        knots: &knots,
        parameters: &parameters,
        periodic: true,
        columns: 4,
        targets: &targets,
    };
    assert_eq!(solve_interpolation(&job, None, Some(0)).unwrap(), expected);
    assert_eq!(job.targets, targets);
    assert!(solve_interpolation(&job, None, None).is_err());
}
