use faer::{Mat, prelude::*};
use nalgebra::Matrix3;

use crate::{GeometryError, Point3, Real, Tolerance, UnitVector3, Vector3};

/// An infinite plane represented by a finite origin and unit normal.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Plane {
    origin: Point3,
    normal: UnitVector3,
}

impl Plane {
    pub const fn new(origin: Point3, normal: UnitVector3) -> Self {
        Self { origin, normal }
    }

    #[inline]
    pub const fn origin(self) -> Point3 {
        self.origin
    }

    #[inline]
    pub const fn normal(self) -> UnitVector3 {
        self.normal
    }

    pub fn signed_distance_to(self, point: Point3) -> Result<Real, GeometryError> {
        let distance = self
            .normal
            .as_vector()
            .dot_point_difference(point, self.origin);
        crate::require_finite([distance], "plane signed distance")?;
        Ok(distance)
    }

    fn equation_constant(self) -> Result<Real, GeometryError> {
        let origin = self.origin;
        Vector3::try_new(origin.x(), origin.y(), origin.z())?.dot(self.normal.as_vector())
    }
}

/// Intersects three planes using a fully-pivoted faer LU solve.
///
/// nalgebra's fixed-size matrix is used for the scale-independent determinant
/// predicate; faer handles the numerical solve. Plane normals are unit length,
/// so the determinant can be compared directly with the angular tolerance.
pub fn intersect_three_planes(
    planes: [Plane; 3],
    tolerance: Tolerance,
) -> Result<Point3, GeometryError> {
    let rows = planes.map(|plane| plane.normal().as_vector().to_array());
    let determinant = Matrix3::from_row_slice(&[
        rows[0][0], rows[0][1], rows[0][2], rows[1][0], rows[1][1], rows[1][2], rows[2][0],
        rows[2][1], rows[2][2],
    ])
    .determinant();

    if !determinant.is_finite() || determinant.abs() <= tolerance.angular() {
        return Err(GeometryError::SingularSystem);
    }

    let constants = [
        planes[0].equation_constant()?,
        planes[1].equation_constant()?,
        planes[2].equation_constant()?,
    ];
    let matrix = Mat::from_fn(3, 3, |row, column| rows[row][column]);
    let rhs = Mat::from_fn(3, 1, |row, _| constants[row]);
    let solution = matrix.full_piv_lu().solve(&rhs);
    Point3::try_new(solution[(0, 0)], solution[(1, 0)], solution[(2, 0)])
}

#[cfg(test)]
mod tests {
    use super::*;

    fn axis_plane(origin: Point3, normal: [Real; 3]) -> Plane {
        Plane::new(
            origin,
            UnitVector3::try_new(normal[0], normal[1], normal[2], Tolerance::DEFAULT).unwrap(),
        )
    }

    #[test]
    fn signed_distance_survives_unrepresentable_tangential_displacements() {
        let huge = 2_f64.powi(1023);
        for axis in 0..3 {
            for sign in [-1., 1.] {
                let mut origin = [-huge; 3];
                let mut target = [huge; 3];
                let mut normal = [0.; 3];
                origin[axis] = 2.;
                target[axis] = 5.;
                normal[axis] = sign;
                let make = |v: [Real; 3]| Point3::try_new(v[0], v[1], v[2]).unwrap();
                let plane = axis_plane(make(origin), normal);
                assert_eq!(plane.signed_distance_to(make(target)).unwrap(), sign * 3.);
                let reverse = axis_plane(make(target), normal);
                assert_eq!(
                    reverse.signed_distance_to(make(origin)).unwrap(),
                    -sign * 3.
                );
                origin[axis] = -huge;
                target[axis] = huge;
                assert!(
                    axis_plane(make(origin), normal)
                        .signed_distance_to(make(target))
                        .is_err()
                );
            }
        }
        // Both nonzero displacement components overflow, but the exact
        // six-product sum cancels before conversion to binary64.
        let origin = Point3::try_new(-huge, huge, 0.).unwrap();
        let target = Point3::try_new(huge, -huge, 0.).unwrap();
        assert_eq!(
            axis_plane(origin, [1., 1., 0.])
                .signed_distance_to(target)
                .unwrap(),
            0.
        );
    }

    #[test]
    fn intersects_orthogonal_planes() {
        let intersection = intersect_three_planes(
            [
                axis_plane(Point3::try_new(2.0, 0.0, 0.0).unwrap(), [1.0, 0.0, 0.0]),
                axis_plane(Point3::try_new(0.0, 3.0, 0.0).unwrap(), [0.0, 1.0, 0.0]),
                axis_plane(Point3::try_new(0.0, 0.0, 4.0).unwrap(), [0.0, 0.0, 1.0]),
            ],
            Tolerance::DEFAULT,
        )
        .unwrap();
        assert_eq!(intersection, Point3::try_new(2.0, 3.0, 4.0).unwrap());
    }

    #[test]
    fn rejects_parallel_planes() {
        let result = intersect_three_planes(
            [
                axis_plane(Point3::try_new(0.0, 0.0, 0.0).unwrap(), [1.0, 0.0, 0.0]),
                axis_plane(Point3::try_new(1.0, 0.0, 0.0).unwrap(), [1.0, 0.0, 0.0]),
                axis_plane(Point3::try_new(0.0, 0.0, 0.0).unwrap(), [0.0, 1.0, 0.0]),
            ],
            Tolerance::DEFAULT,
        );
        assert_eq!(result, Err(GeometryError::SingularSystem));
    }
}
