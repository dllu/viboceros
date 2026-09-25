//! Finite-height cuts of separated skew-cylinder intersection branches.

use super::*;

#[allow(clippy::too_many_arguments)]
pub(super) fn add_boundary_angles(
    angles: &mut Vec<Real>,
    basis: Basis,
    sign: Real,
    (linear, root): (Real, Real),
    target: Real,
    small_axial: bool,
    fit_tolerance: Real,
) -> Result<(), GeometryError> {
    // In either half-circle, theta = base + 2 atan(t), -1 <= t <= 1.
    // Squaring target = A cos(theta) + B sqrt(R^2-(r sin(theta)-h)^2)
    // gives a quartic. Filter its roots against the unsquared equation.
    let radial_scale = basis.big.max(basis.small).max(basis.miss.abs());
    let axial_scale = linear
        .abs()
        .max(target.abs())
        .max(root.abs() * radial_scale);
    if !axial_scale.is_finite() || axial_scale == 0.0 {
        return Err(GeometryError::UnsupportedSurfaceSurfaceIntersection {
            context: "skew cylinder clipping coefficients are ill-conditioned",
        });
    }
    let a = linear / axial_scale;
    let b = root * (radial_scale / axial_scale);
    let target = target / axial_scale;
    let r = basis.small / radial_scale;
    let h = basis.miss / radial_scale;
    let big = basis.big / radial_scale;
    let c = big * big - h * h;
    for (base, phase) in [(0.0, 1.0), (std::f64::consts::PI, -1.0)] {
        let u = target - phase * a;
        let v = target + phase * a;
        let b2 = b * b;
        let odd = -4.0 * phase * b2 * r * h;
        let coefficients = [
            u * u - b2 * c,
            odd,
            2.0 * u * v - b2 * (2.0 * c - 4.0 * r * r),
            odd,
            v * v - b2 * c,
        ];
        if !coefficients.iter().all(|value| value.is_finite()) {
            return Err(GeometryError::UnsupportedSurfaceSurfaceIntersection {
                context: "skew cylinder clipping polynomial is ill-conditioned",
            });
        }
        for parameter in roots_in_unit_interval(coefficients, 4) {
            let angle = (base + 2.0 * parameter.atan()).rem_euclid(std::f64::consts::TAU);
            let exact = if small_axial {
                basis.axials(sign, angle).0
            } else {
                basis.axials(sign, angle).1
            };
            if (exact - target * axial_scale).abs() <= fit_tolerance {
                angles.push(angle);
            }
        }
    }
    Ok(())
}

fn roots_in_unit_interval(coefficients: [Real; 5], mut degree: usize) -> Vec<Real> {
    let scale = coefficients.iter().map(|value| value.abs()).sum::<Real>();
    if scale == 0.0 {
        return Vec::new();
    }
    let epsilon = 64.0 * Real::EPSILON * scale;
    while degree > 0 && coefficients[degree].abs() <= epsilon {
        degree -= 1;
    }
    if degree == 0 {
        return Vec::new();
    }
    if degree == 1 {
        let root = -coefficients[0] / coefficients[1];
        return if (-1.0 - 64.0 * Real::EPSILON..=1.0 + 64.0 * Real::EPSILON).contains(&root) {
            vec![root.clamp(-1.0, 1.0)]
        } else {
            Vec::new()
        };
    }
    let mut derivative = [0.0; 5];
    for index in 1..=degree {
        derivative[index - 1] = index as Real * coefficients[index];
    }
    let mut stations = vec![-1.0];
    stations.extend(roots_in_unit_interval(derivative, degree - 1));
    stations.push(1.0);
    stations.sort_by(Real::total_cmp);
    stations.dedup_by(|left, right| (*left - *right).abs() <= 64.0 * Real::EPSILON);

    let mut roots = Vec::new();
    for &station in &stations {
        if evaluate(coefficients, degree, station).abs() <= epsilon {
            roots.push(station);
        }
    }
    for pair in stations.windows(2) {
        let (mut left, mut right) = (pair[0], pair[1]);
        let mut left_value = evaluate(coefficients, degree, left);
        let right_value = evaluate(coefficients, degree, right);
        if left_value.abs() <= epsilon || right_value.abs() <= epsilon {
            continue;
        }
        if left_value.is_sign_negative() == right_value.is_sign_negative() {
            continue;
        }
        for _ in 0..80 {
            let middle = 0.5 * (left + right);
            if middle == left || middle == right {
                break;
            }
            let value = evaluate(coefficients, degree, middle);
            if value.is_sign_negative() == left_value.is_sign_negative() {
                left = middle;
                left_value = value;
            } else {
                right = middle;
            }
        }
        roots.push(0.5 * (left + right));
    }
    roots.sort_by(Real::total_cmp);
    roots.dedup_by(|left, right| (*left - *right).abs() <= 64.0 * Real::EPSILON);
    roots
}

fn evaluate(coefficients: [Real; 5], degree: usize, parameter: Real) -> Real {
    (0..degree)
        .rev()
        .fold(coefficients[degree], |value, index| {
            value.mul_add(parameter, coefficients[index])
        })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn quartic_isolator_keeps_simple_and_double_roots() {
        // (t+0.8)(t-0.3)^2(t-0.9)
        let roots = roots_in_unit_interval([-0.0648, 0.423, -0.57, -0.7, 1.0], 4);
        assert_eq!(roots.len(), 3);
        for (actual, expected) in roots.into_iter().zip([-0.8, 0.3, 0.9]) {
            assert!((actual - expected).abs() < 1e-12, "{actual} != {expected}");
        }
    }
}
