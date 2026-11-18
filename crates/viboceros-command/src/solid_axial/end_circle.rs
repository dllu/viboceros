//! Projected, signed-axis end-circle sizing for a truncated cone.
use super::Definition;
use crate::circle_input::CircleSizeMode;
use viboceros_geometry::{Frame3, GeometryError, Point3};

#[derive(Clone, Copy, Debug, PartialEq)]
pub(super) struct EndCircle {
    axis: Definition,
    pub frame: Frame3,
}

impl EndCircle {
    pub fn new(axis: Definition) -> Result<Self, GeometryError> {
        let center = axis.frame.point_at([0., 0., axis.height])?;
        Ok(Self {
            axis,
            frame: axis.frame.with_origin(center),
        })
    }

    pub fn projected_size(self, point: Point3) -> Result<f64, GeometryError> {
        let [x, y] = self.frame.projected_coordinates_of(point)?;
        Ok(x.hypot(y))
    }

    pub fn size(self, mode: CircleSizeMode, value: f64) -> Result<Definition, GeometryError> {
        if !value.is_finite() {
            return Err(GeometryError::NonFinite {
                context: "truncated cone top size",
            });
        }
        Ok(Definition {
            end_radius: mode.radius(value.max(0.)),
            ..self.axis
        })
    }
}
