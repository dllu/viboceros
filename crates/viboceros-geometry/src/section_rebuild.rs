//! Shared clamped, arc-length section rebuilding for Loft and Sweep.
use crate::{CurveRef, GeometryError, NurbsCurve, Tolerance};

/// Rebuild section copies with a shared point-count and seam policy.
/// Zero/one preserve each source. A closed section also remains unchanged for
/// two points; otherwise it receives one extra control for its repeated seam.
/// Degree is `min(3, point_count - 1)`. Validate every layout before invoking
/// the supplied numerical backend so a resource failure cannot start work.
pub fn try_rebuild_nurbs_sections_with<'a, E: From<GeometryError>>(
    curves: impl IntoIterator<Item = &'a NurbsCurve>,
    point_count: usize,
    maximum_controls: usize,
    limit: GeometryError,
    tolerance: Tolerance,
    rebuild: &mut impl FnMut(&NurbsCurve, usize, usize, Tolerance) -> Result<NurbsCurve, E>,
) -> Result<Vec<NurbsCurve>, E> {
    if point_count > maximum_controls {
        return Err(limit.into());
    }
    let curves = curves.into_iter().collect::<Vec<_>>();
    let counts = curves
        .iter()
        .map(|source| {
            let closed = CurveRef::NurbsCurve(source).is_closed()?;
            let count = if point_count < 2 || (closed && point_count < 3) {
                0
            } else {
                point_count
                    .checked_add(usize::from(closed))
                    .ok_or_else(|| limit.clone())?
            };
            if count > maximum_controls {
                return Err(limit.clone());
            }
            Ok(count)
        })
        .collect::<Result<Vec<_>, GeometryError>>()?;
    curves
        .into_iter()
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
                    context: "section rebuild backend returned an inconsistent layout",
                }
                .into());
            }
            Ok(curve)
        })
        .collect()
}
