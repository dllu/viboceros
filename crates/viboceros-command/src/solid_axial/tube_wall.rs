//! The Tube second-wall getter is concentric with the accepted base circle.
use crate::circle_input::CircleDefinition;
use viboceros_geometry::{GeometryError, Point3, Tolerance};

#[derive(Clone, Copy, Debug, PartialEq)]
pub(super) enum TubeWall {
    Radius,
    Thickness,
    Ready(f64),
}

impl TubeWall {
    pub(super) fn pending(self) -> bool {
        !matches!(self, Self::Ready(_))
    }

    pub(super) fn radius(self) -> Option<f64> {
        match self {
            Self::Ready(radius) => Some(radius),
            _ => None,
        }
    }

    pub(super) fn projected_radius(
        circle: CircleDefinition,
        point: Point3,
    ) -> Result<f64, GeometryError> {
        let [x, y] = circle.frame.projected_coordinates_of(point)?;
        Ok(x.hypot(y))
    }

    pub(super) fn check_radius(radius: f64) -> Result<f64, GeometryError> {
        if !radius.is_finite() {
            return Err(GeometryError::NonFinite {
                context: "second tube radius",
            });
        }
        if radius <= Tolerance::NUMERICAL_VALIDATION.absolute() {
            return Err(GeometryError::Degenerate {
                context: "second tube radius",
            });
        }
        Ok(radius)
    }

    pub(super) fn from_thickness(base: f64, thickness: f64) -> Result<f64, GeometryError> {
        Self::check_radius(base + thickness)
    }
}
