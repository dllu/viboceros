use crate::{AffineTransform3, GeometryError, Point3, Real, Tolerance, UnitVector3};

/// A non-degenerate finite line segment.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct LineSegment {
    start: Point3,
    end: Point3,
}

impl LineSegment {
    pub fn try_new(
        start: Point3,
        end: Point3,
        tolerance: Tolerance,
    ) -> Result<Self, GeometryError> {
        if start.is_near(end, tolerance) {
            Err(GeometryError::Degenerate {
                context: "line segment",
            })
        } else {
            // Force overflow detection for extreme endpoint differences.
            start.vector_to(end)?;
            Ok(Self { start, end })
        }
    }

    #[inline]
    pub const fn start(self) -> Point3 {
        self.start
    }

    #[inline]
    pub const fn end(self) -> Point3 {
        self.end
    }

    pub fn length(self) -> Result<Real, GeometryError> {
        self.start.vector_to(self.end)?.length()
    }

    pub fn direction(self, tolerance: Tolerance) -> Result<UnitVector3, GeometryError> {
        self.start.vector_to(self.end)?.normalized(tolerance)
    }

    pub fn point_at(self, parameter: Real) -> Result<Point3, GeometryError> {
        let offset = self.start.vector_to(self.end)?.scaled(parameter)?;
        self.start.translated(offset)
    }

    pub fn transformed(
        self,
        transform: AffineTransform3,
        tolerance: Tolerance,
    ) -> Result<Self, GeometryError> {
        Self::try_new(
            transform.transform_point(self.start)?,
            transform.transform_point(self.end)?,
            tolerance,
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn point(x: Real, y: Real, z: Real) -> Point3 {
        Point3::try_new(x, y, z).unwrap()
    }

    #[test]
    fn rejects_short_segments() {
        assert!(
            LineSegment::try_new(
                point(0.0, 0.0, 0.0),
                point(Tolerance::DEFAULT.absolute() / 2.0, 0.0, 0.0),
                Tolerance::DEFAULT,
            )
            .is_err()
        );
    }

    #[test]
    fn evaluates_endpoints_and_midpoint() {
        let line = LineSegment::try_new(
            point(1.0, 2.0, 3.0),
            point(5.0, 6.0, 7.0),
            Tolerance::DEFAULT,
        )
        .unwrap();
        assert_eq!(line.point_at(0.0).unwrap(), line.start());
        assert_eq!(line.point_at(1.0).unwrap(), line.end());
        assert_eq!(line.point_at(0.5).unwrap(), point(3.0, 4.0, 5.0));
    }

    #[test]
    fn affine_transform_revalidates_the_segment() {
        let line = LineSegment::try_new(
            point(1.0, 2.0, 3.0),
            point(5.0, 6.0, 7.0),
            Tolerance::DEFAULT,
        )
        .unwrap();
        let moved = line
            .transformed(
                AffineTransform3::from_translation(
                    crate::Vector3::try_new(10.0, -2.0, 1.0).unwrap(),
                ),
                Tolerance::DEFAULT,
            )
            .unwrap();
        assert_eq!(moved.start(), point(11.0, 0.0, 4.0));
        assert_eq!(moved.end(), point(15.0, 4.0, 8.0));

        let collapsed = AffineTransform3::try_new(
            [[0.0; 3], [0.0; 3], [0.0; 3]],
            crate::Vector3::try_new(0.0, 0.0, 0.0).unwrap(),
        )
        .unwrap();
        assert!(line.transformed(collapsed, Tolerance::DEFAULT).is_err());
    }
}
