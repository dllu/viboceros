//! Endpoint blends with independent position, tangent, or curvature constraints.

use crate::{
    Curve3, EndpointInterpolation, GeometryError, NurbsCurve, ParameterSide, Point3, Real,
    Tolerance, UnitVector3, Vector3, curve_pair_support::selected_end,
};

/// Continuity imposed at one end of a blend.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CurveBlendContinuity {
    Position,
    Tangency,
    Curvature,
}

/// Endpoint continuity and handle lengths for a curve blend.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct CurveBlendOptions {
    pub continuity: [CurveBlendContinuity; 2],
    pub handles: [Option<Real>; 2],
    /// Multipliers for the default handle lengths. An explicit handle length
    /// takes precedence at that end.
    pub bulges: [Real; 2],
    /// Requests endpoint-specific defaults even for equal continuity at the
    /// first curve's end and second curve's start. Other picks or mixed
    /// continuity always use endpoint-specific defaults.
    pub endpoint_specific: bool,
}

impl Default for CurveBlendOptions {
    fn default() -> Self {
        Self {
            continuity: [CurveBlendContinuity::Tangency; 2],
            handles: [None, None],
            bulges: [1.0, 1.0],
            endpoint_specific: false,
        }
    }
}

/// Creates the minimum-degree, single-span nonrational blend connecting
/// selected open-curve ends, from a line for G0/G0 to a quintic for G2/G2.
/// The handle lengths are model-space distances and can be adjusted independently.
pub fn try_blend_curve(
    first: &Curve3,
    first_pick: Point3,
    second: &Curve3,
    second_pick: Point3,
    options: CurveBlendOptions,
    tolerance: Tolerance,
) -> Result<NurbsCurve, GeometryError> {
    try_blend_curve_with_interpolation(
        first,
        first_pick,
        second,
        second_pick,
        options,
        tolerance,
        &mut crate::try_interpolate_endpoints,
    )
}

/// Preserve Blend's endpoint selection, handle policy and domain while
/// delegating its exact minimum-degree endpoint interpolation to a backend.
pub fn try_blend_curve_with_interpolation<E: From<GeometryError>>(
    first: &Curve3,
    first_pick: Point3,
    second: &Curve3,
    second_pick: Point3,
    options: CurveBlendOptions,
    tolerance: Tolerance,
    interpolate: &mut impl FnMut(&EndpointInterpolation<'_>) -> Result<Vec<[Real; 4]>, E>,
) -> Result<NurbsCurve, E> {
    let first_end = selected_end(first, first_pick, tolerance)?;
    let second_end = selected_end(second, second_pick, tolerance)?;
    let endpoint_specific = options.endpoint_specific
        || options.continuity[0] != options.continuity[1]
        || !first_end
        || second_end;
    let mut jets = Vec::with_capacity(2);
    for (side, (curve, at_end)) in [(first, first_end), (second, second_end)]
        .into_iter()
        .enumerate()
    {
        let tangent = match endpoint_tangent(curve, at_end) {
            Ok(t) => Some(if (side == 0) == at_end {
                t
            } else {
                t.opposite()
            }),
            Err(_) if options.continuity[side] == CurveBlendContinuity::Position => None,
            Err(e) => return Err(e.into()),
        };
        let curvature = if options.continuity[side] == CurveBlendContinuity::Curvature {
            endpoint_curvature(curve, at_end)?
        } else {
            Vector3::try_new(0., 0., 0.)?
        };
        jets.push(CurveBlendJet::new(
            endpoint_point(curve, at_end)?,
            tangent,
            curvature,
        ));
    }
    try_blend_curve_from_jets_with_interpolation(
        [jets[0], jets[1]],
        CurveBlendOptions {
            endpoint_specific,
            ..options
        },
        tolerance,
        interpolate,
    )
}

fn continuity_control_count(continuity: CurveBlendContinuity) -> usize {
    match continuity {
        CurveBlendContinuity::Position => 1,
        CurveBlendContinuity::Tangency => 2,
        CurveBlendContinuity::Curvature => 3,
    }
}

fn endpoint_curvature(curve: &Curve3, at_end: bool) -> Result<Vector3, GeometryError> {
    let reference = curve.as_ref();
    let parameter = if at_end {
        *reference.domain().end()
    } else {
        *reference.domain().start()
    };
    reference.curvature_vector(parameter)
}

fn endpoint_point(curve: &Curve3, at_end: bool) -> Result<Point3, GeometryError> {
    let reference = curve.as_ref();
    let parameter = if at_end {
        *reference.domain().end()
    } else {
        *reference.domain().start()
    };
    reference.evaluate(parameter)
}

fn endpoint_tangent(curve: &Curve3, at_end: bool) -> Result<UnitVector3, GeometryError> {
    let reference = curve.as_ref();
    let parameter = if at_end {
        *reference.domain().end()
    } else {
        *reference.domain().start()
    };
    let side = if at_end {
        ParameterSide::Left
    } else {
        ParameterSide::Right
    };
    Ok(reference
        .evaluate_with_tangent_on_side(parameter, side)?
        .tangent())
}

mod jets;
pub use jets::{
    CurveBlendJet, try_blend_curve_from_jets, try_blend_curve_from_jets_with_interpolation,
};
#[cfg(test)]
mod tests;
