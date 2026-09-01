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
    /// Constructs a frame whose x-axis follows `x_direction` while its z-axis
    /// is the component of `preferred_normal` perpendicular to x. This is the
    /// construction-plane rule Rhino uses for two-point orientation commands.
    pub fn try_from_x_and_normal(
        origin: Point3,
        x_direction: Vector3,
        preferred_normal: Vector3,
        tolerance: Tolerance,
    ) -> Result<Self, GeometryError> {
        let x_axis = x_direction.normalized(tolerance)?;
        let projection = preferred_normal.dot(x_axis.as_vector())?;
        let normal = preferred_normal.to_array();
        let x = x_axis.as_vector().to_array();
        let z_axis = Vector3::try_new(
            (-projection).mul_add(x[0], normal[0]),
            (-projection).mul_add(x[1], normal[1]),
            (-projection).mul_add(x[2], normal[2]),
        )?
        .normalized(tolerance)?;
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

    /// Constructs the deterministic OpenNURBS-style plane whose z-axis is the
    /// supplied normal. The x-axis is chosen from the two largest normal
    /// components, and y completes a right-handed orthonormal frame.
    pub fn try_from_normal(
        origin: Point3,
        normal: Vector3,
        tolerance: Tolerance,
    ) -> Result<Self, GeometryError> {
        let z_axis = normal.normalized(tolerance)?;
        let components = z_axis.as_vector().to_array();
        let [x, y, z] = components.map(f64::abs);
        let (first, second, zero) = if y > x {
            if z > y {
                (2, 1, 0)
            } else if z >= x {
                (1, 2, 0)
            } else {
                (1, 0, 2)
            }
        } else if z > x {
            (2, 0, 1)
        } else if z > y {
            (0, 2, 1)
        } else {
            (0, 1, 2)
        };
        let mut perpendicular = [0.0; 3];
        perpendicular[first] = -components[second];
        perpendicular[second] = components[first];
        perpendicular[zero] = 0.0;
        let x_axis = Vector3::try_from(perpendicular)?.normalized_nonzero()?;
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

    #[test]
    fn normal_constructor_matches_opennurbs_axis_selection() {
        let origin = point(1.0, 2.0, 3.0);
        let world_z = Frame3::try_from_normal(
            origin,
            Vector3::try_new(0.0, 0.0, 1.0).unwrap(),
            Tolerance::DEFAULT,
        )
        .unwrap();
        assert_eq!(world_z.x_axis().as_vector().to_array(), [1.0, 0.0, 0.0]);
        assert_eq!(world_z.y_axis().as_vector().to_array(), [0.0, 1.0, 0.0]);

        let world_y = Frame3::try_from_normal(
            origin,
            Vector3::try_new(0.0, 1.0, 0.0).unwrap(),
            Tolerance::DEFAULT,
        )
        .unwrap();
        assert_eq!(world_y.x_axis().as_vector().to_array(), [0.0, 0.0, 1.0]);
        assert_eq!(world_y.y_axis().as_vector().to_array(), [1.0, 0.0, 0.0]);
        assert_eq!(world_y.z_axis().as_vector().to_array(), [0.0, 1.0, 0.0]);

        let oblique = Frame3::try_from_normal(
            origin,
            Vector3::try_new(2.0, -3.0, 4.0).unwrap(),
            Tolerance::DEFAULT,
        )
        .unwrap();
        for axis in oblique.axes() {
            assert!(Tolerance::DEFAULT.approx_eq(axis.as_vector().length().unwrap(), 1.0));
        }
        assert!(
            Tolerance::DEFAULT.approx_eq(
                oblique
                    .x_axis()
                    .as_vector()
                    .dot(oblique.y_axis().as_vector())
                    .unwrap(),
                0.0
            )
        );
    }

    #[test]
    fn x_and_normal_constructor_projects_the_preferred_normal() {
        let frame = Frame3::try_from_x_and_normal(
            point(1.0, 2.0, 3.0),
            Vector3::try_new(1.0, 1.0, 1.0).unwrap(),
            Vector3::try_new(0.0, 0.0, 1.0).unwrap(),
            Tolerance::DEFAULT,
        )
        .unwrap();
        let inverse_root_three = 1.0 / 3.0_f64.sqrt();
        let inverse_root_two = 1.0 / 2.0_f64.sqrt();
        let inverse_root_six = 1.0 / 6.0_f64.sqrt();
        for (actual, expected) in frame.x_axis().as_vector().to_array().into_iter().zip([
            inverse_root_three,
            inverse_root_three,
            inverse_root_three,
        ]) {
            assert!(Tolerance::DEFAULT.approx_eq(actual, expected));
        }
        for (actual, expected) in frame.y_axis().as_vector().to_array().into_iter().zip([
            -inverse_root_two,
            inverse_root_two,
            0.0,
        ]) {
            assert!(Tolerance::DEFAULT.approx_eq(actual, expected));
        }
        for (actual, expected) in frame.z_axis().as_vector().to_array().into_iter().zip([
            -inverse_root_six,
            -inverse_root_six,
            2.0 * inverse_root_six,
        ]) {
            assert!(Tolerance::DEFAULT.approx_eq(actual, expected));
        }
    }
}
