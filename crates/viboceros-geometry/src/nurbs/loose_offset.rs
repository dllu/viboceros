//! Planar offsets interpolating original Greville edit points.
use super::*;
use crate::{CurveEditPointSolver, UnitVector3};

impl NurbsCurve {
    /// Offsets original Greville edit points, retaining degree and rational
    /// weights. Periodic inputs interpolate cyclically before clamping the
    /// result. Uses the edit-point API's bounded degree/control admission.
    /// This is a loose approximation, not a distance-tolerance fit; callers
    /// editing topology must validate the resulting region separately.
    pub fn try_loose_planar_offset(
        &self,
        distance: Real,
        normal: UnitVector3,
        tolerance: Tolerance,
        solver: Option<CurveEditPointSolver>,
    ) -> Result<Self, GeometryError> {
        if !distance.is_finite() || distance == 0. {
            return Err(GeometryError::InvalidCurveOffsetDistance);
        }
        self.tight_bounds(tolerance)?;
        let normal = crate::curve_offset::nurbs_offset_plane(self, normal, tolerance)?;
        let source = if self.is_periodic() {
            self.clone()
        } else {
            self.clamped_to_active_domain()?
        };
        let points = source
            .edit_points()?
            .iter()
            .map(|station| {
                let tangent = source.tangent_at_on_side(station.parameter, station.side)?;
                station.point.translated(
                    normal
                        .as_vector()
                        .cross(tangent.as_vector())?
                        .normalized_nonzero()?
                        .as_vector()
                        .scaled(distance)?,
                )
            })
            .collect::<Result<Vec<_>, GeometryError>>()?;
        let result = if let Some(mut solver) = solver {
            source.try_with_edit_points_with_solver(&points, tolerance, &mut solver)?
        } else {
            source.try_with_edit_points(&points, tolerance)?
        };
        result.clamped_to_active_domain()
    }
}
