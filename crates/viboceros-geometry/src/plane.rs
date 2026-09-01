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
        self.origin.vector_to(point)?.dot(self.normal.as_vector())
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
