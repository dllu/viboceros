//! Boundary admission and ordering for EdgeSrf.

use crate::{GeometryError, NurbsCurve, Real, Tolerance};

mod arrange;
#[cfg(test)]
mod tests;

const MAX_EDGE_CONTROLS: usize = 512;

/// Validate and orient the selected EdgeSrf boundaries without constructing a
/// surface.
pub fn arrange_edge_surface_curves(
    curves: &[NurbsCurve],
    tolerance: Tolerance,
) -> Result<Vec<NurbsCurve>, GeometryError> {
    if !(2..=4).contains(&curves.len()) {
        return Err(GeometryError::InvalidEdgeSurfaceBoundaries);
    }
    for curve in curves {
        let knots = curve.interior_knot_groups();
        let count = curve.degree() + 1 + knots.iter().map(|(_, count)| count).sum::<usize>();
        if count > MAX_EDGE_CONTROLS {
            return Err(GeometryError::EdgeSurfaceResourceLimit {
                maximum: MAX_EDGE_CONTROLS,
            });
        }
        if curve.is_closed()? || knots.iter().any(|(_, m)| *m > curve.degree()) {
            return Err(GeometryError::InvalidEdgeSurfaceBoundaries);
        }
    }
    arrange::curves(curves, tolerance)
}
