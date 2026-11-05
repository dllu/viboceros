//! Continuity queries followed by the existing exact bounded face partition.
use super::*;
use crate::CurveContinuity;
impl Brep {
    /// Split continuous G1/G2 knot lines with the existing exact-locus
    /// certificates and shared budget. Candidates must already have full
    /// degree multiplicity; refinement belongs to fresh geometry construction.
    pub fn try_split_faces_at_discontinuities(
        &self,
        continuity: CurveContinuity,
        angle: Real,
        tolerance: Tolerance,
    ) -> Result<Option<Self>, GeometryError> {
        if !angle.is_finite() || !(0. ..=std::f64::consts::PI).contains(&angle) {
            return Err(GeometryError::InvalidContinuityAngle);
        }
        let mut budget = Budget(MAX_WORK);
        let mut result = None;
        let mut index = 0;
        loop {
            let current = result.as_ref().unwrap_or(self);
            let Some(face) = current.faces.get(index) else {
                break;
            };
            charge_kink_search(&face.surface, &mut budget)?;
            let breaks = face.surface.discontinuity_parameters(continuity, angle)?;
            let mut next = None;
            for (axis, cuts) in breaks.into_iter().enumerate() {
                for parameter in cuts {
                    let direction = if axis == 0 {
                        SurfaceKnotDirection::U
                    } else {
                        SurfaceKnotDirection::V
                    };
                    if let Some(partition) = current.split_face_at_knot_with_budget(
                        index,
                        direction,
                        parameter,
                        tolerance,
                        &mut budget,
                    )? {
                        next = Some(partition);
                        break;
                    }
                }
                if next.is_some() {
                    break;
                }
            }
            if let Some(next) = next {
                result = Some(next);
            } else {
                index += 1;
            }
        }
        Ok(result)
    }
}
