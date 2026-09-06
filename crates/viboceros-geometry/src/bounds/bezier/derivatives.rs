//! Conservative rational partial-derivative signs. A strict sign rules out an
//! interior coordinate extremum; inconclusive intervals never discard one.
use super::{Budget, Net};
use crate::GeometryError;

impl Net {
    pub(in crate::bounds) fn derivative_signs(
        &self,
        budget: &mut Budget,
    ) -> Result<[[i8; 2]; 3], GeometryError> {
        budget.charge(self.controls.len().saturating_mul(24))?;
        let mut signs = [[0; 2]; 3];
        let w = range(self.controls.iter().map(|h| h[3]));
        if w[0] <= 0. && w[1] >= 0. {
            return Ok(signs);
        }
        for (coordinate, result) in signs.iter_mut().enumerate() {
            let center = self.controls[0][coordinate] / self.controls[0][3];
            let shifted = self
                .controls
                .iter()
                .map(|h| (-center).mul_add(h[3], h[coordinate]))
                .collect::<Vec<_>>();
            let y = range(shifted.iter().copied());
            for (axis, sign) in result.iter_mut().enumerate() {
                let mut dy = [f64::INFINITY, f64::NEG_INFINITY];
                let mut dw = dy;
                for line in 0..=self.degrees[1 - axis] {
                    for i in 0..self.degrees[axis] {
                        let a = self.index(axis, line, i);
                        let b = self.index(axis, line, i + 1);
                        // The omitted positive degree/domain factor does not
                        // affect a derivative's sign.
                        extend(&mut dy, shifted[b] - shifted[a]);
                        extend(&mut dw, self.controls[b][3] - self.controls[a][3]);
                    }
                }
                let left = product(dy, w);
                let right = product(y, dw);
                let low = left[0] - right[1];
                let high = left[1] - right[0];
                // Bound cancellation conservatively using uncentered input
                // magnitudes too; a large offset must not manufacture a sign.
                let input = self
                    .controls
                    .iter()
                    .map(|h| h[coordinate].abs().max((center * h[3]).abs()))
                    .fold(0., f64::max);
                let scale = input * w[0].abs().max(w[1].abs())
                    + left[0].abs().max(left[1].abs())
                    + right[0].abs().max(right[1].abs());
                // Do not infer strict signs from underflow-rounded products.
                let guard = (128. * f64::EPSILON * (1. + f64::from(self.depth)) * scale)
                    .max(64. * f64::MIN_POSITIVE);
                if low.is_finite() && high.is_finite() && guard.is_finite() {
                    *sign = if low > guard {
                        1
                    } else if high < -guard {
                        -1
                    } else {
                        0
                    };
                }
            }
        }
        Ok(signs)
    }
}

fn extend(range: &mut [f64; 2], x: f64) {
    range[0] = range[0].min(x);
    range[1] = range[1].max(x);
}
fn range(values: impl Iterator<Item = f64>) -> [f64; 2] {
    values.fold([f64::INFINITY, f64::NEG_INFINITY], |mut r, x| {
        extend(&mut r, x);
        r
    })
}
fn product(a: [f64; 2], b: [f64; 2]) -> [f64; 2] {
    range(a.into_iter().flat_map(|a| b.map(|b| a * b)))
}
