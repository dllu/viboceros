//! Parameter-preserving approximation using the shared exact spline certificate.
use super::*;
use crate::Polyline3;

#[cfg(test)]
mod tests;

impl NurbsCurve {
    /// Approximates this curve by a polyline retaining its native parameter.
    /// Every segment has a continuous normalized-parameter error bound at most
    /// `tolerance.absolute()`, including endpoint rounding. Exact extraction and
    /// rational Bernstein differences reuse the surface correspondence engine.
    ///
    /// Supports degree <=16 and common-sign weights. Knot boundaries are kept;
    /// discontinuities, constant segments, unrepresentable parameters, and an
    /// exhausted bounded arithmetic/subdivision/output budget return an error.
    /// The source is unchanged. This bounds positional error, not topology.
    pub fn try_approximate_polyline(
        &self,
        tolerance: Tolerance,
    ) -> Result<Polyline3, GeometryError> {
        approximate(self, tolerance).map_err(|e| match e {
            GeometryError::SurfaceCurveCertificateWorkLimit => {
                GeometryError::CurveApproximationLimit
            }
            other => other,
        })
    }
}

fn approximate(curve: &NurbsCurve, tolerance: Tolerance) -> Result<Polyline3, GeometryError> {
    if curve.degree() > MAX_DEGREE {
        return Err(GeometryError::UnsupportedCurveApproximation {
            context: "degree exceeds 16",
        });
    }
    let mut budget = Budget(MAX_WORK);
    let spline = curve::Spline::spatial(curve, &mut budget)?.ok_or(
        GeometryError::UnsupportedCurveApproximation {
            context: "weights must have a common sign",
        },
    )?;
    let domain = curve.domain();
    let start = rational(*domain.start());
    let length = rational(*domain.end()) - &start;
    let normalized = |t| (rational(t) - &start) / &length;
    let mut parameters = Vec::new();
    let mut points = Vec::new();
    // Stream spans instead of putting a second copy of every control on a stack.
    for (left, right) in curve.spans() {
        let mut pending = vec![(left, right, 0)];
        while let Some((a, b, depth)) = pending.pop() {
            let net = spline.extract(&normalized(a), &normalized(b), &mut budget)?;
            let first = point(&net[0])?;
            let last = point(net.last().unwrap())?;
            let line = [first, last].map(|p| {
                let xyz = p.to_array();
                std::array::from_fn(|i| {
                    if i == 3 {
                        Rational::one()
                    } else {
                        rational(xyz[i])
                    }
                })
            });
            let difference = algebra::difference(&net, &line, &mut budget)?;
            if difference
                .iter()
                .all(|h| algebra::norm_bound(h, tolerance.absolute()).is_some())
            {
                if parameters.is_empty() {
                    parameters.push(a);
                    points.push(first);
                } else if parameters.last() != Some(&a) || points.last() != Some(&first) {
                    // Never bridge a full-order knot discontinuity or silently
                    // replace a one-sided limit by the other side's endpoint.
                    return Err(GeometryError::UnsupportedCurveApproximation {
                        context: "curve is discontinuous at a knot",
                    });
                }
                if first == last {
                    return Err(GeometryError::UnsupportedCurveApproximation {
                        context: "constant or unresolved closed segment",
                    });
                }
                if points.len() >= MAX_CURVE_DIVISION_POINTS {
                    return Err(GeometryError::CurveApproximationLimit);
                }
                parameters.push(b);
                points.push(last);
            } else {
                let middle = a.midpoint(b);
                if depth == MAX_DEPTH || middle <= a || middle >= b {
                    return Err(GeometryError::CurveApproximationLimit);
                }
                pending.push((middle, b, depth + 1));
                pending.push((a, middle, depth + 1));
            }
        }
    }
    Polyline3::try_with_parameters(points, parameters, Tolerance::NUMERICAL_VALIDATION)
}

fn point(h: &H) -> Result<Point3, GeometryError> {
    Point3::try_new(
        scalar(&(&h[0] / &h[3]))?,
        scalar(&(&h[1] / &h[3]))?,
        scalar(&(&h[2] / &h[3]))?,
    )
}
