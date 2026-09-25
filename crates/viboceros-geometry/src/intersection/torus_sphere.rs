//! Exact circles where a sphere centered on a ring torus axis cuts its tube.

use super::SurfaceSurfaceIntersectionEvent;
use crate::{Circle3, Frame3, GeometryError, Point3, Real, Tolerance};

pub(super) fn intersect(
    (torus_frame, major_radius, minor_radius): (Frame3, Real, Real),
    (sphere_center, sphere_radius): (Point3, Real),
    tolerance: Tolerance,
) -> Result<Vec<SurfaceSurfaceIntersectionEvent>, GeometryError> {
    let coordinate_scale = torus_frame
        .origin()
        .to_array()
        .into_iter()
        .chain(sphere_center.to_array())
        .map(Real::abs)
        .fold(0.0, Real::max);
    let spatial_tolerance = tolerance
        .absolute()
        .max(tolerance.relative() * (major_radius + minor_radius).max(sphere_radius))
        .max(8.0 * Real::EPSILON * coordinate_scale);
    let [offset_x, offset_y, sphere_height] = torus_frame.coordinates_of(sphere_center)?;
    if offset_x.hypot(offset_y) > spatial_tolerance {
        return Err(GeometryError::UnsupportedSurfaceSurfaceIntersection {
            context: "sphere center outside torus axis",
        });
    }

    // A meridian of each surface is a circle in (radial distance, height).
    // Their intersections revolve into the required spatial circles.
    let center_distance = major_radius.hypot(sphere_height);
    if center_distance > minor_radius + sphere_radius + spatial_tolerance
        || center_distance + minor_radius + spatial_tolerance < sphere_radius
        || center_distance + sphere_radius + spatial_tolerance < minor_radius
    {
        return Ok(Vec::new());
    }
    let along = ((minor_radius - sphere_radius) * (minor_radius + sphere_radius)
        + center_distance * center_distance)
        / (2.0 * center_distance);
    let transverse = ((minor_radius - along) * (minor_radius + along))
        .max(0.0)
        .sqrt();
    let radial_base = major_radius * (1.0 - along / center_distance);
    let height_base = along * sphere_height / center_distance;
    let radial_step = -transverse * sphere_height / center_distance;
    let height_step = -transverse * major_radius / center_distance;
    let mut sections = if transverse <= spatial_tolerance {
        vec![(radial_base, height_base)]
    } else {
        vec![
            (radial_base + radial_step, height_base + height_step),
            (radial_base - radial_step, height_base - height_step),
        ]
    };
    sections.sort_by(|left, right| right.1.total_cmp(&left.1));
    let mut events = Vec::with_capacity(sections.len());
    for (radius, height) in sections {
        let center = torus_frame.point_at([0.0, 0.0, height])?;
        let circle = Circle3::try_from_frame(
            center,
            radius,
            torus_frame.x_axis(),
            torus_frame.z_axis(),
            tolerance,
        )?
        .to_nurbs()?;
        events.push(SurfaceSurfaceIntersectionEvent::Curve(circle));
    }
    Ok(events)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{NurbsSurface, Vector3, surface_surface_intersection_events};

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

    fn sphere(center: Point3, radius: Real) -> NurbsSurface {
        let sphere_frame = Frame3::try_from_normal(
            center,
            Vector3::try_new(0.0, 0.0, 1.0).unwrap(),
            Tolerance::DEFAULT,
        )
        .unwrap();
        NurbsSurface::try_sphere(sphere_frame, radius).unwrap()
    }

    #[test]
    fn axis_centered_spheres_cut_exact_torus_circles_in_both_orders() {
        let torus = NurbsSurface::try_torus(frame(), 4.0, 1.0).unwrap();
        for (center, radius, expected) in [
            (point(0.0, 0.0, 0.0), 4.0, 2),
            (point(0.0, 0.0, 0.0), 3.0, 1),
            (point(0.0, 0.0, 0.0), 5.0, 1),
            (point(0.0, 0.0, 0.7), 4.0, 2),
            (point(0.0, 0.0, 0.0), 2.0, 0),
            (point(0.0, 0.0, 0.0), 6.0, 0),
        ] {
            let sphere = sphere(center, radius);
            for (left, right) in [(&torus, &sphere), (&sphere, &torus)] {
                let events =
                    surface_surface_intersection_events(left, right, Tolerance::DEFAULT).unwrap();
                assert_eq!(events.len(), expected);
                for event in events {
                    let SurfaceSurfaceIntersectionEvent::Curve(circle) = event else {
                        panic!("axis-centered torus/sphere intersection should be a circle")
                    };
                    assert_eq!(circle.degree(), 2);
                    assert!(circle.is_closed().unwrap());
                    for index in 0..=16 {
                        let domain = circle.domain();
                        let parameter = *domain.start()
                            + (*domain.end() - *domain.start()) * (index as Real / 16.0);
                        let location = circle.evaluate(parameter).unwrap();
                        let radial = location.x().hypot(location.y());
                        assert!(((radial - 4.0).hypot(location.z()) - 1.0).abs() < 5e-9);
                        assert!((radial.hypot(location.z() - center.z()) - radius).abs() < 5e-9);
                    }
                }
            }
        }
    }

    #[test]
    fn offset_sphere_center_is_unsupported() {
        let torus = NurbsSurface::try_torus(frame(), 4.0, 1.0).unwrap();
        let offset = sphere(point(0.2, 0.0, 0.0), 4.0);
        assert!(matches!(
            surface_surface_intersection_events(&torus, &offset, Tolerance::DEFAULT),
            Err(GeometryError::UnsupportedSurfaceSurfaceIntersection { .. })
        ));
    }

    #[test]
    fn axis_centered_sphere_handles_distant_rotated_frame() {
        let rotated = Frame3::try_from_normal(
            point(1.0e8, -1.0e8, 1.0e8),
            Vector3::try_new(1.0, 2.0, 3.0).unwrap(),
            Tolerance::DEFAULT,
        )
        .unwrap();
        let torus = NurbsSurface::try_torus(rotated, 4.0, 1.0).unwrap();
        let sphere_center = rotated.point_at([0.0, 0.0, 0.7]).unwrap();
        let sphere = sphere(sphere_center, 4.0);
        let events =
            surface_surface_intersection_events(&torus, &sphere, Tolerance::DEFAULT).unwrap();
        assert_eq!(events.len(), 2);
        for event in events {
            let SurfaceSurfaceIntersectionEvent::Curve(circle) = event else {
                panic!("rotated torus/sphere section should be a circle")
            };
            for index in 0..=16 {
                let domain = circle.domain();
                let parameter =
                    *domain.start() + (*domain.end() - *domain.start()) * (index as Real / 16.0);
                let location = circle.evaluate(parameter).unwrap();
                let local = rotated.coordinates_of(location).unwrap();
                let radial = local[0].hypot(local[1]);
                assert!(((radial - 4.0).hypot(local[2]) - 1.0).abs() < 4e-7);
                assert!((location.distance_to(sphere_center).unwrap() - 4.0).abs() < 4e-7);
            }
        }
    }
}
