//! Loft Refit with certified polygon regions and smooth cubic proposals.
use super::*;
use crate::exact_scalar::{Rational, rational, scalar};
use num_traits::{One, Signed, Zero};
mod adaptive;
pub use adaptive::{
    AdaptiveLinearRefitProposal, try_propose_adaptive_linear_refit,
    try_propose_joined_linear_refit, try_propose_joined_refit,
};
mod candidate;
mod matching;
mod polyline;
mod preparation;
mod sweep;
pub(crate) use sweep::prepare_with_operations as prepare_sweep_with_operations;

#[derive(Clone, Copy)]
struct FitScope {
    minimum_curves: usize,
    policy: adaptive::FitPolicy,
}
impl FitScope {
    const LOFT: Self = Self {
        minimum_curves: 2,
        policy: adaptive::FitPolicy::LOFT,
    };
}

struct Section {
    curve: NurbsCurve,
    vertices: Vec<[Rational; 3]>,
}

/// Refit ordered profiles to certified polynomial cubics. Tolerances at most
/// twice the square root of machine epsilon retain the original definitions.
///
/// Straight and polygon sources retain their exact region and corner policy.
/// Smooth sources first try the initial endpoint-tangent cubic and its qualified
/// common-handle correction. Open regular sources can then use bounded adaptive
/// subdivision, joining and C1 reconstruction. Every final fitted curve needs a
/// complete positional certificate against its original source. Rounded edits
/// during adaptive shared-basis preparation require a fresh certificate.
///
/// Adaptive fitting requires at least two profiles, source degree at most 16,
/// and unresolved tangent breaks only. Tolerances may exceed endpoint chords;
/// failed whole-span proposals retain bounded subdivision and source proofs.
/// Fully closed smooth sources, mixed open/closed sets and unproved or exhausted
/// fits are rejected. Existing certified closed polygon fits remain available.
/// Matching can change parameter correspondence. Sources are never mutated;
/// positional certificates do not establish topology or Rhino control-net parity.
pub fn try_prepare_refitted_loft_sections(
    curves: &[NurbsCurve],
    fit_tolerance: Real,
) -> Result<Vec<NurbsCurve>, GeometryError> {
    prepare(curves, fit_tolerance, None)
}

/// Refit with an explicit corner angle in radians, in `[0, pi]`. Zero uses
/// one degree; positive angles have a measured 0.002-radian resolution floor.
/// Suppressed corners require the adaptive smooth fitter and may remain unsupported.
pub fn try_prepare_refitted_loft_sections_with_angle(
    curves: &[NurbsCurve],
    fit_tolerance: Real,
    angle: Real,
) -> Result<Vec<NurbsCurve>, GeometryError> {
    prepare(curves, fit_tolerance, Some(angle))
}

// Public multi-profile Refit captures resolve positive angles above 0.002
// radians. Zero selects the one-degree default instead of that minimum.
fn corner_angle(angle: Option<Real>) -> Real {
    let default = std::f64::consts::PI / 180.;
    angle.filter(|a| *a > 0.).map_or(default, |a| a.max(0.002))
}

// Native Refit preserves the source through twice the square-root-epsilon
// threshold, including equality. Keep this policy separate from raw proposals.
fn fitting_disabled(tolerance: Real) -> bool {
    tolerance <= 2. * Real::EPSILON.sqrt()
}

fn validate_angle(angle: Option<Real>) -> Result<(), GeometryError> {
    if angle.is_some_and(|a| !a.is_finite() || !(0. ..=std::f64::consts::PI).contains(&a)) {
        return Err(GeometryError::InvalidContinuityAngle);
    }
    Ok(())
}

fn prepare(
    curves: &[NurbsCurve],
    tolerance: Real,
    angle: Option<Real>,
) -> Result<Vec<NurbsCurve>, GeometryError> {
    let prepared = preparation::propose(curves, tolerance, angle)?;
    if prepared.mode == preparation::Mode::Adaptive {
        prepared.finish(
            curves,
            tolerance,
            angle,
            &mut |c: &NurbsCurve| c.clamped_to_active_domain(),
            &mut |c: &NurbsCurve, d| c.try_change_degree(d, false),
            &mut |c: &NurbsCurve, k, m| c.try_insert_knot(k, m),
        )
    } else {
        Ok(prepared.curves)
    }
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

/// Ordered Refit Loft with an explicit corner angle in radians. Disabled fitting
/// preserves the source definitions but still matches section regions.
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
    preparation::propose(curves, fit_tolerance, angle)?.finish(
        curves,
        fit_tolerance,
        angle,
        clamp,
        elevate,
        refine,
    )
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
