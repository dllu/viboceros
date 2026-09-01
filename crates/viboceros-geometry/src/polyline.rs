use std::f64::consts::TAU;

use crate::{
    AffineTransform3, BoundingBox3, Circle3, GeometryError, LineSegment, NurbsCurve, Point3, Real,
    Tolerance, UnitVector3, require_finite,
};

/// Allocation guard for regular-polygon construction.
pub const MAX_REGULAR_POLYGON_SIDES: usize = 100_000;

/// A finite piecewise-linear curve with validated, non-degenerate segments.
///
/// Closed polylines repeat their first vertex at the end. Non-adjacent
/// duplicate vertices and self-intersections are intentionally permitted.
#[derive(Clone, Debug, PartialEq)]
pub struct Polyline3 {
    vertices: Vec<Point3>,
}

impl Polyline3 {
    pub fn try_new(vertices: Vec<Point3>, tolerance: Tolerance) -> Result<Self, GeometryError> {
        if vertices.len() < 2 {
            return Err(GeometryError::InsufficientPolylineVertices);
        }
        for (segment, points) in vertices.windows(2).enumerate() {
            match LineSegment::try_new(points[0], points[1], tolerance) {
                Ok(_) => {}
                Err(GeometryError::Degenerate { .. }) => {
                    return Err(GeometryError::DegeneratePolylineSegment { segment });
                }
                Err(error) => return Err(error),
            }
        }
        Ok(Self { vertices })
    }

    /// Constructs a closed regular polygon in the plane described by
    /// `normal`. The first vertex is projected into that plane and fixes both
    /// the circumradius and angular phase.
    pub fn try_regular_polygon(
        side_count: usize,
        center: Point3,
        first_vertex: Point3,
        normal: UnitVector3,
        tolerance: Tolerance,
    ) -> Result<Self, GeometryError> {
        if !(3..=MAX_REGULAR_POLYGON_SIDES).contains(&side_count) {
            return Err(GeometryError::InvalidRegularPolygonSides {
                actual: side_count,
                maximum: MAX_REGULAR_POLYGON_SIDES,
            });
        }
        let circle = Circle3::try_from_center_point(center, first_vertex, normal, tolerance)?;
        let mut vertices = Vec::with_capacity(side_count + 1);
        for index in 0..side_count {
            let angle = TAU * index as Real / side_count as Real;
            vertices.push(circle.point_at_angle(angle)?);
        }
        vertices.push(vertices[0]);
        Self::try_new(vertices, tolerance)
    }

    #[inline]
    pub fn vertices(&self) -> &[Point3] {
        &self.vertices
    }

    #[inline]
    pub fn segment_count(&self) -> usize {
        self.vertices.len() - 1
    }

    #[inline]
    pub fn is_closed(&self) -> bool {
        self.vertices.first() == self.vertices.last()
    }

    pub fn segments(&self) -> impl ExactSizeIterator<Item = LineSegment> + '_ {
        self.vertices
            .windows(2)
            .map(|points| LineSegment::from_validated(points[0], points[1]))
    }

    pub fn bounds(&self) -> BoundingBox3 {
        BoundingBox3::from_points(self.vertices.iter().copied())
            .expect("a validated polyline has vertices")
    }

    pub fn length(&self) -> Result<Real, GeometryError> {
        // Neumaier summation retains small segments beside large ones while
        // still reporting an unrepresentable total instead of storing inf.
        let mut sum = 0.0;
        let mut correction = 0.0;
        for segment in self.segments() {
            let length = segment.length()?;
            let next = sum + length;
            if sum.abs() >= length.abs() {
                correction += (sum - next) + length;
            } else {
                correction += (length - next) + sum;
            }
            sum = next;
        }
        let length = sum + correction;
        require_finite([length], "polyline length")?;
        Ok(length)
    }

    pub fn transformed(
        &self,
        transform: AffineTransform3,
        tolerance: Tolerance,
    ) -> Result<Self, GeometryError> {
        let vertices = self
            .vertices
            .iter()
            .map(|vertex| transform.transform_point(*vertex))
            .collect::<Result<Vec<_>, _>>()?;
        Self::try_new(vertices, tolerance)
    }

    /// Returns the exact degree-one NURBS representation.
    pub fn to_nurbs(&self) -> Result<NurbsCurve, GeometryError> {
        NurbsCurve::try_clamped_uniform(1, self.vertices.clone())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Vector3;

    fn point(x: Real, y: Real, z: Real) -> Point3 {
        Point3::try_new(x, y, z).unwrap()
    }

    #[test]
    fn validates_segments_and_closed_state() {
        assert_eq!(
            Polyline3::try_new(vec![point(0.0, 0.0, 0.0)], Tolerance::DEFAULT),
            Err(GeometryError::InsufficientPolylineVertices)
        );
        assert_eq!(
            Polyline3::try_new(
                vec![
                    point(0.0, 0.0, 0.0),
                    point(1.0, 0.0, 0.0),
                    point(1.0, 0.0, 0.0),
                ],
                Tolerance::DEFAULT,
            ),
            Err(GeometryError::DegeneratePolylineSegment { segment: 1 })
        );

        let closed = Polyline3::try_new(
            vec![
                point(0.0, 0.0, 0.0),
                point(3.0, 0.0, 0.0),
                point(3.0, 4.0, 0.0),
                point(0.0, 0.0, 0.0),
            ],
            Tolerance::DEFAULT,
        )
        .unwrap();
        assert!(closed.is_closed());
        assert_eq!(closed.segment_count(), 3);
    }

    #[test]
    fn computes_bounds_length_and_exact_linear_nurbs() {
        let polyline = Polyline3::try_new(
            vec![
                point(-2.0, 1.0, 3.0),
                point(1.0, 5.0, 3.0),
                point(1.0, 5.0, 15.0),
            ],
            Tolerance::DEFAULT,
        )
        .unwrap();
        assert_eq!(polyline.length().unwrap(), 17.0);
        assert_eq!(polyline.bounds().min(), point(-2.0, 1.0, 3.0));
        assert_eq!(polyline.bounds().max(), point(1.0, 5.0, 15.0));

        let curve = polyline.to_nurbs().unwrap();
        assert_eq!(curve.degree(), 1);
        assert_eq!(curve.control_points().len(), 3);
        assert_eq!(curve.evaluate(0.0).unwrap(), point(-2.0, 1.0, 3.0));
        assert_eq!(curve.evaluate(0.5).unwrap(), point(1.0, 5.0, 3.0));
        assert_eq!(curve.evaluate(1.0).unwrap(), point(1.0, 5.0, 15.0));
    }

    #[test]
    fn transform_revalidates_every_segment() {
        let polyline = Polyline3::try_new(
            vec![point(0.0, 0.0, 0.0), point(2.0, 0.0, 0.0)],
            Tolerance::DEFAULT,
        )
        .unwrap();
        let moved = polyline
            .transformed(
                AffineTransform3::from_translation(Vector3::try_new(1.0, 2.0, 3.0).unwrap()),
                Tolerance::DEFAULT,
            )
            .unwrap();
        assert_eq!(
            moved.vertices(),
            &[point(1.0, 2.0, 3.0), point(3.0, 2.0, 3.0)]
        );

        let collapsed = AffineTransform3::try_new(
            [[0.0; 3], [0.0; 3], [0.0; 3]],
            Vector3::try_new(0.0, 0.0, 0.0).unwrap(),
        )
        .unwrap();
        assert!(polyline.transformed(collapsed, Tolerance::DEFAULT).is_err());
    }

    #[test]
    fn regular_polygon_is_closed_oriented_and_bounded() {
        let normal = UnitVector3::try_new(0.0, 0.0, 1.0, Tolerance::DEFAULT).unwrap();
        let polygon = Polyline3::try_regular_polygon(
            6,
            point(1.0, 2.0, 3.0),
            point(5.0, 2.0, 8.0),
            normal,
            Tolerance::DEFAULT,
        )
        .unwrap();
        assert!(polygon.is_closed());
        assert_eq!(polygon.segment_count(), 6);
        assert_eq!(polygon.vertices()[0], point(5.0, 2.0, 3.0));
        assert!(polygon.vertices().iter().all(|vertex| vertex.z() == 3.0));
        for vertex in &polygon.vertices()[..6] {
            assert!(
                Tolerance::DEFAULT
                    .approx_eq(vertex.distance_to(point(1.0, 2.0, 3.0)).unwrap(), 4.0)
            );
        }
    }

    #[test]
    fn regular_polygon_rejects_side_and_tolerance_degeneracy() {
        let normal = UnitVector3::try_new(0.0, 0.0, 1.0, Tolerance::DEFAULT).unwrap();
        assert!(matches!(
            Polyline3::try_regular_polygon(
                2,
                point(0.0, 0.0, 0.0),
                point(1.0, 0.0, 0.0),
                normal,
                Tolerance::DEFAULT,
            ),
            Err(GeometryError::InvalidRegularPolygonSides { actual: 2, .. })
        ));
        assert!(
            Polyline3::try_regular_polygon(
                3,
                point(0.0, 0.0, 0.0),
                point(1.0e-12, 0.0, 0.0),
                normal,
                Tolerance::DEFAULT,
            )
            .is_err()
        );
    }
}
