//! Adaptive Hermite proposals certified against the original stored spline.
use super::*;
use crate::{MAX_CURVE_FIT_CONTROL_POINTS, WeightedPoint3};

#[cfg(test)]
mod tests;

impl NurbsCurve {
    /// Approximates this spline with polynomial cubic pieces, retaining its
    /// native parameter domain and original knot boundaries. Every returned
    /// curve has a continuous positional error bound at absolute tolerance.
    /// See [`Self::try_approximate_cubic_with_bound`] for limits.
    pub fn try_approximate_cubic(&self, tolerance: Tolerance) -> Result<Self, GeometryError> {
        self.try_approximate_cubic_with_bound(tolerance)
            .map(|p| p.0)
    }

    /// Returns a polynomial cubic and a whole-domain parameter-correspondence
    /// bound. Local Hermite proposals interpolate exact endpoint positions and
    /// derivatives before their controls are rounded once to binary64. Exact
    /// rational Bernstein differences certify the rounded pieces and final net.
    /// No sampled-only proposal is accepted; the source remains unchanged.
    ///
    /// Common endpoints share one output control. Genuine source jumps retain
    /// independent one-sided limits at full-order knots. Constant spans and
    /// closed curves are supported. Tangents can change through control rounding;
    /// this certifies position, not smoothness, topology or injectivity.
    ///
    /// Supports degree <=16 and common-sign weights. At most 4096 controls,
    /// depth 48, and the shared exact certificate's arithmetic/work limits apply.
    /// Unrepresentable endpoints, unresolved intervals and exhausted budgets fail.
    /// This preserves source parameters; it does not implement Rhino Loft Refit.
    pub fn try_approximate_cubic_with_bound(
        &self,
        tolerance: Tolerance,
    ) -> Result<(Self, Real), GeometryError> {
        approximate(self, tolerance.absolute()).map_err(curve_pair::map_work_error)
    }
}

fn approximate(source: &NurbsCurve, limit: Real) -> Result<(NurbsCurve, Real), GeometryError> {
    if source.degree() > MAX_DEGREE {
        return Err(GeometryError::UnsupportedCurveApproximation {
            context: "degree exceeds 16",
        });
    }
    if source.spans().count() > (MAX_CURVE_FIT_CONTROL_POINTS - 1) / 3 {
        return Err(GeometryError::CurveApproximationLimit);
    }
    let mut budget = Budget(MAX_WORK);
    let spline = curve::Spline::spatial(source, &mut budget)?.ok_or(
        GeometryError::UnsupportedCurveApproximation {
            context: "weights must have a common sign",
        },
    )?;
    let domain = source.domain();
    let start = rational(*domain.start());
    let length = rational(*domain.end()) - &start;
    let normalized = |t| (rational(t) - &start) / &length;
    let mut output = Pieces::default();
    let mut coefficients = interval::Coefficients::default();
    for (a, b) in source.spans() {
        let net = spline.extract(&normalized(a), &normalized(b), &mut budget)?;
        let mut pending = vec![(a, b, net, 0)];
        while let Some((a, b, net, depth)) = pending.pop() {
            let proposal = hermite(&net, &mut budget)?;
            if let Some(controls) = proposal {
                let polynomial = controls.map(|p| {
                    let xyz = p.to_array();
                    std::array::from_fn(|i| {
                        if i == 3 {
                            Rational::one()
                        } else {
                            rational(xyz[i])
                        }
                    })
                });
                match interval::curves(
                    &net,
                    &polynomial,
                    limit,
                    &mut coefficients,
                    &mut budget,
                    false,
                )? {
                    interval::Outcome::Within(_) => {
                        output.append([a, b], controls, &net)?;
                        continue;
                    }
                    interval::Outcome::Outside => {
                        return Err(GeometryError::UnsupportedCurveApproximation {
                            context: "rounded endpoint exceeds the approximation tolerance",
                        });
                    }
                    interval::Outcome::Inconclusive => {}
                }
                let difference = algebra::difference(&net, &polynomial, &mut budget)?;
                if algebra::norm_bound(&difference[0], limit).is_none()
                    || algebra::norm_bound(difference.last().unwrap(), limit).is_none()
                {
                    return Err(GeometryError::UnsupportedCurveApproximation {
                        context: "rounded endpoint exceeds the approximation tolerance",
                    });
                }
                // Coefficient bounds are sufficient and deliberately avoid a
                // second adaptive tree for a candidate that can be refined.
                if difference
                    .iter()
                    .all(|p| algebra::norm_bound(p, limit).is_some())
                {
                    output.append([a, b], controls, &net)?;
                    continue;
                }
            }
            let middle = a.midpoint(b);
            if depth == MAX_DEPTH || middle <= a || middle >= b {
                return Err(GeometryError::CurveApproximationLimit);
            }
            let (left, right) = if rational(middle) * rational(2.) == rational(a) + rational(b) {
                algebra::split(&net, &mut budget)?
            } else {
                (
                    spline.extract(&normalized(a), &normalized(middle), &mut budget)?,
                    spline.extract(&normalized(middle), &normalized(b), &mut budget)?,
                )
            };
            pending.push((middle, b, right, depth + 1));
            pending.push((a, middle, left, depth + 1));
        }
    }
    let curve = output.finish()?;
    let bound = source
        .parameter_curve_deviation_bound(&curve, limit)?
        .ok_or(GeometryError::UnsupportedCurveApproximation {
            context: "assembled cubic failed its source correspondence certificate",
        })?;
    Ok((curve, bound))
}

fn hermite(net: &[H], budget: &mut Budget) -> Result<Option<[Point3; 4]>, GeometryError> {
    budget.charge(12)?;
    let p = Rational::from_integer((net.len() - 1).into());
    let first = &net[0];
    let last = net.last().unwrap();
    let start: [Rational; 3] = std::array::from_fn(|i| &first[i] / &first[3]);
    let end: [Rational; 3] = std::array::from_fn(|i| &last[i] / &last[3]);
    let exact: [[Rational; 3]; 4] = [
        start.clone(),
        std::array::from_fn(|i| {
            &start[i] + &p * (&net[1][i] - &start[i] * &net[1][3]) / &first[3] / rational(3.)
        }),
        std::array::from_fn(|i| {
            &end[i]
                - &p * (&end[i] * &net[net.len() - 2][3] - &net[net.len() - 2][i])
                    / &last[3]
                    / rational(3.)
        }),
        end,
    ];
    for coordinate in exact.iter().flatten() {
        budget.check(coordinate)?;
    }
    let mut points = [Point3::try_new(0., 0., 0.)?; 4];
    for (i, value) in exact.iter().enumerate() {
        let coordinates = value.iter().map(scalar).collect::<Result<Vec<_>, _>>();
        match coordinates {
            Ok(v) => points[i] = Point3::try_new(v[0], v[1], v[2])?,
            Err(_) if i == 1 || i == 2 => return Ok(None),
            Err(e) => return Err(e),
        }
    }
    Ok(Some(points))
}

#[derive(Default)]
struct Pieces {
    controls: Vec<Point3>,
    knots: Vec<Real>,
    last: Option<H>,
    end: Real,
}

impl Pieces {
    fn append(
        &mut self,
        interval: [Real; 2],
        controls: [Point3; 4],
        net: &[H],
    ) -> Result<(), GeometryError> {
        let joined = self
            .last
            .as_ref()
            .is_some_and(|last| (0..3).all(|i| &last[i] * &net[0][3] == &net[0][i] * &last[3]));
        let skip = usize::from(joined);
        if self.controls.len() + 4 - skip > MAX_CURVE_FIT_CONTROL_POINTS {
            return Err(GeometryError::CurveApproximationLimit);
        }
        debug_assert!(self.controls.is_empty() || self.end == interval[0]);
        debug_assert!(!joined || self.controls.last() == Some(&controls[0]));
        self.knots
            .extend(std::iter::repeat_n(interval[0], 4 - skip));
        self.controls.extend_from_slice(&controls[skip..]);
        self.last = Some(net.last().unwrap().clone());
        self.end = interval[1];
        Ok(())
    }

    fn finish(mut self) -> Result<NurbsCurve, GeometryError> {
        self.knots.extend(std::iter::repeat_n(self.end, 4));
        NurbsCurve::try_new_rational(
            3,
            self.controls
                .into_iter()
                .map(|p| WeightedPoint3::try_new(p, 1.))
                .collect::<Result<Vec<_>, _>>()?,
            self.knots,
        )
    }
}
