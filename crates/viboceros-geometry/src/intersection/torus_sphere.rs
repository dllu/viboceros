//! Torus/sphere sections: exact axial circles and fitted offset curves.

use super::{SurfaceSurfaceIntersectionEvent, torus_meridian};
use crate::{Circle3, Frame3, GeometryError, Point3, Real, Tolerance, Vector3};

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
    let radial_offset = offset_x.hypot(offset_y);
    if radial_offset > spatial_tolerance {
        // Subtracting the two implicit equations leaves a line in each torus
        // meridian. Its radial coefficient varies with the meridian azimuth.
        let line_constant = 0.5
            * ((major_radius - minor_radius) * (major_radius + minor_radius)
                + (sphere_radius - radial_offset) * (sphere_radius + radial_offset)
                - sphere_height * sphere_height);
        let scale = major_radius
            .max(minor_radius)
            .max(radial_offset)
            .max(sphere_radius);
        if radial_offset >= major_radius
            && sphere_height.abs() <= 16.0 * Real::EPSILON * scale
            && line_constant.abs() <= 16.0 * Real::EPSILON * scale * scale
        {
            // At these azimuths the meridian line vanishes. The whole tube
            // circle belongs to the sphere, so fitting would divide by zero.
            let azimuth = (major_radius / radial_offset).acos();
            let mut events = Vec::new();
            for angle in [azimuth, -azimuth] {
                if !events.is_empty() && angle == 0.0 {
                    break;
                }
                let (sine, cosine) = angle.sin_cos();
                let center =
                    torus_frame.point_at([major_radius * cosine, major_radius * sine, 0.0])?;
                let x = torus_frame.x_axis().as_vector().to_array();
                let y = torus_frame.y_axis().as_vector().to_array();
                let normal = Vector3::try_new(
                    -sine * x[0] + cosine * y[0],
                    -sine * x[1] + cosine * y[1],
                    -sine * x[2] + cosine * y[2],
                )?
                .normalized(tolerance)?;
                let circle = Circle3::try_from_frame(
                    center,
                    minor_radius,
                    torus_frame.z_axis(),
                    normal,
                    tolerance,
                )?
                .to_nurbs()?;
                events.push(SurfaceSurfaceIntersectionEvent::Curve(circle));
            }
            return Ok(events);
        }
        let section = torus_meridian::Section {
            frame: torus_frame,
            major: major_radius,
            minor: minor_radius,
            radial_base: major_radius,
            radial_cosine: -radial_offset,
            axial_coefficient: -sphere_height,
            line_constant,
            radial_axis: [offset_x / radial_offset, offset_y / radial_offset],
        };
        return torus_meridian::intersect_meridian(section, spatial_tolerance);
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
    fn offset_spheres_form_full_and_turned_loops_in_both_orders() {
        let torus = NurbsSurface::try_torus(frame(), 4.0, 1.0).unwrap();
        for (center, radius, expected) in [
            (point(0.5, 0.0, 0.0), 4.0, 2),
            (point(2.0, 0.0, 0.0), 4.0, 2),
            (point(0.5, 0.0, 0.7), 4.0, 2),
            (point(0.5, 0.0, 0.0), 1.0, 0),
        ] {
            let offset = sphere(center, radius);
            for (left, right) in [(&torus, &offset), (&offset, &torus)] {
                let events = surface_surface_intersection_events(left, right, Tolerance::DEFAULT)
                    .unwrap_or_else(|error| panic!("center={center:?}: {error:?}"));
                assert_eq!(events.len(), expected, "center={center:?}");
                for event in events {
                    let SurfaceSurfaceIntersectionEvent::Curve(curve) = event else {
                        panic!("offset torus/sphere section should be a curve")
                    };
                    assert_eq!(curve.degree(), 3);
                    assert!(curve.is_closed().unwrap());
                    for index in 0..=64 {
                        let domain = curve.domain();
                        let parameter = *domain.start()
                            + (*domain.end() - *domain.start()) * (index as Real / 64.0);
                        let location = curve.evaluate(parameter).unwrap();
                        let radial = location.x().hypot(location.y());
                        assert!(((radial - 4.0).hypot(location.z()) - 1.0).abs() < 5e-9);
                        assert!((location.distance_to(center).unwrap() - radius).abs() < 5e-9);
                    }
                }
            }
        }
    }

    #[test]
    fn offset_sphere_tangent_returns_a_point() {
        let torus = NurbsSurface::try_torus(frame(), 4.0, 1.0).unwrap();
        let offset = sphere(point(7.0, 0.0, 0.0), 2.0);
        for (left, right) in [(&torus, &offset), (&offset, &torus)] {
            let events =
                surface_surface_intersection_events(left, right, Tolerance::DEFAULT).unwrap();
            assert_eq!(events.len(), 1);
            let SurfaceSurfaceIntersectionEvent::Point(touch) = events[0] else {
                panic!("externally tangent torus and sphere should meet at a point")
            };
            assert!(touch.distance_to(point(5.0, 0.0, 0.0)).unwrap() < 5e-9);
        }
    }

    #[test]
    fn offset_sphere_can_contain_entire_torus_meridians() {
        let torus = NurbsSurface::try_torus(frame(), 4.0, 1.0).unwrap();
        for (center, radius, expected) in [
            (point(4.0, 0.0, 0.0), 1.0, 1),
            (point(5.0, 0.0, 0.0), 10.0_f64.sqrt(), 2),
        ] {
            let offset = sphere(center, radius);
            let events =
                surface_surface_intersection_events(&torus, &offset, Tolerance::DEFAULT).unwrap();
            assert_eq!(events.len(), expected);
            for event in events {
                let SurfaceSurfaceIntersectionEvent::Curve(circle) = event else {
                    panic!("contained meridian should form a circle")
                };
                assert_eq!(circle.degree(), 2);
                for index in 0..=32 {
                    let domain = circle.domain();
                    let parameter = *domain.start()
                        + (*domain.end() - *domain.start()) * (index as Real / 32.0);
                    let location = circle.evaluate(parameter).unwrap();
                    let radial = location.x().hypot(location.y());
                    assert!(((radial - 4.0).hypot(location.z()) - 1.0).abs() < 5e-9);
                    assert!((location.distance_to(center).unwrap() - radius).abs() < 5e-9);
                }
            }
        }
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

    #[test]
    fn offset_sphere_handles_distant_rotated_frame() {
        let rotated = Frame3::try_from_normal(
            point(1.0e8, -1.0e8, 1.0e8),
            Vector3::try_new(1.0, 2.0, 3.0).unwrap(),
            Tolerance::DEFAULT,
        )
        .unwrap();
        let torus = NurbsSurface::try_torus(rotated, 4.0, 1.0).unwrap();
        let sphere_center = rotated.point_at([0.5, 0.0, 0.7]).unwrap();
        let sphere = sphere(sphere_center, 4.0);
        let events =
            surface_surface_intersection_events(&torus, &sphere, Tolerance::DEFAULT).unwrap();
        assert_eq!(events.len(), 2);
        for event in events {
            let SurfaceSurfaceIntersectionEvent::Curve(curve) = event else {
                panic!("offset rotated torus/sphere section should be a loop")
            };
            for index in 0..=32 {
                let domain = curve.domain();
                let parameter =
                    *domain.start() + (*domain.end() - *domain.start()) * (index as Real / 32.0);
                let location = curve.evaluate(parameter).unwrap();
                let local = rotated.coordinates_of(location).unwrap();
                let radial = local[0].hypot(local[1]);
                assert!(((radial - 4.0).hypot(local[2]) - 1.0).abs() < 4e-7);
                assert!((location.distance_to(sphere_center).unwrap() - 4.0).abs() < 4e-7);
            }
        }
    }
}
