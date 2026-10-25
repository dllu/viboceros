//! Loft's clamped section rebuild, separate from periodic Curve.Rebuild.
use super::*;
use crate::{CurveRef, Tolerance, try_rebuild_open_curve};

/// Rebuild the ordered profiles before common-basis preparation and lofting.
/// The original profiles are retained. Zero/one leave rebuilding off; a closed
/// profile also retains its basis for a two-point request. Other closed profiles
/// use one extra control for the repeated seam. Degree is min(3, count - 1).
pub fn try_loft_nurbs_curves_rebuilt(
    curves: &[NurbsCurve],
    style: LoftStyle,
    closed: bool,
    ends: LoftEndPoints,
    point_count: usize,
    tolerance: Tolerance,
) -> Result<NurbsSurface, GeometryError> {
    let sections = try_prepare_rebuilt_loft_sections_with(
        curves,
        style,
        closed,
        ends,
        point_count,
        tolerance,
        &mut |c, n, d, t| try_rebuild_open_curve(CurveRef::NurbsCurve(c), n, d, t),
    )?;
    if ends.is_empty() {
        try_loft_nurbs_curves(&sections, style, closed)
    } else {
        try_loft_nurbs_curves_with_end_points(&sections, style, ends)
    }
}

/// Keep the section count, degree and seam policy while delegating each
/// clamped arc-length rebuild to the selected math backend.
pub fn try_prepare_rebuilt_loft_sections_with<E: From<GeometryError>>(
    curves: &[NurbsCurve],
    style: LoftStyle,
    closed: bool,
    ends: LoftEndPoints,
    point_count: usize,
    tolerance: Tolerance,
    rebuild: &mut impl FnMut(&NurbsCurve, usize, usize, Tolerance) -> Result<NurbsCurve, E>,
) -> Result<Vec<NurbsCurve>, E> {
    if closed && !ends.is_empty() {
        return Err(GeometryError::InvalidControlNet {
            context: "closed Loft cannot have point ends",
        }
        .into());
    }
    if ends.is_empty() {
        validate_count(curves.len(), closed)?;
    } else {
        validate_end_points(curves.len(), style, ends)?;
    }
    if point_count > MAX_LOFT_SECTION_CONTROLS {
        return Err(GeometryError::LoftResourceLimit {
            context: "rebuilt section control count",
            maximum: MAX_LOFT_SECTION_CONTROLS,
        }
        .into());
    }
    let counts = curves
        .iter()
        .map(|c| {
            let is_closed = CurveRef::NurbsCurve(c).is_closed()?;
            let n = if point_count < 2 || (is_closed && point_count < 3) {
                0
            } else {
                point_count + usize::from(is_closed)
            };
            if n > MAX_LOFT_SECTION_CONTROLS {
                return Err(GeometryError::LoftResourceLimit {
                    context: "rebuilt section control count including seam",
                    maximum: MAX_LOFT_SECTION_CONTROLS,
                });
            }
            Ok(n)
        })
        .collect::<Result<Vec<_>, GeometryError>>()?;
    curves
        .iter()
        .zip(counts)
        .map(|(source, count)| {
            if count == 0 {
                return Ok(source.clone());
            }
            let degree = (point_count - 1).min(3);
            let curve = rebuild(source, count, degree, tolerance)?;
            if curve.degree() != degree
                || curve.control_points().len() != count
                || curve.control_points().iter().any(|p| p.weight() != 1.)
            {
                return Err(GeometryError::InvalidControlNet {
                    context: "Loft rebuild backend returned an inconsistent layout",
                }
                .into());
            }
            Ok(curve)
        })
        .collect()
}

#[cfg(test)]
mod tests;
