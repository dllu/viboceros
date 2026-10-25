//! Shared Bernstein coefficient products with compensated accumulation.
use crate::GeometryError;

pub(crate) fn binomial(degree: usize) -> Vec<f64> {
    let mut result = vec![1.; degree + 1];
    for i in 1..=degree {
        result[i] = result[i - 1] * (degree + 1 - i) as f64 / i as f64;
    }
    result
}

/// Half subdivision in coefficient space, retaining every channel's basis.
pub(crate) fn subdivide<const N: usize>(
    controls: &[[f64; N]],
    charge: &mut impl FnMut(usize) -> Result<(), GeometryError>,
) -> Result<[Vec<[f64; N]>; 2], GeometryError> {
    charge(
        controls
            .len()
            .saturating_mul(controls.len())
            .saturating_mul(N),
    )?;
    let mut work = controls.to_vec();
    let mut left = vec![work[0]];
    let mut right = vec![work[work.len() - 1]];
    for count in (1..work.len()).rev() {
        for i in 0..count {
            work[i] = std::array::from_fn(|k| work[i][k].midpoint(work[i + 1][k]));
        }
        left.push(work[0]);
        right.push(work[count - 1]);
    }
    right.reverse();
    Ok([left, right])
}

pub(crate) fn product(
    a: &[f64],
    b: &[f64],
    charge: &mut impl FnMut(usize) -> Result<(), GeometryError>,
) -> Result<Vec<f64>, GeometryError> {
    charge(a.len().saturating_mul(b.len()))?;
    let (p, q) = (a.len() - 1, b.len() - 1);
    let (cp, cq, cn) = (binomial(p), binomial(q), binomial(p + q));
    let mut result = vec![0.; p + q + 1];
    for k in 0..=p + q {
        let mut correction = 0.;
        for i in k.saturating_sub(q)..=k.min(p) {
            let value = a[i] * b[k - i] * (cp[i] * cq[k - i] / cn[k]);
            let sum = result[k] + value;
            correction += if result[k].abs() >= value.abs() {
                (result[k] - sum) + value
            } else {
                (value - sum) + result[k]
            };
            result[k] = sum;
        }
        result[k] += correction;
    }
    Ok(result)
}
