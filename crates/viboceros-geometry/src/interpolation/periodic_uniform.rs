//! Linear cyclic solve for uniform cubic interpolation.
use super::*;

/// Solves D[i-1] + 4 D[i] + D[i+1] = 6 P[i] with cyclic indices,
/// then rotates the controls to the existing periodic B-spline basis order.
pub(super) fn solve(points: &[Point3]) -> Result<Vec<Point3>, GeometryError> {
    let n = points.len();
    debug_assert!(n >= 2);
    let scales: [Real; 3] = std::array::from_fn(|axis| {
        points
            .iter()
            .map(|p| p.to_array()[axis].abs())
            .fold(0., Real::max)
            .max(Real::MIN_POSITIVE)
    });
    let fixed: [Option<Real>; 3] = std::array::from_fn(|axis| {
        points
            .iter()
            .all(|p| p.to_array()[axis] == points[0].to_array()[axis])
            .then_some(points[0].to_array()[axis])
    });
    // A = T + u v^T, with gamma=-4, u=(-4,0,...,1), v=(1,0,...,-1/4).
    // T is strictly diagonally dominant and shares one factorization for XYZ/u.
    let mut diagonal: Vec<Real> = vec![4.; n];
    diagonal[0] = 8.;
    diagonal[n - 1] = 4.25;
    let mut rhs = points
        .iter()
        .map(|p| {
            let p = p.to_array();
            [
                6. * (p[0] / scales[0]),
                6. * (p[1] / scales[1]),
                6. * (p[2] / scales[2]),
                0.,
            ]
        })
        .collect::<Vec<_>>();
    rhs[0][3] = -4.;
    rhs[n - 1][3] = 1.;
    for row in 1..n {
        let factor: Real = 1. / diagonal[row - 1];
        diagonal[row] -= factor;
        let prior = rhs[row - 1];
        for (value, prior) in rhs[row].iter_mut().zip(prior) {
            *value = (-factor).mul_add(prior, *value);
        }
    }
    for value in &mut rhs[n - 1] {
        *value /= diagonal[n - 1];
    }
    for row in (0..n - 1).rev() {
        let next = rhs[row + 1];
        for (value, next) in rhs[row].iter_mut().zip(next) {
            *value = (*value - next) / diagonal[row];
        }
    }
    let denominator = 1. + rhs[0][3] - 0.25 * rhs[n - 1][3];
    let factors: [Real; 3] =
        std::array::from_fn(|axis| (rhs[0][axis] - 0.25 * rhs[n - 1][axis]) / denominator);
    let mut controls = rhs
        .iter()
        .map(|row| {
            Point3::try_from(std::array::from_fn(|axis| {
                fixed[axis]
                    .unwrap_or_else(|| (-factors[axis]).mul_add(row[3], row[axis]) * scales[axis])
            }))
        })
        .collect::<Result<Vec<_>, _>>()?;
    controls.rotate_right(1);
    Ok(controls)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn cyclic_uniform_matches_pivoted_dense_and_preserves_constant_planes() {
        for n in [2, 3, 4, 7, 17, 64] {
            for scale in [1e-200, 1., 1e200] {
                let points = (0..n)
                    .map(|i| {
                        let t = i as Real;
                        Point3::try_new(
                            scale * (0.31 * t).sin(),
                            scale * (0.57 * t).cos(),
                            scale * 0.7,
                        )
                        .unwrap()
                    })
                    .collect::<Vec<_>>();
                let rows = (0..n)
                    .map(|i| {
                        let mut r = vec![0.; n];
                        r[i] = 4. / 6.;
                        r[(i + n - 1) % n] += 1. / 6.;
                        r[(i + 1) % n] += 1. / 6.;
                        r
                    })
                    .collect::<Vec<_>>();
                let mut expected = solve_control_points(&rows, &points).unwrap();
                expected.rotate_right(1);
                let actual = solve(&points).unwrap();
                for (a, b) in actual.iter().zip(expected) {
                    for (x, y) in a.to_array().into_iter().zip(b.to_array()) {
                        assert!(((x - y) / scale).abs() < 2e-14, "n={n}: {x} != {y}");
                    }
                    assert_eq!(a.z(), scale * 0.7);
                }
            }
        }
    }
    #[test]
    fn large_uniform_periodic_sampling_interpolates_and_keeps_its_seam() {
        let n = 10_000;
        let samples = (0..n)
            .map(|i| {
                let t = std::f64::consts::TAU * i as Real / n as Real;
                Point3::try_new(5. * t.cos(), 3. * t.sin(), 2.).unwrap()
            })
            .collect::<Vec<_>>();
        let curve = NurbsCurve::try_interpolate_periodic_uniform_samples(&samples, n).unwrap();
        assert_eq!(curve.control_points().len(), n + 3);
        assert!(curve.is_periodic());
        assert!(curve.is_closed().unwrap());
        for i in (0..n).step_by(137) {
            assert!(
                curve
                    .evaluate(i as Real)
                    .unwrap()
                    .distance_to(samples[i])
                    .unwrap()
                    < 2e-12
            );
        }
        assert_eq!(&curve.control_points()[..3], &curve.control_points()[n..]);
    }
}
