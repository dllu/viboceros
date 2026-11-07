//! Convert dimensionless control gaps to scalar parameter freedoms.
use super::*;
use crate::exact_scalar::{Rational, rational, scalar};
fn coefficients(order: u8) -> &'static [i32] {
    match order {
        2 => &[-1, 1],
        3 => &[1, -2, 1],
        4 => &[-1, 3, -3, 1],
        _ => unreachable!(),
    }
}
fn falling(degree: usize, order: u8) -> Real {
    (0..usize::from(order))
        .map(|j| (degree - j) as Real)
        .product()
}
fn sign(order: u8, end: bool) -> Real {
    if end && order.is_multiple_of(2) {
        -1.
    } else {
        1.
    }
}
pub(super) fn tangent_derivative(
    ratios: [Real; 3],
    handle: Real,
    degree: usize,
    order: u8,
    end: bool,
) -> Option<Real> {
    let gaps = [1., ratios[0], ratios[1], ratios[2]];
    let mut delta = coefficients(order)
        .iter()
        .zip(gaps)
        .fold(0., |sum, (&c, g)| (c as Real).mul_add(g, sum));
    let magnitude: Real = coefficients(order)
        .iter()
        .zip(gaps)
        .map(|(&c, g)| (c as Real * g).abs())
        .sum();
    if delta.abs() <= 64. * Real::EPSILON * magnitude {
        let exact: Rational = coefficients(order)
            .iter()
            .zip(gaps)
            .map(|(&c, g)| rational(c as Real) * rational(g))
            .sum();
        delta = scalar(&exact).ok()?;
    }
    if delta == 0. {
        return Some(0.);
    }
    let raw = sign(order, end) * falling(degree, order) * handle * delta;
    raw.is_normal().then_some(raw)
}
pub(super) fn exact_derivatives(
    base: [Vector3; 4],
    ratios: [Real; 3],
    handle: Real,
    degree: usize,
    order: u8,
    end: bool,
    mut out: [[Real; 3]; 4],
) -> Result<[[Real; 3]; 4], GeometryError> {
    let h = rational(handle);
    let s = rational(degree as Real * handle);
    let gaps = [
        rational(1.),
        rational(ratios[0]),
        rational(ratios[1]),
        rational(ratios[2]),
    ];
    let mut parameters: [Rational; 3] = std::array::from_fn(|_| rational(0.));
    for k in 2..=order {
        let delta: Rational = coefficients(k)
            .iter()
            .zip(&gaps)
            .map(|(&c, g)| rational(c as Real) * g)
            .sum();
        parameters[usize::from(k - 2)] = rational(sign(k, end) * falling(degree, k)) * &h * delta;
    }
    let [a, b, c] = parameters;
    let curvature_squared: Rational = base[1]
        .to_array()
        .map(|v| rational(v).pow(2))
        .into_iter()
        .sum();
    for (axis, [t, k, j, l]) in (0..3)
        .map(|axis| base.map(|v| rational(v.to_array()[axis])))
        .enumerate()
    {
        let values = [
            s.pow(2) * &k + &a * &t,
            s.pow(3) * &j + rational(3.) * &s * &a * &k + &b * &t,
            s.pow(4) * &l
                + rational(6.) * s.pow(2) * &a * &j
                + (rational(12.) * &a * &b / &s
                    + rational(12.) * &a * &curvature_squared * s.pow(2)
                    + rational(9.) * a.pow(3) / s.pow(2))
                    * &k
                + &c * &t,
        ];
        for (index, value) in values
            .into_iter()
            .enumerate()
            .take(usize::from(order.saturating_sub(1)))
        {
            out[index + 1][axis] = scalar(&value)?;
        }
    }
    Ok(out)
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn tiny_speed_retains_finite_control_motion_without_representable_normalized_freedoms() {
        let zero = Vector3::try_new(0., 0., 0.).unwrap();
        let jet = CurveBlendJet::new(
            Point3::try_new(0., 0., 0.).unwrap(),
            Some(
                Vector3::try_new(1., 0., 0.)
                    .unwrap()
                    .normalized_nonzero()
                    .unwrap(),
            ),
            zero,
        )
        .with_higher_derivatives(zero, zero)
        .with_control_spacing_ratios([1e200, 1., 1.])
        .unwrap();
        for end in [false, true] {
            let actual = jet.parameterized_derivatives(1e-200, 9, 4, end).unwrap();
            assert!((actual[1][0] - (if end { -72. } else { 72. })).abs() < 1e-12);
            assert!((actual[2][0] + 1008.).abs() < 1e-11);
            assert!((actual[3][0] - (if end { -9072. } else { 9072. })).abs() < 1e-10);
        }
    }
}
