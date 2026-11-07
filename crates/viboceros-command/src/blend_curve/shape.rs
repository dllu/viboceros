//! Higher controls retain dimensionless tangent gaps as other handles move.
use super::*;
impl Input {
    pub fn secondary_control_adjustment(&self) -> bool {
        !self.angle_editing
            && self
                .active
                .and_then(Handle::shape)
                .is_some_and(|(_, k)| k >= 2)
    }
    /// A secondary numeric entry uses the second control's tangential coordinate
    /// as its origin, including when the third/fourth control is being edited.
    pub fn set_shape_distance(&mut self, distance: Real) -> Result<(), CommandError> {
        if !self.secondary_control_adjustment() || !distance.is_finite() || distance <= 0. {
            return Err(CommandError::Usage(
                "Select a higher blend control and enter a positive distance",
            ));
        }
        let (i, _) = self
            .active
            .and_then(Handle::shape)
            .ok_or(CommandError::Usage(USAGE))?;
        let c = self.shape_constraint().ok_or(CommandError::Usage(USAGE))?;
        let points = self
            .preview
            .as_ref()
            .ok_or(CommandError::Usage(USAGE))?
            .control_points();
        let second = points[if i == 0 { 2 } else { points.len() - 3 }].point();
        let offset = self
            .shape_origin
            .ok_or(CommandError::Usage(USAGE))?
            .vector_to(second)?
            .dot(c.direction.unwrap().as_vector())?;
        self.shape_distance = Some((distance, offset));
        Ok(())
    }
    pub fn shape_constraint(&self) -> Option<crate::translation::DestinationConstraint> {
        let (i, k) = self.active?.shape()?;
        let guide = self.endpoints[i].curve()?;
        let mut direction = guide.direction(i).ok()?;
        if i == 1 {
            direction = direction.opposite();
        }
        Some(crate::translation::DestinationConstraint {
            anchor: if k == 1 {
                guide.point().ok()?
            } else {
                let p = self.shape_origin?;
                if let Some((_, offset)) = self.shape_distance {
                    p.translated(direction.as_vector().scaled(offset).ok()?)
                        .ok()?
                } else {
                    p
                }
            },
            direction: Some(direction),
            distance: None,
        })
    }
    /// Primary lengths are absolute; higher controls use displacement from the
    /// selected control along the outward tangent. Later gap ratios stay fixed.
    pub fn adjust_shape_length(&mut self, mut length: Real) -> Result<(), CommandError> {
        let (i, k) = self
            .active
            .and_then(Handle::shape)
            .ok_or(CommandError::Usage(USAGE))?;
        if !length.is_finite() || (k == 1 && length <= 0.) {
            return Err(CommandError::Usage(
                "Expected a finite blend control displacement or positive primary length",
            ));
        }
        let mut next = self.clone();
        if k == 1 {
            next.handles[i] = Some(length);
        } else {
            if let Some((distance, offset)) = self.shape_distance {
                if length == 0. {
                    return Err(CommandError::Usage(
                        "Pick a direction away from the selected control",
                    ));
                }
                length = offset + distance.copysign(length);
            }
            next.freeze_handles();
            let h = next.handles[i].ok_or(CommandError::Usage(USAGE))?;
            let ratio = self.spacing[i][usize::from(k - 2)] + length / h;
            if !ratio.is_finite() {
                return Err(CommandError::Usage(
                    "Blend control displacement is too large",
                ));
            }
            next.spacing[i][usize::from(k - 2)] = ratio;
        }
        if self.paired_handle().is_some() {
            next.handles[1 - i] = next.handles[i];
            next.bulges[1 - i] = next.bulges[i];
            next.spacing[1 - i] = next.spacing[i];
        }
        next.rebuild()?;
        *self = next;
        Ok(())
    }
}
