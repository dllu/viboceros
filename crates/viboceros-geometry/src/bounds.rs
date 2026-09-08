use crate::{GeometryError, Point3};
mod bezier;
mod curves;
mod parameter_curves;
mod surfaces;
mod trim_region;
mod trimmed_surfaces;

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
        self.min.midpoint(self.max)
    }

    pub fn union(self, other: Self) -> Result<Self, GeometryError> {
        Self::from_points([self.min, self.max, other.min, other.max])
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Real;

    #[test]
    fn subnormal_centers_round_the_exact_midpoint_once() {
        let unit = Real::from_bits(1);
        for left in -32..=32 {
            for right in -32..=32 {
                let bounds = BoundingBox3::from_points([
                    Point3::try_new(Real::from(left) * unit, 0.0, 0.0).unwrap(),
                    Point3::try_new(Real::from(right) * unit, 0.0, 0.0).unwrap(),
                ])
                .unwrap();
                // Integer subnormal units make this an exact reference: round
                // the half-integer to even before scaling by the minimum unit.
                let expected = (Real::from(left + right) * 0.5).round_ties_even() * unit;
                assert_eq!(
                    bounds.center().unwrap().x(),
                    expected,
                    "units {left}, {right}"
                );
            }
        }
    }

    #[test]
    fn centers_handle_extreme_and_degenerate_boxes_on_every_axis() {
        for (left, right, expected) in [
            (Real::MAX, Real::MAX, Real::MAX),
            (-Real::MAX, -Real::MAX, -Real::MAX),
            (-Real::MAX, Real::MAX, 0.0),
            (0.0, Real::MAX, Real::MAX * 0.5),
            (-Real::MAX, 0.0, -Real::MAX * 0.5),
            (1.0, 1.0_f64.next_up(), 1.0),
            (
                1.0_f64.next_up(),
                1.0_f64.next_up().next_up(),
                1.0_f64.next_up().next_up(),
            ),
        ] {
            let bounds = BoundingBox3::from_points([
                Point3::try_new(left, right, left).unwrap(),
                Point3::try_new(right, left, right).unwrap(),
            ])
            .unwrap();
            assert_eq!(bounds.center().unwrap().to_array(), [expected; 3]);
        }
    }

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
