//! Reproduce known power polynomials from independent endpoint conditions.
use viboceros_geometry::EndpointInterpolation;
pub fn check<E: std::fmt::Debug>(
    mut solve: impl FnMut(&EndpointInterpolation<'_>) -> Result<Vec<[f64; 4]>, E>,
) {
    for first in 0..=4 {
        for second in 0..=4 {
            let degree = usize::from(first + second + 1);
            let columns = 3;
            let coefficient = |k: usize, column: usize, channel: usize| {
                (-1_f64).powi(k as i32) * (column * 4 + channel + 1) as f64 / (k + 1) as f64
            };
            let value = |t: f64, order: usize, column, channel| {
                (order..=degree)
                    .map(|k| {
                        let falling = (0..order).map(|j| (k - j) as f64).product::<f64>();
                        coefficient(k, column, channel) * falling * t.powi((k - order) as i32)
                    })
                    .sum::<f64>()
            };
            let mut targets = vec![];
            for (t, orders) in [
                (0., (0..=first).collect::<Vec<_>>()),
                (1., (0..=second).rev().collect()),
            ] {
                for order in orders {
                    for column in 0..columns {
                        targets.push(std::array::from_fn(|channel| {
                            value(t, usize::from(order), column, channel)
                        }));
                    }
                }
            }
            let input = EndpointInterpolation {
                orders: [first, second],
                column_count: columns,
                targets: &targets,
            };
            let net = solve(&input).unwrap();
            assert_eq!(net.len(), targets.len());
            for station in 0..=16 {
                let t = station as f64 / 16.;
                for column in 0..columns {
                    let mut rows = net
                        .iter()
                        .skip(column)
                        .step_by(columns)
                        .copied()
                        .collect::<Vec<_>>();
                    for remaining in (1..=degree).rev() {
                        for i in 0..remaining {
                            rows[i] =
                                std::array::from_fn(|k| (1. - t) * rows[i][k] + t * rows[i + 1][k]);
                        }
                    }
                    for k in 0..4 {
                        let expected = value(t, 0, column, k);
                        assert!(
                            (rows[0][k] - expected).abs() < 2e-10,
                            "{first}/{second}, {column}/{k} at {t}: {:?} vs {expected}",
                            rows[0]
                        );
                    }
                }
            }
        }
    }
}
