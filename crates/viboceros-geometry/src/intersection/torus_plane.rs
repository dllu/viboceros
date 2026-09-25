//! Sections of a canonical torus and a finite planar patch.

mod parallel_offset;

use super::{SurfaceSurfaceIntersectionEvent, intersect_curve_with_planar_surface};
use crate::{Circle3, Frame3, GeometryError, NurbsSurface, Plane, Real, Tolerance};

pub(super) fn intersect(
    (frame, major_radius, minor_radius): (Frame3, Real, Real),
    planar_surface: &NurbsSurface,
    plane: Plane,
    tolerance: Tolerance,
) -> Result<Vec<SurfaceSurfaceIntersectionEvent>, GeometryError> {
    let axis = frame.z_axis().as_vector();
    let normal = plane.normal().as_vector();
    let axial_dot = axis.dot(normal)?;
    let signed_distance = plane.signed_distance_to(frame.origin())?;
    let coordinate_scale = frame
        .origin()
        .to_array()
        .into_iter()
        .chain(plane.origin().to_array())
        .map(Real::abs)
        .fold(0.0, Real::max);
    let distance_tolerance = tolerance
        .absolute()
        .max(tolerance.relative() * (major_radius + minor_radius))
        .max(8.0 * Real::EPSILON * coordinate_scale);
    let angular_tolerance = tolerance
        .angular()
        .max(distance_tolerance / (major_radius + minor_radius));
    if signed_distance.abs() > major_radius + minor_radius + distance_tolerance {
        return Ok(Vec::new());
    }
    if axis.cross(normal)?.length()? <= angular_tolerance {
        let axial = -signed_distance / axial_dot;
        if axial.abs() > minor_radius + distance_tolerance {
            return Ok(Vec::new());
        }
        let root = ((minor_radius - axial.abs()) * (minor_radius + axial.abs()))
            .max(0.0)
            .sqrt();
        let center = frame.point_at([0.0, 0.0, axial])?;
        let radii: &[Real] = if root <= distance_tolerance {
            &[major_radius]
        } else {
            &[major_radius + root, major_radius - root]
        };
        let mut events = Vec::new();
        for &radius in radii {
            let circle =
                Circle3::try_from_frame(center, radius, frame.x_axis(), frame.z_axis(), tolerance)?
                    .to_nurbs()?;
            events.extend(intersect_curve_with_planar_surface(
                &circle,
                planar_surface,
                tolerance,
            )?);
        }
        return Ok(events);
    }
    if axial_dot.abs() <= angular_tolerance && signed_distance.abs() <= distance_tolerance {
        let radial = axis.cross(normal)?.normalized_nonzero()?.as_vector();
        let center_on_plane = frame
            .origin()
            .translated(normal.scaled(-signed_distance)?)?;
        let mut events = Vec::new();
        for sign in [1.0, -1.0] {
            let center = center_on_plane.translated(radial.scaled(sign * major_radius)?)?;
            let circle =
                Circle3::try_new(center, minor_radius, plane.normal(), tolerance)?.to_nurbs()?;
            events.extend(intersect_curve_with_planar_surface(
                &circle,
                planar_surface,
                tolerance,
            )?);
        }
        return Ok(events);
    }
    if axial_dot.abs() <= angular_tolerance {
        return parallel_offset::intersect(
            frame,
            major_radius,
            minor_radius,
            planar_surface,
            plane,
            signed_distance,
            tolerance,
            distance_tolerance,
        );
    }
    Err(GeometryError::UnsupportedSurfaceSurfaceIntersection {
        context: "oblique torus/plane section",
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Point3, Vector3, surface_surface_intersection_events};

    fn point(x: Real, y: Real, z: Real) -> Point3 {
        Point3::try_new(x, y, z).unwrap()
    }

    fn frame() -> Frame3 {
        Frame3::try_from_normal(
            point(0.0, 0.0, 0.0),
            Vector3::try_new(0.0, 0.0, 1.0).unwrap(),
            Tolerance::DEFAULT,
        )
        .unwrap()
    }

    fn torus() -> NurbsSurface {
        NurbsSurface::try_torus(frame(), 4.0, 1.0).unwrap()
    }

    fn horizontal(z: Real, x_low: Real, x_high: Real) -> NurbsSurface {
        NurbsSurface::try_bilinear([
            point(x_low, -6.0, z),
            point(x_high, -6.0, z),
            point(x_high, 6.0, z),
            point(x_low, 6.0, z),
        ])
        .unwrap()
    }

    fn meridional(y_low: Real, y_high: Real) -> NurbsSurface {
        NurbsSurface::try_bilinear([
            point(0.0, y_low, -2.0),
            point(0.0, y_high, -2.0),
            point(0.0, y_high, 2.0),
            point(0.0, y_low, 2.0),
        ])
        .unwrap()
    }

    fn assert_on_torus(location: Point3) {
        let radial = location.x().hypot(location.y());
        assert!(((radial - 4.0).hypot(location.z()) - 1.0).abs() < 5e-9);
    }

    #[test]
    fn horizontal_torus_sections_are_two_exact_circles_or_one_tangent_circle() {
        let torus = torus();
        for (height, radii) in [
            (0.0, vec![5.0, 3.0]),
            (0.6, vec![4.8, 3.2]),
            (1.0, vec![4.0]),
        ] {
            let patch = horizontal(height, -6.0, 6.0);
            for (left, right) in [(&torus, &patch), (&patch, &torus)] {
                let events =
                    surface_surface_intersection_events(left, right, Tolerance::DEFAULT).unwrap();
                assert_eq!(events.len(), radii.len());
                for (event, radius) in events.iter().zip(&radii) {
                    let SurfaceSurfaceIntersectionEvent::Curve(circle) = event else {
                        panic!("horizontal torus section must be a circle")
                    };
                    assert_eq!(circle.degree(), 2);
                    assert!(circle.is_closed().unwrap());
                    for index in 0..=16 {
                        let domain = circle.domain();
                        let parameter = *domain.start()
                            + (*domain.end() - *domain.start()) * (index as Real / 16.0);
                        let location = circle.evaluate(parameter).unwrap();
                        assert!((location.z() - height).abs() < 5e-9);
                        assert!((location.x().hypot(location.y()) - radius).abs() < 5e-9);
                        assert_on_torus(location);
                    }
                }
            }
        }
        assert!(
            surface_surface_intersection_events(
                &torus,
                &horizontal(1.5, -6.0, 6.0),
                Tolerance::DEFAULT,
            )
            .unwrap()
            .is_empty()
        );
    }

    #[test]
    fn meridional_torus_sections_are_two_circles_and_finite_patches_clip_them() {
        let torus = torus();
        for (patch, expected) in [(meridional(-6.0, 6.0), 2), (meridional(2.5, 5.5), 1)] {
            let events =
                surface_surface_intersection_events(&torus, &patch, Tolerance::DEFAULT).unwrap();
            assert_eq!(events.len(), expected);
            for event in events {
                let SurfaceSurfaceIntersectionEvent::Curve(circle) = event else {
                    panic!("axis-containing plane should retain circular sections")
                };
                assert_eq!(circle.degree(), 2);
                assert!(circle.is_closed().unwrap());
                for index in 0..=16 {
                    let domain = circle.domain();
                    let parameter = *domain.start()
                        + (*domain.end() - *domain.start()) * (index as Real / 16.0);
                    let location = circle.evaluate(parameter).unwrap();
                    assert!(location.x().abs() < 5e-9);
                    assert_on_torus(location);
                }
            }
        }
        let half = horizontal(0.0, 0.0, 6.0);
        let clipped =
            surface_surface_intersection_events(&torus, &half, Tolerance::DEFAULT).unwrap();
        assert!(!clipped.is_empty());
        for event in clipped {
            let SurfaceSurfaceIntersectionEvent::Curve(arc) = event else {
                panic!("half patch should clip horizontal circles to arcs")
            };
            assert!(!arc.is_closed().unwrap());
            for index in 0..=16 {
                let domain = arc.domain();
                let parameter =
                    *domain.start() + (*domain.end() - *domain.start()) * (index as Real / 16.0);
                let location = arc.evaluate(parameter).unwrap();
                assert!(location.x() >= -5e-9);
                assert_on_torus(location);
            }
        }
    }

    #[test]
    fn torus_plane_sections_handle_distant_rotated_coordinates() {
        let rotated = Frame3::try_from_normal(
            point(1.0e8, -1.0e8, 1.0e8),
            Vector3::try_new(1.0, 2.0, 3.0).unwrap(),
            Tolerance::DEFAULT,
        )
        .unwrap();
        let torus = NurbsSurface::try_torus(rotated, 4.0, 1.0).unwrap();
        let corner = |x: Real, y: Real| rotated.point_at([x, y, 0.6]).unwrap();
        let patch = NurbsSurface::try_bilinear([
            corner(-6.0, -6.0),
            corner(6.0, -6.0),
            corner(6.0, 6.0),
            corner(-6.0, 6.0),
        ])
        .unwrap();
        let events =
            surface_surface_intersection_events(&torus, &patch, Tolerance::DEFAULT).unwrap();
        assert_eq!(events.len(), 2);
        for event in events {
            let SurfaceSurfaceIntersectionEvent::Curve(circle) = event else {
                panic!("rotated torus should retain circles")
            };
            for index in 0..=16 {
                let domain = circle.domain();
                let parameter =
                    *domain.start() + (*domain.end() - *domain.start()) * (index as Real / 16.0);
                let local = rotated
                    .coordinates_of(circle.evaluate(parameter).unwrap())
                    .unwrap();
                assert!((local[2] - 0.6).abs() < 4e-7);
                assert!(((local[0].hypot(local[1]) - 4.0).hypot(local[2]) - 1.0).abs() < 4e-7);
            }
        }
    }
}
