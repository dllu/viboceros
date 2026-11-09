//! Planar offsets interpolating original Greville edit points.
use super::*;
use crate::{CurveEditPointSolver, ParameterSide, UnitVector3};

#[cfg(test)]
mod tests;

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
        self.loose_planar_offset(distance, normal, tolerance, solver, false, false)
    }

    /// Offset original Greville positions into a polynomial curve in the same
    /// basis. This is the public Offset/Ribbon Loose policy, distinct from
    /// Inset's weight-preserving edit. It does not certify constant distance
    /// between stations. Periodic input uses the shared cyclic edit-point solve.
    pub fn try_loose_polynomial_offset(
        &self,
        distance: Real,
        normal: UnitVector3,
        tolerance: Tolerance,
        solver: Option<CurveEditPointSolver>,
    ) -> Result<Self, GeometryError> {
        self.loose_planar_offset(distance, normal, tolerance, solver, true, false)
    }

    /// Offset Greville positions perpendicular to a fixed plane normal while
    /// retaining their original heights. The source need not be planar. This
    /// is Ribbon's Loose/InCPlane locus; it retains the polynomial fit policy.
    pub fn try_loose_polynomial_offset_in_plane(
        &self,
        distance: Real,
        normal: UnitVector3,
        tolerance: Tolerance,
        solver: Option<CurveEditPointSolver>,
    ) -> Result<Self, GeometryError> {
        self.loose_planar_offset(distance, normal, tolerance, solver, true, true)
    }

    #[allow(clippy::too_many_arguments)]
    fn loose_planar_offset(
        &self,
        distance: Real,
        normal: UnitVector3,
        tolerance: Tolerance,
        solver: Option<CurveEditPointSolver>,
        polynomial: bool,
        fixed_normal: bool,
    ) -> Result<Self, GeometryError> {
        if !distance.is_finite() || distance == 0. {
            return Err(GeometryError::InvalidCurveOffsetDistance);
        }
        self.tight_bounds(tolerance)?;
        let normal = if fixed_normal {
            normal
        } else {
            crate::curve_offset::nurbs_offset_plane(self, normal, tolerance)?
        };
        let source = if self.is_periodic() {
            self.clone()
        } else {
            self.clamped_to_active_domain()?
        };
        let points = source
            .edit_points()?
            .iter()
            .map(|station| {
                let side = if polynomial {
                    ParameterSide::Right
                } else {
                    station.side
                };
                let point = if polynomial {
                    source.evaluate_on_side(station.parameter, side)?
                } else {
                    station.point
                };
                let tangent = source.tangent_at_on_side(station.parameter, side)?;
                point.translated(
                    normal
                        .as_vector()
                        .cross(tangent.as_vector())?
                        .normalized_nonzero()?
                        .as_vector()
                        .scaled(distance)?,
                )
            })
            .collect::<Result<Vec<_>, GeometryError>>()?;
        let basis = if polynomial {
            Self::try_new(
                source.degree(),
                source.control_points().iter().map(|p| p.point()).collect(),
                source.knots().to_vec(),
            )?
        } else {
            source
        };
        let result = if let Some(mut solver) = solver {
            basis.try_with_edit_points_with_solver(&points, tolerance, &mut solver)?
        } else {
            basis.try_with_edit_points(&points, tolerance)?
        };
        result.clamped_to_active_domain()
    }
}
