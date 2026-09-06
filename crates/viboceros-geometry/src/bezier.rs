//! Local projective Bezier extraction; no repeated whole-spline splitting.
use crate::{GeometryError, NurbsCurve, NurbsSurface, Point3, WeightedPoint3};

#[cfg(test)]
mod tests;

/// Aggregate output-control limit for one curve/surface decomposition.
pub const MAX_BEZIER_CONTROL_POINTS: usize = 1_048_576;
const MAX_WORK: usize = 33_554_432;
type H = [f64; 4];

#[derive(Default)]
struct Budget {
    work: usize,
    controls: usize,
}
impl Budget {
    fn output(&mut self, count: usize) -> Result<(), GeometryError> {
        self.controls = self.controls.saturating_add(count);
        if self.controls > MAX_BEZIER_CONTROL_POINTS {
            return Err(GeometryError::BezierDecompositionLimit);
        }
        self.charge(count)
    }
    fn charge(&mut self, count: usize) -> Result<(), GeometryError> {
        self.work = self.work.saturating_add(count);
        if self.work > MAX_WORK {
            Err(GeometryError::BezierDecompositionLimit)
        } else {
            Ok(())
        }
    }
}

fn isolated(degree: usize, knots: &[f64], span: usize) -> bool {
    knots[span - degree..=span]
        .iter()
        .all(|k| *k == knots[span])
        && knots[span + 1..=span + degree + 1]
            .iter()
            .all(|k| *k == knots[span + 1])
}

/// Polar-form evaluation of p-i left and i right arguments. All intermediate
/// coordinates remain homogeneous: an intermediate zero weight is not a pole.
/// The caller supplies exactly the active p+1 controls at a validated span.
pub(crate) fn extract_homogeneous_span(
    degree: usize,
    knots: &[f64],
    span: usize,
    source: &[H],
) -> Result<Vec<H>, GeometryError> {
    debug_assert_eq!(source.len(), degree + 1);
    if isolated(degree, knots, span) {
        return Ok(source.to_vec());
    }
    let (a, b) = (knots[span], knots[span + 1]);
    let mut output = Vec::with_capacity(degree + 1);
    let mut work = source.to_vec();
    for end_arguments in 0..=degree {
        work.copy_from_slice(source);
        for level in 1..=degree {
            let t = if level <= degree - end_arguments {
                a
            } else {
                b
            };
            for j in (level..=degree).rev() {
                let k = span - degree + j;
                let alpha =
                    crate::nurbs::interval_fraction(t, knots[k], knots[k + degree - level + 1])?;
                work[j] = if alpha == 0. {
                    work[j - 1]
                } else if alpha == 1. {
                    work[j]
                } else {
                    std::array::from_fn(|axis| {
                        work[j - 1][axis].mul_add(1. - alpha, work[j][axis] * alpha)
                    })
                };
            }
        }
        if work[degree].iter().any(|x| !x.is_finite()) {
            return Err(GeometryError::UnrepresentableBezierControl);
        }
        output.push(work[degree]);
    }
    Ok(output)
}

/// A local affine origin and common weight scale prevent otherwise avoidable
/// overflow/cancellation. Unlike the bounds net, output weights keep their gauge.
struct LocalControls {
    origin: [f64; 3],
    scale: f64,
    values: Vec<H>,
}
impl LocalControls {
    fn new(controls: &[WeightedPoint3]) -> Result<Self, GeometryError> {
        let candidate = controls[0].point().to_array();
        let origin = if controls.iter().all(|c| {
            c.point()
                .to_array()
                .iter()
                .zip(candidate)
                .all(|(a, b)| (a - b).is_finite())
        }) {
            candidate
        } else {
            [0.; 3]
        };
        let scale = controls.iter().map(|c| c.weight().abs()).fold(0., f64::max);
        let values = controls
            .iter()
            .map(|c| {
                let w = c.weight() / scale;
                if w == 0. {
                    return Err(GeometryError::UnrepresentableBezierControl);
                }
                let p = c.point().to_array();
                Ok([
                    (p[0] - origin[0]) * w,
                    (p[1] - origin[1]) * w,
                    (p[2] - origin[2]) * w,
                    w,
                ])
            })
            .collect::<Result<Vec<_>, _>>()?;
        Ok(Self {
            origin,
            scale,
            values,
        })
    }
    fn project(self) -> Result<Vec<WeightedPoint3>, GeometryError> {
        self.values
            .into_iter()
            .map(|h| {
                let weight = h[3] * self.scale;
                if weight == 0. || !weight.is_finite() {
                    return Err(GeometryError::UnrepresentableBezierControl);
                }
                let p = std::array::from_fn(|i| {
                    let x = h[i] / h[3] + self.origin[i];
                    if x.is_finite() {
                        x
                    } else {
                        self.origin[i].mul_add(h[3], h[i]) / h[3]
                    }
                });
                WeightedPoint3::try_new(Point3::try_from(p)?, weight)
            })
            .collect()
    }
}

fn unit_span_knots(degree: usize, a: f64, b: f64) -> Vec<f64> {
    std::iter::repeat_n(a, degree + 1)
        .chain(std::iter::repeat_n(b, degree + 1))
        .collect()
}

fn charge_span(
    budget: &mut Budget,
    degree: usize,
    knots: &[f64],
    span: usize,
    lines: usize,
) -> Result<(), GeometryError> {
    let per_line = if isolated(degree, knots, span) {
        degree + 1
    } else {
        (degree + 1).saturating_pow(3)
    };
    budget.charge(lines.saturating_mul(per_line))
}

impl NurbsCurve {
    /// Exactly extracts each nonempty active knot span as a clamped Bezier
    /// NURBS curve, retaining its source interval and common weight gauge.
    /// Covers unclamped, periodic, signed-rational and full-order knot vectors.
    /// Intermediate zero weights are allowed; final controls at infinity or an
    /// exhausted work/output budget return an error without changing the source.
    pub fn try_bezier_spans(&self) -> Result<Vec<Self>, GeometryError> {
        let p = self.degree();
        let mut budget = Budget::default();
        budget.output(self.spans().count().saturating_mul(p + 1))?;
        let mut outputs = Vec::new();
        for span in p..self.control_points().len() {
            let (a, b) = (self.knots()[span], self.knots()[span + 1]);
            if a == b {
                continue;
            }
            charge_span(&mut budget, p, self.knots(), span, 1)?;
            let controls = &self.control_points()[span - p..=span];
            let controls = if isolated(p, self.knots(), span) {
                controls.to_vec()
            } else {
                let mut local = LocalControls::new(controls)?;
                local.values = extract_homogeneous_span(p, self.knots(), span, &local.values)?;
                local.project()?
            };
            outputs.push(Self::try_new_rational(
                p,
                controls,
                unit_span_knots(p, a, b),
            )?);
        }
        Ok(outputs)
    }
}

impl NurbsSurface {
    /// Exactly extracts all active knot rectangles into clamped tensor-product
    /// Bezier NURBS patches, U-span outer/V-span inner, with source UV domains.
    /// Work and aggregate output controls are bounded independently of trimming.
    pub fn try_bezier_patches(&self) -> Result<Vec<Self>, GeometryError> {
        let (p, q) = (self.degree_u(), self.degree_v());
        let (width, height) = (p + 1, q + 1);
        let count = self
            .spans_u()
            .count()
            .saturating_mul(self.spans_v().count());
        let mut budget = Budget::default();
        budget.output(count.saturating_mul(width).saturating_mul(height))?;
        let mut outputs = Vec::new();
        for u in p..self.control_point_count_u() {
            let (a, b) = (self.knots_u()[u], self.knots_u()[u + 1]);
            if a == b {
                continue;
            }
            for v in q..self.control_point_count_v() {
                let (c, d) = (self.knots_v()[v], self.knots_v()[v + 1]);
                if c == d {
                    continue;
                }
                charge_span(&mut budget, p, self.knots_u(), u, height)?;
                charge_span(&mut budget, q, self.knots_v(), v, width)?;
                let controls = (v - q..=v)
                    .flat_map(|j| (u - p..=u).map(move |i| self.control_point(i, j).unwrap()))
                    .collect::<Vec<_>>();
                let controls = if isolated(p, self.knots_u(), u) && isolated(q, self.knots_v(), v) {
                    controls
                } else {
                    let mut local = LocalControls::new(&controls)?;
                    for row in local.values.chunks_exact_mut(width) {
                        row.copy_from_slice(&extract_homogeneous_span(p, self.knots_u(), u, row)?);
                    }
                    for column in 0..width {
                        let source = (0..height)
                            .map(|j| local.values[j * width + column])
                            .collect::<Vec<_>>();
                        let result = extract_homogeneous_span(q, self.knots_v(), v, &source)?;
                        for (j, value) in result.into_iter().enumerate() {
                            local.values[j * width + column] = value;
                        }
                    }
                    local.project()?
                };
                outputs.push(Self::try_new_rational(
                    p,
                    q,
                    width,
                    height,
                    controls,
                    unit_span_knots(p, a, b),
                    unit_span_knots(q, c, d),
                )?);
            }
        }
        Ok(outputs)
    }
}
