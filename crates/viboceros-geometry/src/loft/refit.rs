//! The straight-profile part of Loft Refit, without a sampled linearity test.
use super::*;
use crate::exact_scalar::{Rational, rational, scalar};
use num_traits::{One, Signed, Zero};

/// Refit ordered straight profiles to polynomial cubics. Zero retains the
/// original definitions. Positive tolerance currently requires clamped,
/// continuous, non-reversing collinear control polygons with same-sign weights.
/// Other profiles fail explicitly; the general curved Refit policy is pending.
///
/// Collinearity and control order are checked with exact rational arithmetic.
/// Positive rational basis functions then keep the source on its endpoint
/// segment. The cubic uses uniform line speed and one final rounding per handle;
/// its whole-image distance from that segment is bounded by `fit_tolerance`.
/// This changes parameter correspondence and never mutates the sources.
pub fn try_prepare_refitted_loft_sections(
    curves: &[NurbsCurve],
    fit_tolerance: Real,
) -> Result<Vec<NurbsCurve>, GeometryError> {
    if !fit_tolerance.is_finite() || fit_tolerance < 0. {
        return Err(GeometryError::InvalidTolerance);
    }
    if curves.len() > MAX_LOFT_SECTIONS {
        return Err(GeometryError::LoftResourceLimit {
            context: "refitted section count",
            maximum: MAX_LOFT_SECTIONS,
        });
    }
    curves
        .iter()
        .map(|source| {
            if fit_tolerance == 0. {
                Ok(source.clone())
            } else {
                straight(source, fit_tolerance)
            }
        })
        .collect()
}

/// Loft the refitted ordered sections, retaining the usual style, closed-loft
/// and point-end policies. See [`try_prepare_refitted_loft_sections`] for the
/// currently supported profiles and continuous image-distance guarantee.
pub fn try_loft_nurbs_curves_refitted(
    curves: &[NurbsCurve],
    style: LoftStyle,
    closed: bool,
    ends: LoftEndPoints,
    fit_tolerance: Real,
) -> Result<NurbsSurface, GeometryError> {
    if closed && !ends.is_empty() {
        return Err(GeometryError::InvalidControlNet {
            context: "closed Loft cannot have point ends",
        });
    }
    if ends.is_empty() {
        validate_count(curves.len(), closed)?;
    } else {
        validate_end_points(curves.len(), style, ends)?;
    }
    let sections = try_prepare_refitted_loft_sections(curves, fit_tolerance)?;
    if ends.is_empty() {
        try_loft_nurbs_curves(&sections, style, closed)
    } else {
        try_loft_nurbs_curves_with_end_points(&sections, style, ends)
    }
}

fn straight(source: &NurbsCurve, tolerance: Real) -> Result<NurbsCurve, GeometryError> {
    let unsupported = || GeometryError::UnsupportedCurveApproximation {
        context: "Loft Refit currently requires continuous clamped non-reversing straight sections with same-sign weights",
    };
    let controls = source.control_points();
    if controls.len() > MAX_LOFT_SECTION_CONTROLS {
        return Err(GeometryError::LoftResourceLimit {
            context: "Refit source section controls",
            maximum: MAX_LOFT_SECTION_CONTROLS,
        });
    }
    let knots = source.knots();
    let degree = source.degree();
    let domain = source.domain();
    if knots[..=degree].iter().any(|k| k != domain.start())
        || knots[controls.len()..].iter().any(|k| k != domain.end())
        || knots
            .windows(degree + 1)
            .any(|w| w[0] > *domain.start() && w[0] < *domain.end() && w[0] == w[degree])
        || controls
            .iter()
            .any(|c| c.weight().is_sign_negative() != controls[0].weight().is_sign_negative())
    {
        return Err(unsupported());
    }
    let start = controls[0].point().to_array().map(rational);
    let end = controls.last().unwrap().point().to_array().map(rational);
    let delta = std::array::from_fn::<_, 3, _>(|i| &end[i] - &start[i]);
    let axis = delta
        .iter()
        .position(|v| !v.is_zero())
        .ok_or_else(unsupported)?;
    let mut previous = Rational::zero();
    for control in controls {
        let point = control.point().to_array().map(rational);
        let parameter = (&point[axis] - &start[axis]) / &delta[axis];
        if parameter < previous
            || parameter > Rational::one()
            || (0..3).any(|i| &point[i] - &start[i] != &parameter * &delta[i])
        {
            return Err(unsupported());
        }
        previous = parameter;
    }
    let limit = rational(tolerance);
    let mut result = Vec::with_capacity(4);
    for index in 0..=3 {
        let fraction = rational(f64::from(index)) / rational(3.);
        let exact = std::array::from_fn::<_, 3, _>(|i| &start[i] + &fraction * &delta[i]);
        let [x, y, z] = exact.each_ref().map(scalar);
        let xyz = [x?, y?, z?];
        // L1 bounds Euclidean distance; convex combinations bound the complete
        // cubic against the ideal uniformly parameterized endpoint segment.
        let error = (0..3)
            .map(|i| (rational(xyz[i]) - &exact[i]).abs())
            .sum::<Rational>();
        if error > limit {
            return Err(GeometryError::UnsupportedCurveApproximation {
                context: "Loft Refit cubic handle rounding exceeds the requested tolerance",
            });
        }
        result.push(WeightedPoint3::try_new(crate::Point3::try_from(xyz)?, 1.)?);
    }
    NurbsCurve::try_new_rational(3, result, vec![0., 0., 0., 0., 1., 1., 1., 1.])
}

#[cfg(test)]
mod tests;
