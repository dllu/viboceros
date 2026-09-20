//! Prepared orthogonal projection onto infinite lines and planes.
use crate::exact_scalar::{Rational, rational, scalar};
use crate::{GeometryError, Point3, Vector3};
use num_traits::Zero;

#[cfg(test)]
mod tests;

/// An orthogonal point projector, not a geometry-flattening transform.
///
/// Definitions use the exact values of their binary64 inputs, with one rounding
/// per output coordinate. Intermediate differences, cross products and dot
/// products may exceed the floating-point range. Only degenerate definitions
/// and unrepresentable final points fail; there is no implicit distance tolerance.
#[derive(Clone, Debug)]
pub struct PointProjection3 {
    origin: Point3,
    axis: [Rational; 3],
    squared_length: Rational,
    line: bool,
    coordinate_axis: Option<usize>,
}

impl PointProjection3 {
    /// Project onto the supporting line through two distinct points (unclamped).
    pub fn onto_line(start: Point3, end: Point3) -> Result<Self, GeometryError> {
        Self::new(start, difference(end, start), true)
    }

    /// Project onto a plane through `origin` with a nonzero, not necessarily
    /// normalized normal. Normal length does not affect the result.
    pub fn onto_plane(origin: Point3, normal: Vector3) -> Result<Self, GeometryError> {
        Self::new(origin, normal.to_array().map(rational), false)
    }

    /// Project onto the plane through three noncollinear points.
    pub fn onto_three_point_plane(points: [Point3; 3]) -> Result<Self, GeometryError> {
        Self::new(
            points[0],
            cross(
                &difference(points[1], points[0]),
                &difference(points[2], points[0]),
            ),
            false,
        )
    }

    /// Project onto the plane containing the reference line and parallel to
    /// `direction`. The reference line and direction must not be parallel.
    pub fn onto_plane_parallel_to(
        start: Point3,
        end: Point3,
        direction: Vector3,
    ) -> Result<Self, GeometryError> {
        Self::new(
            start,
            cross(&difference(end, start), &direction.to_array().map(rational)),
            false,
        )
    }

    fn new(origin: Point3, axis: [Rational; 3], line: bool) -> Result<Self, GeometryError> {
        let squared_length = axis.iter().map(|a| a * a).sum::<Rational>();
        if squared_length.is_zero() {
            return Err(GeometryError::Degenerate {
                context: "point projection definition",
            });
        }
        let nonzero = axis.iter().filter(|a| !a.is_zero()).count();
        let coordinate_axis =
            (nonzero == 1).then(|| axis.iter().position(|a| !a.is_zero()).unwrap());
        Ok(Self {
            origin,
            axis,
            squared_length,
            line,
            coordinate_axis,
        })
    }

    pub fn project(&self, point: Point3) -> Result<Point3, GeometryError> {
        let origin = self.origin.to_array();
        let input = point.to_array();
        if let Some(axis) = self.coordinate_axis {
            // Exact coordinate replacement, with no allocation for axis-aligned
            // queries and no loss of tangential coordinates at extreme scales.
            return Point3::try_from(std::array::from_fn(|i| {
                if (i == axis) == self.line {
                    input[i]
                } else {
                    origin[i]
                }
            }));
        }
        let parameter = self
            .axis
            .iter()
            .zip(difference(point, self.origin))
            .map(|(a, d)| a * d)
            .sum::<Rational>()
            / &self.squared_length;
        let output: [Result<f64, GeometryError>; 3] = std::array::from_fn(|i| {
            scalar(&if self.line {
                rational(origin[i]) + &self.axis[i] * &parameter
            } else {
                rational(input[i]) - &self.axis[i] * &parameter
            })
        });
        let [x, y, z] = output;
        Point3::try_new(x?, y?, z?)
    }
}

fn difference(a: Point3, b: Point3) -> [Rational; 3] {
    let (a, b) = (a.to_array(), b.to_array());
    std::array::from_fn(|i| rational(a[i]) - rational(b[i]))
}

fn cross(a: &[Rational; 3], b: &[Rational; 3]) -> [Rational; 3] {
    std::array::from_fn(|i| {
        let (j, k) = ((i + 1) % 3, (i + 2) % 3);
        &a[j] * &b[k] - &a[k] * &b[j]
    })
}
