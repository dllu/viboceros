//! Bernstein composition without projection of intermediate coefficients.
use super::*;

const MAX_COMPOSED_DEGREE: usize = 256;

impl Net {
    /// Compose a tensor patch with a homogeneous UV Bezier curve. The result
    /// is a rational Bezier curve of degree m*(p+q), with no fitting/sampling.
    /// Outside the patch domain this is polynomial/rational extrapolation;
    /// the caller must restrict its use to an enclosure, not attained points.
    pub(in crate::bounds) fn compose(
        &self,
        uv: &Net,
        domain: [[f64; 2]; 2],
        budget: &mut Budget,
    ) -> Result<Net, GeometryError> {
        let [p, q] = self.degrees;
        let degree = uv.degrees[0].saturating_mul(p.saturating_add(q));
        if degree > MAX_COMPOSED_DEGREE {
            return Err(GeometryError::BoundingBoxDidNotConverge);
        }
        budget.charge((degree + 1).saturating_mul(self.controls.len()))?;
        let mut axes = Vec::with_capacity(2);
        for (axis, [a, b]) in domain.into_iter().enumerate() {
            let width = b - a;
            // Halve opposite huge endpoints before subtraction when needed.
            let scale = if width.is_infinite() { 0.5 } else { 1. };
            let width = b * scale - a * scale;
            let offset = uv.origin[axis] * scale - a * scale;
            let values = uv
                .controls
                .iter()
                .map(|h| {
                    let x = offset.mul_add(h[3], h[axis] * scale) / width;
                    [x, h[3] - x]
                })
                .collect::<Vec<_>>();
            if values.iter().flatten().any(|x| !x.is_finite()) {
                return Err(GeometryError::BoundingBoxDidNotConverge);
            }
            axes.push(basis(&values, self.degrees[axis], budget)?);
        }
        let mut controls = vec![[0.; 4]; degree + 1];
        let mut correction = controls.clone();
        for (j, v) in axes[1].iter().enumerate() {
            for (i, u) in axes[0].iter().enumerate() {
                let basis = product(u, v, budget)?;
                for (k, coefficient) in basis.into_iter().enumerate() {
                    for axis in 0..4 {
                        let value = coefficient * self.controls[j * (p + 1) + i][axis];
                        let sum = controls[k][axis] + value;
                        correction[k][axis] += if controls[k][axis].abs() >= value.abs() {
                            (controls[k][axis] - sum) + value
                        } else {
                            (value - sum) + controls[k][axis]
                        };
                        controls[k][axis] = sum;
                    }
                }
            }
        }
        for (control, correction) in controls.iter_mut().zip(correction) {
            for axis in 0..4 {
                control[axis] += correction[axis];
            }
        }
        if controls.iter().flatten().any(|x| !x.is_finite()) {
            return Err(GeometryError::BoundingBoxDidNotConverge);
        }
        Ok(Net {
            degrees: [degree, 0],
            origin: self.origin,
            controls,
            depth: uv.depth,
        })
    }
}

fn basis(
    values: &[[f64; 2]],
    degree: usize,
    budget: &mut Budget,
) -> Result<Vec<Vec<f64>>, GeometryError> {
    let x = values.iter().map(|a| a[0]).collect::<Vec<_>>();
    let complement = values.iter().map(|a| a[1]).collect::<Vec<_>>();
    let mut result = vec![vec![1.]];
    for _ in 0..degree {
        let mut next = vec![vec![0.; result[0].len() + values.len() - 1]; result.len() + 1];
        for (i, previous) in result.iter().enumerate() {
            for (j, multiplier) in [&complement, &x].into_iter().enumerate() {
                for (target, value) in next[i + j]
                    .iter_mut()
                    .zip(product(previous, multiplier, budget)?)
                {
                    *target += value;
                }
            }
        }
        result = next;
    }
    Ok(result)
}

fn binomial(degree: usize) -> Vec<f64> {
    let mut result = vec![1.; degree + 1];
    for i in 1..=degree {
        result[i] = result[i - 1] * (degree + 1 - i) as f64 / i as f64;
    }
    result
}

fn product(a: &[f64], b: &[f64], budget: &mut Budget) -> Result<Vec<f64>, GeometryError> {
    budget.charge(a.len().saturating_mul(b.len()))?;
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
