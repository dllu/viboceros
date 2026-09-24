//! One-pick scalar distance and angular cursor constraints.

use thiserror::Error;
use viboceros_geometry::{Frame3, GeometryError, Point3, Real};

use crate::{PointConstraintInput, plane};

#[derive(Clone, Copy, Debug, PartialEq)]
enum DistanceLock {
    Fixed(Real),
    Increment(Real),
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PointConstraintState {
    anchor: Point3,
    plane: Frame3,
    distance: Option<DistanceLock>,
    angle_increment_degrees: Option<Real>,
}

#[derive(Clone, Debug, Error, PartialEq)]
pub enum PointConstraintError {
    #[error("distance and angle constraints require a previous point in the active command")]
    MissingAnchor,
    #[error("constrained point needs a direction from the previous point")]
    ZeroDirection,
    #[error("constraint value or resulting point is outside the finite model range")]
    OutOfRange,
    #[error(transparent)]
    Geometry(#[from] GeometryError),
}

impl PointConstraintState {
    pub fn new(anchor: Point3, plane: Frame3) -> Self {
        Self {
            anchor,
            plane,
            distance: None,
            angle_increment_degrees: None,
        }
    }

    pub fn angle_active(self) -> bool {
        self.angle_increment_degrees.is_some()
    }

    pub fn set(
        &mut self,
        input: PointConstraintInput,
        plane: Frame3,
    ) -> Result<(), PointConstraintError> {
        match input {
            PointConstraintInput::Distance(value) => {
                if !value.is_finite() || value == 0.0 {
                    return Err(PointConstraintError::OutOfRange);
                }
                self.distance = Some(if value > 0.0 {
                    DistanceLock::Fixed(value)
                } else {
                    DistanceLock::Increment(-value)
                });
            }
            PointConstraintInput::Angle(value) => {
                if !value.is_finite() || value.abs() > 180.0 {
                    return Err(PointConstraintError::OutOfRange);
                }
                self.plane = plane;
                self.angle_increment_degrees = Some(value.abs());
            }
        }
        Ok(())
    }

    /// Rhino constrains typed point coordinates by distance, while an angle
    /// constraint changes cursor tracking but leaves typed coordinates intact.
    pub fn apply_typed(self, candidate: Point3) -> Result<Point3, PointConstraintError> {
        self.apply_distance(candidate)
    }

    /// Snap a picked cursor to the nearest local angular ray, then its locked
    /// 3D distance from the previously accepted point.
    pub fn apply_cursor(self, candidate: Point3) -> Result<Point3, PointConstraintError> {
        let angled = if let Some(step) = self.angle_increment_degrees {
            plane::ortho_point(
                candidate,
                self.anchor,
                self.plane,
                if step == 0.0 { 180.0 } else { step },
            )
            .ok_or(PointConstraintError::OutOfRange)?
        } else {
            candidate
        };
        self.apply_distance(angled)
    }

    fn apply_distance(self, candidate: Point3) -> Result<Point3, PointConstraintError> {
        let Some(distance) = self.distance else {
            return Ok(candidate);
        };
        if candidate == self.anchor {
            return Err(PointConstraintError::ZeroDirection);
        }
        let radius = match distance {
            DistanceLock::Fixed(value) => value,
            DistanceLock::Increment(step) => {
                let raw = self.anchor.distance_to(candidate)?;
                let quotient = raw / step;
                let radius = if quotient.is_finite() {
                    quotient.round().max(1.0) * step
                } else if step <= raw * Real::EPSILON {
                    raw
                } else {
                    return Err(PointConstraintError::OutOfRange);
                };
                if !radius.is_finite() || radius <= 0.0 {
                    return Err(PointConstraintError::OutOfRange);
                }
                radius
            }
        };
        let direction = self.anchor.direction_to(candidate)?.as_vector().to_array();
        let origin = self.anchor.to_array();
        Ok(Point3::try_from(std::array::from_fn(|axis| {
            direction[axis].mul_add(radius, origin[axis])
        }))?)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use viboceros_geometry::{Tolerance, Vector3};

    fn point(x: Real, y: Real, z: Real) -> Point3 {
        Point3::try_new(x, y, z).unwrap()
    }

    fn plane() -> Frame3 {
        Frame3::try_from_directions(
            point(0.0, 0.0, 0.0),
            Vector3::try_new(1.0, 0.0, 0.0).unwrap(),
            Vector3::try_new(0.0, 1.0, 0.0).unwrap(),
            Tolerance::DEFAULT,
        )
        .unwrap()
    }

    #[test]
    fn fixed_distance_scales_the_three_dimensional_direction() {
        let mut state = PointConstraintState::new(point(0.0, 0.0, 3.0), plane());
        state
            .set(PointConstraintInput::Distance(5.0), plane())
            .unwrap();
        let actual = state.apply_typed(point(6.0, 8.0, 0.0)).unwrap();
        let expected = point(2.873478855663454, 3.8313051408846057, 1.563260572168273);
        assert!(actual.distance_to(expected).unwrap() < 1e-12);
        assert_eq!(
            state.apply_typed(point(0.0, 0.0, 3.0)),
            Err(PointConstraintError::ZeroDirection)
        );
    }

    #[test]
    fn negative_distance_uses_nearest_positive_increment() {
        let mut state = PointConstraintState::new(point(0.0, 0.0, 0.0), plane());
        state
            .set(PointConstraintInput::Distance(-5.0), plane())
            .unwrap();
        for (source, expected) in [(2.0, 5.0), (7.0, 5.0), (7.5, 10.0), (8.0, 10.0)] {
            assert_eq!(
                state.apply_typed(point(source, 0.0, 0.0)).unwrap().x(),
                expected
            );
        }
    }

    #[test]
    fn typed_angles_leave_coordinates_alone_but_cursor_snaps_to_rays() {
        let mut state = PointConstraintState::new(point(0.0, 0.0, 0.0), plane());
        state
            .set(PointConstraintInput::Angle(30.0), plane())
            .unwrap();
        let candidate = point(10.0, 1.0, 0.0);
        assert_eq!(state.apply_typed(candidate), Ok(candidate));
        let snapped = state.apply_cursor(candidate).unwrap();
        assert_eq!(snapped.y(), 0.0);
        assert_eq!(snapped.x(), 10.0);
        state
            .set(PointConstraintInput::Distance(5.0), plane())
            .unwrap();
        assert!(
            state
                .apply_typed(candidate)
                .unwrap()
                .distance_to(point(4.975185951049946, 0.49751859510499463, 0.0))
                .unwrap()
                < 1e-14
        );
        assert_eq!(state.apply_cursor(candidate).unwrap(), point(5.0, 0.0, 0.0));
    }
}
