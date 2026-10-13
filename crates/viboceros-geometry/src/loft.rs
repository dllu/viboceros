//! Surface construction from ordered, structurally compatible section curves.

use crate::{GeometryError, NurbsCurve, NurbsSurface, Real, WeightedPoint3};

mod compatible;
mod interpolate;
#[cfg(test)]
mod tests;

/// Maximum number of supplied loft sections.
pub const MAX_LOFT_SECTIONS: usize = 256;
/// Maximum control count in the shared section basis.
pub const MAX_LOFT_SECTION_CONTROLS: usize = 512;

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum LoftStyle {
    #[default]
    Normal,
    Loose,
    Tight,
    Straight,
    Uniform,
}

/// Shared homogeneous interpolation targets and section parameter policy.
/// Targets use row-major order; open systems append two control-handle rows.
pub type LoftInterpolation<'a> = crate::CubicInterpolationSystem<'a>;

/// Lofts ordered profiles without reordering, reversing or relocating seams.
/// U runs through the sections; V follows each section's normalized parameter.
/// Matching follows Rhino's common-basis endpoint-weight policy. With unequal
/// endpoint-weight ratios, later sections' geometric images may change; this
/// is not an unconditional exact interpolation of the original input curves.
pub fn try_loft_nurbs_curves(
    curves: &[NurbsCurve],
    style: LoftStyle,
    closed: bool,
) -> Result<NurbsSurface, GeometryError> {
    try_loft_nurbs_curves_with_preparation(curves, style, closed, &mut compatible::prepare)
}

/// Preserve section/end-handle interpolation and size policy while delegating
/// exact common-basis preparation to an external geometry backend.
pub fn try_loft_nurbs_curves_with_preparation<E: From<GeometryError>>(
    curves: &[NurbsCurve],
    style: LoftStyle,
    closed: bool,
    prepare: &mut impl FnMut(&[NurbsCurve]) -> Result<Vec<NurbsCurve>, E>,
) -> Result<NurbsSurface, E> {
    try_loft_nurbs_curves_with_operations(
        curves,
        style,
        closed,
        prepare,
        &mut interpolate::solve_portable,
    )
}

/// Preserve Loft policy while delegating common-basis edits and the homogeneous
/// interpolation solve to the caller's backend. Loose/Straight bypass solving.
pub fn try_loft_nurbs_curves_with_operations<E: From<GeometryError>>(
    curves: &[NurbsCurve],
    style: LoftStyle,
    closed: bool,
    prepare: &mut impl FnMut(&[NurbsCurve]) -> Result<Vec<NurbsCurve>, E>,
    solve: &mut impl FnMut(&LoftInterpolation<'_>) -> Result<Vec<Real>, E>,
) -> Result<NurbsSurface, E> {
    validate_count(curves.len(), closed)?;
    let sections = prepare(curves)?;
    if sections.len() != curves.len()
        || sections.iter().any(|s| {
            s.degree() != sections[0].degree()
                || s.knots() != sections[0].knots()
                || s.control_points().len() > MAX_LOFT_SECTION_CONTROLS
        })
    {
        return Err(GeometryError::InvalidControlNet {
            context: "loft preparation produced inconsistent section layouts",
        }
        .into());
    }
    let (degree, knots, controls) = interpolate::loft_with_solver(&sections, style, closed, solve)?;
    let count_v = sections[0].control_points().len();
    let count_u = controls.len() / count_v;
    let surface = NurbsSurface::try_new_rational(
        degree,
        sections[0].degree(),
        count_u,
        count_v,
        controls,
        knots,
        sections[0].knots().to_vec(),
    )?;
    let [u, v] = surface.estimated_size()?;
    Ok(surface.try_reparameterized(0.0..=u, 0.0..=v)?)
}

/// Shared Loft weight and basis policy with exact caller-supplied operations.
pub fn prepare_loft_sections_with_operations<E: From<GeometryError>>(
    curves: &[NurbsCurve],
    clamp: &mut impl FnMut(&NurbsCurve) -> Result<NurbsCurve, E>,
    elevate: &mut impl FnMut(&NurbsCurve, usize) -> Result<NurbsCurve, E>,
    refine: &mut impl FnMut(&NurbsCurve, Real, usize) -> Result<NurbsCurve, E>,
) -> Result<Vec<NurbsCurve>, E> {
    compatible::prepare_with_operations(curves, clamp, elevate, refine)
}

pub(crate) fn validate_count(count: usize, closed: bool) -> Result<(), GeometryError> {
    if count < if closed { 3 } else { 2 } {
        return Err(GeometryError::InsufficientLoftSections {
            closed,
            actual: count,
        });
    }
    if count > MAX_LOFT_SECTIONS {
        return Err(GeometryError::LoftResourceLimit {
            context: "section count",
            maximum: MAX_LOFT_SECTIONS,
        });
    }
    Ok(())
}
