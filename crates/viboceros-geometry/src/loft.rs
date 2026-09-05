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
    validate_count(curves.len(), closed)?;
    let sections = compatible::prepare(curves)?;
    let (degree, knots, controls) = interpolate::loft(&sections, style, closed)?;
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
    surface.try_reparameterized(0.0..=u, 0.0..=v)
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
