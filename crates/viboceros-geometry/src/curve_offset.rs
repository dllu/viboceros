//! Exact offsets of analytic curves in an oriented plane.

use crate::{Circle3, CircularArc3, Curve3, GeometryError, Point3, Real, Tolerance, UnitVector3};

impl Curve3 {
    /// Offset left of the curve direction for positive `distance`. Lines use
    /// `plane_normal`; circular curves use their own oriented supporting plane.
    /// Native parameter intervals and analytic representations are retained.
    pub fn try_offset(
        &self,
        distance: Real,
        plane_normal: UnitVector3,
        tolerance: Tolerance,
    ) -> Result<Self, GeometryError> {
        if !distance.is_finite() || distance == 0.0 {
            return Err(GeometryError::InvalidCurveOffsetDistance);
        }
        match self {
            Self::Line(line) => {
                let left = plane_normal
                    .as_vector()
                    .cross(line.direction(tolerance)?.as_vector())?
                    .normalized(tolerance)?;
                let shift = left.as_vector().scaled(distance)?;
                let start = line.start().translated(shift)?;
                let end = line.end().translated(shift)?;
                if start == line.start() || end == line.end() {
                    return Err(GeometryError::Degenerate {
                        context: "offset line under model precision",
                    });
                }
                Ok(Self::Line(line.try_with_endpoints(
                    Some(start),
                    Some(end),
                    tolerance,
                )?))
            }
            Self::Circle(circle) => Ok(Self::Circle(offset_circle(*circle, distance, tolerance)?)),
            Self::Arc(arc) => {
                let circle = Circle3::try_from_frame(
                    arc.center(),
                    arc.radius(),
                    arc.x_axis(),
                    arc.normal()?,
                    tolerance,
                )?;
                let offset = offset_circle(circle, distance, tolerance)?;
                Ok(Self::Arc(
                    CircularArc3::try_from_circle_sweep(offset, arc.sweep_radians())?
                        .try_reparameterized(arc.domain())?,
                ))
            }
            _ => Err(GeometryError::UnsupportedCurveOffset),
        }
    }

    /// Returns which signed offset reaches `side` for this curve's oriented
    /// plane. A point on the supporting locus is ambiguous even if it lies
    /// beyond a finite line or arc endpoint.
    pub fn offset_side(
        &self,
        side: Point3,
        plane_normal: UnitVector3,
        tolerance: Tolerance,
    ) -> Result<Real, GeometryError> {
        let signed = match self {
            Self::Line(line) => {
                let left = plane_normal
                    .as_vector()
                    .cross(line.direction(tolerance)?.as_vector())?
                    .normalized(tolerance)?;
                line.start().vector_to(side)?.dot(left.as_vector())?
            }
            Self::Circle(circle) => {
                circle.radius()
                    - in_plane_radius(circle.center(), side, circle.x_axis(), circle.y_axis())?
            }
            Self::Arc(arc) => {
                arc.radius() - in_plane_radius(arc.center(), side, arc.x_axis(), arc.y_axis())?
            }
            _ => return Err(GeometryError::UnsupportedCurveOffset),
        };
        if signed.abs() <= tolerance.absolute() {
            return Err(GeometryError::AmbiguousCurveOffsetSide);
        }
        Ok(signed.signum())
    }
}

fn in_plane_radius(
    center: Point3,
    point: Point3,
    x_axis: UnitVector3,
    y_axis: UnitVector3,
) -> Result<Real, GeometryError> {
    let radial = center.vector_to(point)?;
    Ok(radial
        .dot(x_axis.as_vector())?
        .hypot(radial.dot(y_axis.as_vector())?))
}

fn offset_circle(
    circle: Circle3,
    distance: Real,
    tolerance: Tolerance,
) -> Result<Circle3, GeometryError> {
    let radius = circle.radius() - distance;
    if !radius.is_finite() || radius <= tolerance.absolute() || radius == circle.radius() {
        return Err(GeometryError::Degenerate {
            context: "offset circle radius",
        });
    }
    Circle3::try_from_frame(
        circle.center(),
        radius,
        circle.x_axis(),
        circle.normal()?,
        tolerance,
    )?
    .try_reparameterized(circle.domain())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{LineSegment, Point3, Vector3};

    fn point(x: Real, y: Real, z: Real) -> Point3 {
        Point3::try_new(x, y, z).unwrap()
    }

    #[test]
    fn line_offset_preserves_interval_and_rejects_ambiguous_side() {
        let tol = Tolerance::DEFAULT;
        let normal = Vector3::try_new(0.0, 0.0, 1.0)
            .unwrap()
            .normalized(tol)
            .unwrap();
        let line = LineSegment::try_new(point(0.0, 0.0, 0.0), point(4.0, 0.0, 0.0), tol)
            .unwrap()
            .try_reparameterized(7.0..=9.0)
            .unwrap();
        let curve = Curve3::Line(line);
        assert_eq!(
            curve
                .offset_side(point(2.0, 5.0, 0.0), normal, tol)
                .unwrap(),
            1.0
        );
        assert_eq!(
            curve
                .offset_side(point(2.0, -5.0, 0.0), normal, tol)
                .unwrap(),
            -1.0
        );
        assert_eq!(
            curve.offset_side(point(5.0, 0.0, 0.0), normal, tol),
            Err(GeometryError::AmbiguousCurveOffsetSide)
        );
        let Curve3::Line(offset) = curve.try_offset(2.0, normal, tol).unwrap() else {
            panic!("line")
        };
        assert_eq!(offset.start(), point(0.0, 2.0, 0.0));
        assert_eq!(offset.end(), point(4.0, 2.0, 0.0));
        assert_eq!(offset.domain(), 7.0..=9.0);
    }

    #[test]
    fn circular_offsets_are_exact_and_keep_domains() {
        let tol = Tolerance::DEFAULT;
        let normal = Vector3::try_new(0.0, 0.0, 1.0)
            .unwrap()
            .normalized(tol)
            .unwrap();
        let circle = Circle3::try_new(point(1.0, 2.0, 0.0), 5.0, normal, tol)
            .unwrap()
            .try_reparameterized(10.0..=20.0)
            .unwrap();
        let curve = Curve3::Circle(circle);
        assert_eq!(
            curve
                .offset_side(point(1.0, 2.0, 0.0), normal, tol)
                .unwrap(),
            1.0
        );
        assert_eq!(
            curve
                .offset_side(point(8.0, 2.0, 0.0), normal, tol)
                .unwrap(),
            -1.0
        );
        let Curve3::Circle(offset) = curve.try_offset(-2.0, normal, tol).unwrap() else {
            panic!("circle")
        };
        assert_eq!(offset.radius(), 7.0);
        assert_eq!(offset.domain(), circle.domain());
        assert!(curve.try_offset(5.0, normal, tol).is_err());

        let arc = CircularArc3::try_from_circle_sweep(circle, std::f64::consts::FRAC_PI_2)
            .unwrap()
            .try_reparameterized(3.0..=4.0)
            .unwrap();
        let Curve3::Arc(offset) = Curve3::Arc(arc).try_offset(2.0, normal, tol).unwrap() else {
            panic!("arc")
        };
        assert_eq!(offset.radius(), 3.0);
        assert_eq!(offset.sweep_radians(), arc.sweep_radians());
        assert_eq!(offset.domain(), arc.domain());
    }

    #[test]
    fn unrepresentable_offsets_fail_instead_of_returning_the_input() {
        let tol = Tolerance::DEFAULT;
        let normal = Vector3::try_new(0.0, 0.0, 1.0)
            .unwrap()
            .normalized(tol)
            .unwrap();
        let line = Curve3::Line(
            LineSegment::try_new(point(0.0, 1e16, 0.0), point(4.0, 1e16, 0.0), tol).unwrap(),
        );
        assert!(matches!(
            line.try_offset(1.0, normal, tol),
            Err(GeometryError::Degenerate { .. })
        ));
        let circle =
            Curve3::Circle(Circle3::try_new(point(0.0, 0.0, 0.0), 1e16, normal, tol).unwrap());
        assert!(matches!(
            circle.try_offset(1.0, normal, tol),
            Err(GeometryError::Degenerate { .. })
        ));
    }
}
