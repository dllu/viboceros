//! Point poles are inserted after real profiles share their basis and weights.
use super::*;
use crate::Point3;
#[cfg(test)]
mod tests;

/// Optional poles before and after the ordered real curve profiles.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct LoftEndPoints {
    pub start: Option<Point3>,
    pub end: Option<Point3>,
}

impl LoftEndPoints {
    pub fn is_empty(self) -> bool {
        self.start.is_none() && self.end.is_none()
    }
}

/// Ordered open Loft, optionally tapered to either point. Poles inherit the
/// adjacent prepared profile's weights. At least one curve is required.
pub fn try_loft_nurbs_curves_with_end_points(
    curves: &[NurbsCurve],
    style: LoftStyle,
    ends: LoftEndPoints,
) -> Result<NurbsSurface, GeometryError> {
    try_loft_nurbs_curves_with_end_points_and_operations(
        curves,
        style,
        ends,
        &mut compatible::prepare,
        &mut interpolate::solve_portable,
    )
}

/// Use the existing exact section preparation and interpolation backends;
/// collapsed pole rows do not participate in geometric profile matching.
pub fn try_loft_nurbs_curves_with_end_points_and_operations<E: From<GeometryError>>(
    curves: &[NurbsCurve],
    style: LoftStyle,
    ends: LoftEndPoints,
    prepare: &mut impl FnMut(&[NurbsCurve]) -> Result<Vec<NurbsCurve>, E>,
    solve: &mut impl FnMut(&LoftInterpolation<'_>) -> Result<Vec<Real>, E>,
) -> Result<NurbsSurface, E> {
    validate_end_points(curves.len(), style, ends)?;
    let mut sections = checked_sections(curves, prepare)?;
    let start = ends
        .start
        .map(|point| pole(&sections[0], point))
        .transpose()?;
    let end = ends
        .end
        .map(|point| pole(sections.last().unwrap(), point))
        .transpose()?;
    if let Some(start) = start {
        sections.insert(0, start);
    }
    if let Some(end) = end {
        sections.push(end);
    }
    surface_from_sections(&sections, style, false, solve)
}

pub(crate) fn validate_end_points(
    curve_count: usize,
    style: LoftStyle,
    ends: LoftEndPoints,
) -> Result<(), GeometryError> {
    if curve_count == 0 {
        return Err(GeometryError::InvalidControlNet {
            context: "Loft point ends require at least one curve",
        });
    }
    if style == LoftStyle::Uniform && !ends.is_empty() {
        return Err(GeometryError::InvalidControlNet {
            context: "Uniform Loft does not support point ends",
        });
    }
    let count = curve_count
        .checked_add(usize::from(ends.start.is_some()) + usize::from(ends.end.is_some()))
        .ok_or(GeometryError::LoftResourceLimit {
            context: "section count",
            maximum: MAX_LOFT_SECTIONS,
        })?;
    validate_count(count, false)
}

fn pole(template: &NurbsCurve, point: Point3) -> Result<NurbsCurve, GeometryError> {
    NurbsCurve::try_new_rational(
        template.degree(),
        template
            .control_points()
            .iter()
            .map(|cp| WeightedPoint3::try_new(point, cp.weight()))
            .collect::<Result<_, _>>()?,
        template.knots().to_vec(),
    )
}
