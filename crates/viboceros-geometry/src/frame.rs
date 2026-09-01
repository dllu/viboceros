use crate::{GeometryError, Point3, Tolerance, UnitVector3, Vector3};

/// A finite right-handed orthonormal coordinate frame.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Frame3 {
    origin: Point3,
    x_axis: UnitVector3,
    y_axis: UnitVector3,
    z_axis: UnitVector3,
}

impl Frame3 {
    /// Constructs a frame from an origin, a point on its positive x-axis, and
    /// a point in its positive xy half-plane.
    pub fn try_from_points(
        origin: Point3,
        point_on_x_axis: Point3,
        point_in_xy_plane: Point3,
        tolerance: Tolerance,
    ) -> Result<Self, GeometryError> {
        Self::try_from_directions(
            origin,
            origin.vector_to(point_on_x_axis)?,
            origin.vector_to(point_in_xy_plane)?,
            tolerance,
        )
    }

    /// Constructs a frame from an x direction and a second direction whose
    /// component perpendicular to x determines positive y.
    pub fn try_from_directions(
        origin: Point3,
        x_direction: Vector3,
        xy_direction: Vector3,
        tolerance: Tolerance,
    ) -> Result<Self, GeometryError> {
        let x_axis = x_direction.normalized(tolerance)?;
        let xy_axis = xy_direction.normalized(tolerance)?;
        let cross = x_axis.as_vector().cross(xy_axis.as_vector())?;
        if cross.length()? <= tolerance.angular() {
            return Err(GeometryError::Degenerate {
                context: "coordinate frame",
            });
        }
        let z_axis = cross.normalized_nonzero()?;
        let y_axis = z_axis
            .as_vector()
            .cross(x_axis.as_vector())?
            .normalized_nonzero()?;
        Ok(Self {
            origin,
            x_axis,
            y_axis,
            z_axis,
        })
    }

    #[inline]
    pub const fn origin(self) -> Point3 {
        self.origin
    }

    #[inline]
    pub const fn x_axis(self) -> UnitVector3 {
        self.x_axis
    }

    #[inline]
    pub const fn y_axis(self) -> UnitVector3 {
        self.y_axis
    }

    #[inline]
    pub const fn z_axis(self) -> UnitVector3 {
        self.z_axis
    }

    #[inline]
    pub const fn axes(self) -> [UnitVector3; 3] {
        [self.x_axis, self.y_axis, self.z_axis]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn point(x: f64, y: f64, z: f64) -> Point3 {
        Point3::try_new(x, y, z).unwrap()
    }

    #[test]
    fn constructs_a_right_handed_frame_by_projecting_the_plane_point() {
        let frame = Frame3::try_from_points(
            point(1.0, 2.0, 3.0),
            point(3.0, 2.0, 3.0),
            point(2.0, 5.0, 3.0),
            Tolerance::DEFAULT,
        )
        .unwrap();
        assert_eq!(frame.origin(), point(1.0, 2.0, 3.0));
        assert_eq!(frame.x_axis().as_vector().to_array(), [1.0, 0.0, 0.0]);
        assert_eq!(frame.y_axis().as_vector().to_array(), [0.0, 1.0, 0.0]);
        assert_eq!(frame.z_axis().as_vector().to_array(), [0.0, 0.0, 1.0]);
    }

    #[test]
    fn rejects_short_and_angularly_collinear_frame_directions() {
        assert!(
            Frame3::try_from_points(
                point(0.0, 0.0, 0.0),
                point(1.0e-12, 0.0, 0.0),
                point(0.0, 1.0, 0.0),
                Tolerance::DEFAULT,
            )
            .is_err()
        );
        assert!(
            Frame3::try_from_points(
                point(0.0, 0.0, 0.0),
                point(1.0, 0.0, 0.0),
                point(2.0, 1.0e-12, 0.0),
                Tolerance::DEFAULT,
            )
            .is_err()
        );
    }
}
