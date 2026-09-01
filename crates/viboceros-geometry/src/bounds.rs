use crate::{GeometryError, Point3, Real};

/// Axis-aligned finite bounding box.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct BoundingBox3 {
    min: Point3,
    max: Point3,
}

impl BoundingBox3 {
    pub fn from_points(points: impl IntoIterator<Item = Point3>) -> Result<Self, GeometryError> {
        let mut points = points.into_iter();
        let first = points.next().ok_or(GeometryError::EmptyPointSet)?;
        let mut min = first.to_array();
        let mut max = min;

        for point in points {
            for (axis, value) in point.to_array().into_iter().enumerate() {
                min[axis] = min[axis].min(value);
                max[axis] = max[axis].max(value);
            }
        }

        Ok(Self {
            min: Point3::try_from(min)?,
            max: Point3::try_from(max)?,
        })
    }

    #[inline]
    pub const fn min(self) -> Point3 {
        self.min
    }

    #[inline]
    pub const fn max(self) -> Point3 {
        self.max
    }

    pub fn center(self) -> Result<Point3, GeometryError> {
        // Halving before adding avoids overflow when both values are large.
        Point3::try_new(
            midpoint(self.min.x(), self.max.x()),
            midpoint(self.min.y(), self.max.y()),
            midpoint(self.min.z(), self.max.z()),
        )
    }

    pub fn union(self, other: Self) -> Result<Self, GeometryError> {
        Self::from_points([self.min, self.max, other.min, other.max])
    }
}

#[inline]
fn midpoint(left: Real, right: Real) -> Real {
    if left.is_sign_negative() == right.is_sign_negative() {
        left + (right - left) * 0.5
    } else {
        left * 0.5 + right * 0.5
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn finds_bounds_and_safe_center() {
        let bounds = BoundingBox3::from_points([
            Point3::try_new(-Real::MAX, -2.0, 4.0).unwrap(),
            Point3::try_new(Real::MAX, 6.0, -8.0).unwrap(),
        ])
        .unwrap();
        assert_eq!(
            bounds.center().unwrap(),
            Point3::try_new(0.0, 2.0, -2.0).unwrap()
        );
    }
}
