//! The final TruncatedPyramid getter sizes the top along the defining base ray.
use super::{Base, CommandError, Definition, End};
use crate::translation::DestinationConstraint;
use viboceros_geometry::{Frame3, GeometryError, Point3, Tolerance};

#[derive(Clone, Copy, Debug, PartialEq)]
pub(super) struct Top {
    pub base: Base,
    frame: Frame3,
}

impl Top {
    pub fn new(base: Base, center: Point3, tolerance: Tolerance) -> Result<Self, GeometryError> {
        Ok(Self {
            base,
            frame: base.top_size_frame(center, tolerance)?,
        })
    }

    pub fn center(self) -> Point3 {
        self.frame.origin()
    }

    pub fn constraint(self) -> DestinationConstraint {
        DestinationConstraint {
            anchor: self.center(),
            direction: Some(self.frame.x_axis()),
            distance: None,
        }
    }

    pub fn picked_size(self, point: Point3) -> Result<f64, GeometryError> {
        Ok(self.frame.projected_coordinates_of(point)?[0].abs())
    }

    pub fn definition(self, size: f64, solid: bool) -> Result<Definition, CommandError> {
        if size <= 0.0 {
            return Err(CommandError::OperationDeclined);
        }
        let scale = size / self.base.top_reference_size();
        if !scale.is_finite() {
            return Err(GeometryError::NonFinite {
                context: "truncated pyramid top scale",
            }
            .into());
        }
        Ok(Definition {
            base: self.base,
            end: End::Top(self.base.scaled_at(self.center(), scale)),
            solid,
        })
    }
}
