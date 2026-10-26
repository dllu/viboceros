//! Loft Refit with certified polygon regions and smooth cubic proposals.
use super::*;
use crate::exact_scalar::{Rational, rational, scalar};
use num_traits::{One, Signed, Zero};
mod candidate;
mod matching;
mod polyline;

struct Section {
    curve: NurbsCurve,
    vertices: Vec<[Rational; 3]>,
}

/// Refit ordered profiles to certified polynomial cubics. Zero retains the
/// original definitions. The polygon path requires same-sign weights and
/// continuous regions with ordered collinear controls, and
/// internal bends above the legacy one-degree corner threshold. Constant spans
/// and redundant collinear vertices disappear; true reversals remain.
/// With at least two profiles, a set of clamped single-Bezier sources may
/// instead use an endpoint-tangent cubic with at most one common handle correction
/// when its complete image error
/// is bounded by half the tolerance. Monotone correspondence proposals require
/// a complete certificate across every matching interval. Tolerance must be
/// smaller than every endpoint chord. Subdivision, multispan smooth sources
/// and mixed open/closed profiles remain unsupported.
///
/// Collinearity and control order are checked with exact rational arithmetic.
/// Positive rational basis functions then keep the source on its endpoint
/// segment. Region matching uses degree-one proxies before constructing a shared
/// cubic basis. Exact source vertices remain authoritative; direct construction
/// avoids rounded knot-insertion drift. One final rounding per control bounds
/// the complete image distance by `fit_tolerance`.
/// Unclamped endpoints use the shared bounded exact extraction, retaining their
/// rational values through handle construction and the final error check.
/// Smooth proposals use the shared endpoint-tangent constructor and correction;
/// its rounded controls require a continuous exact Bernstein certificate.
/// This changes parameter correspondence and never mutates the sources.
pub fn try_prepare_refitted_loft_sections(
    curves: &[NurbsCurve],
    fit_tolerance: Real,
) -> Result<Vec<NurbsCurve>, GeometryError> {
    prepare(curves, fit_tolerance, None)
}

/// Refit with an explicit corner angle in radians, in `[0, pi]`. The legacy
/// one-degree resolution floor still applies. Corners suppressed by the angle
/// require the adaptive smooth fitter and may remain unsupported.
pub fn try_prepare_refitted_loft_sections_with_angle(
    curves: &[NurbsCurve],
    fit_tolerance: Real,
    angle: Real,
) -> Result<Vec<NurbsCurve>, GeometryError> {
    prepare(curves, fit_tolerance, Some(angle))
}

fn validate_angle(angle: Option<Real>) -> Result<(), GeometryError> {
    if angle.is_some_and(|a| !a.is_finite() || !(0. ..=std::f64::consts::PI).contains(&a)) {
        return Err(GeometryError::InvalidContinuityAngle);
    }
    Ok(())
}

fn prepare(
    curves: &[NurbsCurve],
    fit_tolerance: Real,
    angle: Option<Real>,
) -> Result<Vec<NurbsCurve>, GeometryError> {
    validate_angle(angle)?;
    if !fit_tolerance.is_finite() || fit_tolerance < 0. {
        return Err(GeometryError::InvalidTolerance);
    }
    if curves.len() > MAX_LOFT_SECTIONS {
        return Err(GeometryError::LoftResourceLimit {
            context: "refitted section count",
            maximum: MAX_LOFT_SECTIONS,
        });
    }
    if fit_tolerance == 0. {
        return Ok(curves.to_vec());
    }
    let sections = curves
        .iter()
        .map(|source| match straight(source, fit_tolerance) {
            Err(GeometryError::UnsupportedCurveApproximation { .. }) => {
                polyline::refit(source, fit_tolerance, angle)
            }
            Ok(_) if angle.is_some_and(|a| a > std::f64::consts::FRAC_PI_2) => {
                Err(GeometryError::UnsupportedCurveApproximation {
                    context: "Loft Refit currently requires straight-section angles at most pi/2",
                })
            }
            result => result,
        })
        .collect::<Result<Vec<_>, GeometryError>>();
    let sections = match sections {
        Ok(sections) => sections,
        Err(GeometryError::UnsupportedCurveApproximation { .. })
            if curves.len() >= 2 && angle.is_none_or(|a| a <= std::f64::consts::FRAC_PI_2) =>
        {
            return candidate::prepare(curves, fit_tolerance);
        }
        Err(error) => return Err(error),
    };
    if let Some(first) = sections.first() {
        let closed = first.curve.is_closed()?;
        for section in &sections[1..] {
            if section.curve.is_closed()? != closed {
                return Err(GeometryError::UnsupportedCurveApproximation {
                    context: "Loft Refit currently requires profiles with matching open/closed state",
                });
            }
        }
    }
    matching::prepare(sections, fit_tolerance)
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
    loft(curves, style, closed, ends, fit_tolerance, None)
}

/// Ordered Refit Loft with an explicit corner angle in radians. Zero fitting
/// tolerance preserves the source definitions but still matches section regions.
pub fn try_loft_nurbs_curves_refitted_with_angle(
    curves: &[NurbsCurve],
    style: LoftStyle,
    closed: bool,
    ends: LoftEndPoints,
    fit_tolerance: Real,
    angle: Real,
) -> Result<NurbsSurface, GeometryError> {
    loft(curves, style, closed, ends, fit_tolerance, Some(angle))
}

fn loft(
    curves: &[NurbsCurve],
    style: LoftStyle,
    closed: bool,
    ends: LoftEndPoints,
    fit_tolerance: Real,
    angle: Option<Real>,
) -> Result<NurbsSurface, GeometryError> {
    if closed && !ends.is_empty() {
        return Err(GeometryError::InvalidControlNet {
            context: "closed Loft cannot have point ends",
        });
    }
    let mut prepare = |curves: &[NurbsCurve]| {
        prepare_refitted_loft_sections_with_operations(
            curves,
            fit_tolerance,
            angle,
            &mut |c| c.clamped_to_active_domain(),
            &mut |c, d| c.try_change_degree(d, false),
            &mut |c, k, m| c.try_insert_knot(k, m),
        )
    };
    if ends.is_empty() {
        try_loft_nurbs_curves_with_preparation(curves, style, closed, &mut prepare)
    } else {
        try_loft_nurbs_curves_with_end_points_and_operations(
            curves,
            style,
            ends,
            &mut prepare,
            &mut interpolate::solve_portable,
        )
    }
}

/// Refit profiles and prepare their shared basis with supplied exact edits.
/// A disabled fit still applies the Refit overload's region correspondence;
/// source definitions supplied to this operation are never mutated.
pub fn prepare_refitted_loft_sections_with_operations<E: From<GeometryError>>(
    curves: &[NurbsCurve],
    fit_tolerance: Real,
    angle: Option<Real>,
    clamp: &mut impl FnMut(&NurbsCurve) -> Result<NurbsCurve, E>,
    elevate: &mut impl FnMut(&NurbsCurve, usize) -> Result<NurbsCurve, E>,
    refine: &mut impl FnMut(&NurbsCurve, Real, usize) -> Result<NurbsCurve, E>,
) -> Result<Vec<NurbsCurve>, E> {
    let sections = prepare(curves, fit_tolerance, angle)?;
    if fit_tolerance == 0. {
        compatible::prepare_refit_with_operations(&sections, angle, clamp, elevate, refine)
    } else {
        compatible::prepare_with_operations(&sections, clamp, elevate, refine)
    }
}

fn straight(source: &NurbsCurve, tolerance: Real) -> Result<Section, GeometryError> {
    let unsupported = || GeometryError::UnsupportedCurveApproximation {
        context: "Loft Refit currently requires continuous non-reversing straight sections with same-sign weights",
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
    if knots
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
    let [start, end] = if knots[..=degree].iter().all(|k| k == domain.start())
        && knots[controls.len()..].iter().all(|k| k == domain.end())
    {
        [start, end]
    } else {
        crate::surface_pullback::exact_curve_endpoints(source)?.ok_or_else(unsupported)?
    };
    if start == end {
        return Err(unsupported());
    }
    let controls = cubic_controls(&start, &end, tolerance)?;
    Ok(Section {
        curve: NurbsCurve::try_new_rational(3, controls, vec![0., 0., 0., 0., 1., 1., 1., 1.])?,
        vertices: vec![start, end],
    })
}

fn cubic_controls(
    start: &[Rational; 3],
    end: &[Rational; 3],
    tolerance: Real,
) -> Result<Vec<WeightedPoint3>, GeometryError> {
    let delta = std::array::from_fn::<_, 3, _>(|i| &end[i] - &start[i]);
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
    Ok(result)
}

#[cfg(test)]
mod angle_tests;
#[cfg(test)]
mod polyline_tests;
#[cfg(test)]
mod tests;
