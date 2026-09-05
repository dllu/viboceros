//! Locus-preserving Möbius reparameterization, independent of weight gauge.

use super::{GeometryError, NurbsCurve, Real, WeightedPoint3, rescale_controls};
use crate::nurbs::reparameterize_value;

#[cfg(test)]
mod tests;

/// Piecewise-Bezier trimming retains every interior break and changes only
/// the two outer spans. Its requested inner endpoint weights need not be one.
pub(crate) fn change_bezier_end_weights(
    controls: &mut [WeightedPoint3],
    desired_start: Real,
    desired_end: Real,
) -> Result<(), GeometryError> {
    debug_assert!(controls.len() >= 2);
    let last = controls.len() - 1;
    let start = controls[0].weight();
    let end = controls[last].weight();
    if start == desired_start && end == desired_end {
        return Ok(());
    }
    let first = WeightedPoint3::try_new(controls[0].point(), desired_start)?;
    let final_control = WeightedPoint3::try_new(controls[last].point(), desired_end)?;
    let same_sign = desired_start.is_sign_positive() == start.is_sign_positive();
    if same_sign != (desired_end.is_sign_positive() == end.is_sign_positive()) {
        return Err(GeometryError::InvalidControlNet {
            context: "Bezier endpoint-weight changes require a positive projective factor",
        });
    }
    // w'_i = w_i * (desired_start/start)^(1-i/p)
    //              * (desired_end/end)^(i/p).
    // Form logs of ratios before combining with the desired weight gauge.
    // Neither the common multiplier nor the projective factor must fit f64.
    let changed = controls
        .iter()
        .enumerate()
        .map(|(i, control)| {
            if i == 0 {
                return Ok(first);
            }
            if i == last {
                return Ok(final_control);
            }
            let fraction = i as Real / last as Real;
            let a = log_ratio(control.weight().abs(), start.abs()) + desired_start.abs().ln();
            let b = log_ratio(control.weight().abs(), end.abs()) + desired_end.abs().ln();
            let magnitude = ((1.0 - fraction) * a + fraction * b).exp();
            let sign = if control.weight().is_sign_positive() == same_sign {
                1.0
            } else {
                -1.0
            };
            WeightedPoint3::try_new(control.point(), sign * magnitude)
        })
        .collect::<Result<Vec<_>, GeometryError>>()?;
    controls.copy_from_slice(&changed);
    Ok(())
}

impl NurbsCurve {
    /// Projectively reparameterizes a clamped curve so its two end weights
    /// are exactly one. Opposite-sign endpoints cannot be normalized this way.
    ///
    /// Clamping preserves the active curve; subsequent normalization retains
    /// its control locations, domain, and geometric locus up to roundoff.
    /// Interior knots and parameter-to-point correspondence generally change.
    /// Unlike OpenNURBS' `ChangeEndWeights` near-equal-weight shortcut, this
    /// operation does not approximate unequal endpoint weights as equal.
    ///
    /// Returns an error if a resulting weight or distinct knot interval cannot
    /// be represented in `f64`. No interior span is silently collapsed.
    pub fn try_normalized_end_weights(&self) -> Result<Self, GeometryError> {
        let curve = self.clamped_to_active_domain()?;
        let count = curve.control_points.len();
        let start = curve.control_points[0].weight();
        let end = curve.control_points[count - 1].weight();
        if start.is_sign_positive() != end.is_sign_positive() {
            return Err(GeometryError::InvalidControlNet {
                context: "NURBS endpoint weights must have the same sign",
            });
        }
        if start == end {
            // Divide weights directly: the reciprocal of a valid subnormal
            // common scale need not be representable.
            return Self::try_new_rational(
                curve.degree,
                rescale_controls(&curve.control_points, start, 1.0)?,
                curve.knots,
            );
        }

        let degree = curve.degree as Real;
        let log_c = log_ratio(end.abs(), start.abs()) / degree;
        let domain = curve.domain();
        let a = *domain.start();
        let b = *domain.end();
        let mapped = curve
            .knots
            .iter()
            .map(|&knot| {
                let u = reparameterize_value(knot, a, b, 0.0, 1.0)?;
                Ok(MappedKnot::new(u, log_c))
            })
            .collect::<Result<Vec<_>, GeometryError>>()?;
        let knots = mapped
            .iter()
            .map(|knot| reparameterize_value(knot.value, 0.0, 1.0, a, b))
            .collect::<Result<Vec<_>, _>>()?;
        for (old, new) in curve.knots.windows(2).zip(knots.windows(2)) {
            if old[0] < old[1] && new[0] >= new[1] {
                return Err(GeometryError::InvalidKnotVector {
                    context: "endpoint-weight normalization collapsed a distinct knot interval",
                });
            }
        }
        let controls = curve
            .control_points
            .iter()
            .enumerate()
            .map(|(i, control)| {
                let weight = if i == 0 || i + 1 == count {
                    1.0
                } else {
                    let from_start = log_ratio(control.weight().abs(), start.abs()) / degree;
                    let from_end = log_ratio(control.weight().abs(), end.abs()) / degree;
                    // For each knot, use the smaller logarithmic correction.
                    // This avoids subtracting the full log(c) at endpoints
                    // and works even when c or a weight ratio would overflow.
                    let log_weight: Real = mapped[i + 1..i + 1 + curve.degree]
                        .iter()
                        .map(|knot| {
                            if knot.from_start.abs() <= knot.from_end.abs() {
                                from_start + knot.from_start
                            } else {
                                from_end + knot.from_end
                            }
                        })
                        .sum();
                    let sign = if control.weight().is_sign_positive() == start.is_sign_positive() {
                        1.0
                    } else {
                        -1.0
                    };
                    sign * log_weight.exp()
                };
                WeightedPoint3::try_new(control.point(), weight)
            })
            .collect::<Result<Vec<_>, _>>()?;
        Self::try_new_rational(curve.degree, controls, knots)
    }
}

/// log(x/y) for finite positive weights. Preserve small relative differences
/// without subtracting nearly equal large logarithms or overflowing x/y.
fn log_ratio(x: Real, y: Real) -> Real {
    let ratio = x / y;
    if (0.5..=2.0).contains(&ratio) {
        ((x - y) / y).ln_1p()
    } else if ratio.is_normal() {
        ratio.ln()
    } else {
        x.ln() - y.ln()
    }
}

struct MappedKnot {
    value: Real,
    from_start: Real,
    from_end: Real,
}

impl MappedKnot {
    fn new(u: Real, log_c: Real) -> Self {
        // v = c*u / (1-u+c*u). Per-knot weight corrections are -log(D)
        // when anchored at the start weight, or log(c)-log(D) at the end.
        // Endpoint branches also handle unrepresentable c on Bezier curves.
        let (value, from_start, from_end) = if u == 0.0 {
            (0.0, 0.0, log_c)
        } else if u == 1.0 {
            (1.0, -log_c, 0.0)
        } else if log_c.abs() <= 0.5 {
            let delta = u * log_c.exp_m1();
            let log_d = delta.ln_1p();
            ((u + delta) / (1.0 + delta), -log_d, log_c - log_d)
        } else if log_c > 0.0 {
            let inverse_c = (-log_c).exp();
            if !inverse_c.is_normal() {
                return Self::from_logit(u, log_c);
            }
            let scaled_d = u + (1.0 - u) * inverse_c;
            let log_scaled_d = scaled_d.ln();
            (u / scaled_d, -log_c - log_scaled_d, -log_scaled_d)
        } else {
            let c = log_c.exp();
            if !c.is_normal() {
                return Self::from_logit(u, log_c);
            }
            let numerator = u * c;
            let d = 1.0 - u + numerator;
            let log_d = d.ln();
            (numerator / d, -log_d, log_c - log_d)
        };
        Self {
            value,
            from_start,
            from_end,
        }
    }

    fn from_logit(u: Real, log_c: Real) -> Self {
        // A rounded subnormal c (or 1/c) can cause order-one errors when a
        // knot is similarly tiny. Work entirely in logarithms before the
        // final bounded logistic map; do not amplify that intermediate loss.
        let log_u = u.ln();
        let log_complement = (-u).ln_1p();
        let logit = log_c + log_u - log_complement;
        if logit >= 0.0 {
            let inverse_ratio = (-logit).exp();
            let from_end = -log_u - inverse_ratio.ln_1p();
            Self {
                value: 1.0 / (1.0 + inverse_ratio),
                from_start: from_end - log_c,
                from_end,
            }
        } else {
            let ratio = logit.exp();
            let from_start = -log_complement - ratio.ln_1p();
            Self {
                value: ratio / (1.0 + ratio),
                from_start,
                from_end: log_c + from_start,
            }
        }
    }
}
