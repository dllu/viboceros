//! Local homogeneous nets retain zero intermediate weights during subdivision.
use super::*;

#[derive(Clone)]
pub(super) struct Net {
    degrees: [usize; 2],
    controls: Vec<[Real; 4]>,
}
impl Net {
    pub fn new(surface: &NurbsSurface) -> Result<Self, GeometryError> {
        let origin = surface.control_points()[0].point().to_array();
        let scale = surface
            .control_points()
            .iter()
            .map(|p| p.weight().abs())
            .fold(0., Real::max);
        let controls = surface
            .control_points()
            .iter()
            .map(|c| {
                let w = c.weight() / scale;
                let point = c.point().to_array();
                let h = [
                    (point[0] - origin[0]) * w,
                    (point[1] - origin[1]) * w,
                    (point[2] - origin[2]) * w,
                    w,
                ];
                if w == 0. || h.iter().any(|v| !v.is_finite()) {
                    Err(GeometryError::MeshSubdivisionLimit)
                } else {
                    Ok(h)
                }
            })
            .collect::<Result<Vec<_>, _>>()?;
        Ok(Self {
            degrees: [surface.degree_u(), surface.degree_v()],
            controls,
        })
    }
    pub fn work(&self) -> usize {
        self.controls
            .len()
            .saturating_mul(self.degrees[0] + self.degrees[1] + 8)
    }
    fn index(&self, axis: usize, line: usize, i: usize) -> usize {
        if axis == 0 {
            line * (self.degrees[0] + 1) + i
        } else {
            i * (self.degrees[0] + 1) + line
        }
    }
    pub fn split(self, axis: usize) -> [Self; 2] {
        let degree = self.degrees[axis];
        let mut left = self.controls.clone();
        let mut right = self.controls.clone();
        let mut work = vec![[0.; 4]; degree + 1];
        for line in 0..=self.degrees[1 - axis] {
            for (i, p) in work.iter_mut().enumerate() {
                *p = self.controls[self.index(axis, line, i)];
            }
            for level in 1..=degree {
                for i in 0..=degree - level {
                    work[i] =
                        std::array::from_fn(|k| 0.5_f64.mul_add(work[i][k], 0.5 * work[i + 1][k]));
                }
                left[self.index(axis, line, level)] = work[0];
                right[self.index(axis, line, degree - level)] = work[degree - level];
            }
        }
        [
            Self {
                degrees: self.degrees,
                controls: left,
            },
            Self {
                degrees: self.degrees,
                controls: right,
            },
        ]
    }

    pub fn weight_variation_axis(&self) -> usize {
        let variation = [0, 1].map(|axis| {
            let mut value: Real = 0.;
            for line in 0..=self.degrees[1 - axis] {
                for i in 0..self.degrees[axis] {
                    value = value.max(
                        (self.controls[self.index(axis, line, i)][3]
                            - self.controls[self.index(axis, line, i + 1)][3])
                            .abs(),
                    );
                }
            }
            value
        });
        usize::from(variation[1] > variation[0])
    }

    /// Bound the parameter-corresponding distance to either diagonal's
    /// piecewise linear mesh. No midpoint/normal sampling is used.
    /// This is floating-point algebra with a rounding allowance, not interval arithmetic.
    pub fn chord_bound(&self) -> Option<Real> {
        let sign = self.controls[0][3].is_sign_positive();
        if self
            .controls
            .iter()
            .any(|p| p[3] == 0. || p[3].is_sign_positive() != sign)
        {
            return None;
        }
        let [p, q] = self.degrees;
        let project = |i: usize| {
            let h = self.controls[i];
            [h[0] / h[3], h[1] / h[3], h[2] / h[3]]
        };
        let corners = [
            project(0),
            project(p),
            project(q * (p + 1)),
            project((q + 1) * (p + 1) - 1),
        ];
        if corners.iter().flatten().any(|v| !v.is_finite()) {
            return None;
        }
        let mut error: Real = 0.;
        for j in 0..=q + 1 {
            for i in 0..=p + 1 {
                // S-L = (N-WL)/W, where L is the corner bilinear patch.
                // Elevating N,W and multiplying W by L give degree (p+1,q+1).
                // Same-sign denominator coefficients make the resulting
                // rational residual a convex combination of these controls.
                let mut numerator = [0.; 3];
                let mut denominator = 0.;
                for b in 0..=1 {
                    for a in 0..=1 {
                        if i < a || i - a > p || j < b || j - b > q {
                            continue;
                        }
                        let cu = if a == 0 {
                            (p + 1 - i) as Real
                        } else {
                            i as Real
                        } / (p + 1) as Real;
                        let cv = if b == 0 {
                            (q + 1 - j) as Real
                        } else {
                            j as Real
                        } / (q + 1) as Real;
                        let h = self.controls[(j - b) * (p + 1) + i - a];
                        let coefficient = cu * cv;
                        denominator += coefficient * h[3];
                        for k in 0..3 {
                            numerator[k] +=
                                coefficient * (-h[3]).mul_add(corners[b * 2 + a][k], h[k]);
                        }
                    }
                }
                let r = numerator.map(|n| n / denominator);
                if denominator == 0. || !denominator.is_finite() || r.iter().any(|v| !v.is_finite())
                {
                    return None;
                }
                error = error.max(r[0].hypot(r[1]).hypot(r[2]));
            }
        }
        // Bilinear-to-triangle displacement is at most |twist|/4.
        let twist: [Real; 3] = std::array::from_fn(|i| {
            (corners[0][i] - corners[1][i]) + (corners[3][i] - corners[2][i])
        });
        error += 0.25 * twist[0].hypot(twist[1]).hypot(twist[2]);
        let scale = self
            .controls
            .iter()
            .flat_map(|h| [h[0] / h[3], h[1] / h[3], h[2] / h[3]])
            .map(Real::abs)
            .fold(0., Real::max);
        error += scale * Real::EPSILON * 64.;
        error.is_finite().then_some(error.next_up())
    }
}
