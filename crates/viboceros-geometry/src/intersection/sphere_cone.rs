//! Finite intersections of a canonical cone and a sphere.

mod noncoaxial_inside_apex;
mod noncoaxial_on_apex;
mod noncoaxial_pinched;
mod noncoaxial_turning;
mod noncoaxial_two_loops;

use super::SurfaceSurfaceIntersectionEvent;
use crate::{Circle3, Frame3, GeometryError, Point3, Real, Tolerance};

pub(super) fn sphere_cone_intersection_events(
    sphere_center: Point3,
    sphere_radius: Real,
    (cone_frame, cone_radius, signed_height): (Frame3, Real, Real),
    tolerance: Tolerance,
) -> Result<Vec<SurfaceSurfaceIntersectionEvent>, GeometryError> {
    let [radial_x, radial_y, axial_coordinate] = cone_frame.coordinates_of(sphere_center)?;
    let radial_offset = radial_x.hypot(radial_y);
    let height = signed_height.abs();
    let axial_center = axial_coordinate * signed_height.signum();
    let coordinate_scale = cone_frame
        .origin()
        .to_array()
        .into_iter()
        .chain(sphere_center.to_array())
        .map(Real::abs)
        .fold(0.0, Real::max);
    let coordinate_roundoff = 8.0 * Real::EPSILON * coordinate_scale;
    let radial_tolerance = tolerance
        .absolute()
        .max(tolerance.relative() * cone_radius.max(sphere_radius).max(radial_offset));
    let coaxial_tolerance = radial_tolerance.max(coordinate_roundoff);
    let axial_tolerance = tolerance
        .absolute()
        .max(tolerance.relative() * height.max(sphere_radius))
        .max(coordinate_roundoff);
    if axial_center + sphere_radius < -axial_tolerance
        || axial_center - sphere_radius > height + axial_tolerance
        || radial_offset > cone_radius + sphere_radius + coaxial_tolerance
    {
        return Ok(Vec::new());
    }
    if radial_offset > coaxial_tolerance {
        let apex_distance = radial_offset.hypot(axial_center);
        if apex_distance < sphere_radius - coaxial_tolerance {
            return noncoaxial_inside_apex::intersect(
                sphere_center,
                sphere_radius,
                (cone_frame, cone_radius, signed_height),
                tolerance,
                coordinate_roundoff,
            );
        }
        if (apex_distance - sphere_radius).abs() <= coaxial_tolerance {
            return noncoaxial_on_apex::intersect(
                sphere_center,
                sphere_radius,
                (cone_frame, cone_radius, signed_height),
                tolerance,
                coordinate_roundoff,
            );
        }
        if apex_distance > sphere_radius + coaxial_tolerance {
            let slope = cone_radius / height;
            let quadratic = slope.mul_add(slope, 1.0);
            let highest_linear = axial_center + slope * radial_offset;
            if highest_linear <= 0.0 {
                return Ok(Vec::new());
            }
            let lowest_linear = axial_center - slope * radial_offset;
            // The generator discriminants factor as A*R² minus a squared
            // perpendicular distance. This avoids subtracting B² and A*C
            // after both have grown large.
            let maximum_distance = radial_offset - slope * axial_center;
            let opposite_distance = radial_offset + slope * axial_center;
            let maximum_discriminant =
                quadratic * sphere_radius * sphere_radius - maximum_distance * maximum_distance;
            let opposite_discriminant =
                quadratic * sphere_radius * sphere_radius - opposite_distance * opposite_distance;
            let scale = highest_linear
                .abs()
                .max(lowest_linear.abs())
                .max(sphere_radius)
                .max(cone_radius);
            let displacement_roundoff = (1.0 + slope.abs()) * coordinate_roundoff;
            let discriminant_roundoff = 64.0 * Real::EPSILON * scale * scale
                + 2.0 * maximum_distance.abs().max(opposite_distance.abs()) * displacement_roundoff
                + displacement_roundoff * displacement_roundoff
                + quadratic * coordinate_roundoff * (2.0 * sphere_radius + coordinate_roundoff);
            if !discriminant_roundoff.is_finite() {
                return Err(GeometryError::UnsupportedSurfaceSurfaceIntersection {
                    context: "noncoaxial sphere/cone discriminant is ill-conditioned",
                });
            }
            if maximum_discriminant < -discriminant_roundoff {
                return Ok(Vec::new());
            }
            if maximum_discriminant.abs() <= discriminant_roundoff {
                let axial = highest_linear / quadratic;
                if axial > height + axial_tolerance || slope * axial <= radial_tolerance {
                    return Ok(Vec::new());
                }
                let radius = slope * axial;
                let point = cone_frame.point_at([
                    radius * radial_x / radial_offset,
                    radius * radial_y / radial_offset,
                    signed_height.signum() * axial,
                ])?;
                return Ok(vec![SurfaceSurfaceIntersectionEvent::Point(point)]);
            }
            if maximum_discriminant > discriminant_roundoff
                && opposite_discriminant < -discriminant_roundoff
            {
                return noncoaxial_turning::intersect(
                    sphere_center,
                    sphere_radius,
                    (cone_frame, cone_radius, signed_height),
                    tolerance,
                    coordinate_roundoff,
                );
            }
            if maximum_discriminant > discriminant_roundoff
                && opposite_discriminant.abs() <= discriminant_roundoff
                && lowest_linear > 0.0
            {
                return noncoaxial_pinched::intersect(
                    sphere_center,
                    sphere_radius,
                    (cone_frame, cone_radius, signed_height),
                    tolerance,
                    coordinate_roundoff,
                );
            }
            if maximum_discriminant > discriminant_roundoff
                && opposite_discriminant > discriminant_roundoff
                && lowest_linear > 0.0
            {
                return noncoaxial_two_loops::intersect(
                    sphere_center,
                    sphere_radius,
                    (cone_frame, cone_radius, signed_height),
                    tolerance,
                    coordinate_roundoff,
                );
            }
        }
        return Err(GeometryError::UnsupportedSurfaceSurfaceIntersection {
            context: "noncoaxial sphere and cone",
        });
    }

    // On the cone, rho = slope * t. Substitute this into the sphere equation.
    let slope = cone_radius / height;
    let slope_squared = slope * slope;
    let coefficient = 1.0 + slope_squared;
    let positive_term = coefficient * sphere_radius * sphere_radius;
    let negative_term = slope_squared * axial_center * axial_center;
    let discriminant = positive_term - negative_term;
    let discriminant_tolerance = (64.0 * Real::EPSILON * (positive_term + negative_term))
        .max(radial_tolerance * radial_tolerance);
    if discriminant < -discriminant_tolerance {
        return Ok(Vec::new());
    }
    let root = discriminant.max(0.0).sqrt();
    let roots = if root / coefficient <= axial_tolerance {
        vec![axial_center / coefficient]
    } else {
        vec![
            (axial_center - root) / coefficient,
            (axial_center + root) / coefficient,
        ]
    };
    let mut events = Vec::new();
    for axial in roots {
        if axial < -axial_tolerance || axial > height + axial_tolerance {
            continue;
        }
        let axial = axial.clamp(0.0, height);
        let circle_radius = slope * axial;
        // Rhino's surface/surface API omits a contact at the singular apex.
        if circle_radius <= radial_tolerance {
            continue;
        }
        let center = cone_frame.point_at([0.0, 0.0, signed_height.signum() * axial])?;
        let circle = Circle3::try_from_frame(
            center,
            circle_radius,
            cone_frame.x_axis(),
            cone_frame.z_axis(),
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
        NurbsSurface::try_sphere(frame().with_origin(center), radius).unwrap()
    }

    #[test]
    fn coaxial_sphere_cone_sections_are_exact_finite_circles_in_both_orders() {
        let cone = NurbsSurface::try_cone(frame(), 3.0, 4.0).unwrap();
        let sphere = sphere(point(0.0, 0.0, 2.0), 1.5);
        for (first, second) in [(&sphere, &cone), (&cone, &sphere)] {
            let events =
                surface_surface_intersection_events(first, second, Tolerance::DEFAULT).unwrap();
            assert_eq!(events.len(), 2);
            for (event, expected) in events.iter().zip([(0.56, 0.42), (2.0, 1.5)]) {
                let SurfaceSurfaceIntersectionEvent::Curve(circle) = event else {
                    panic!("expected exact circles, got {events:#?}")
                };
                assert_eq!(circle.degree(), 2);
                assert!(circle.is_closed().unwrap());
                let domain = circle.domain();
                for fraction in [0.0, 0.125, 0.33, 0.75] {
                    let sample = circle
                        .evaluate(*domain.start() + fraction * (*domain.end() - *domain.start()))
                        .unwrap();
                    assert!((sample.z() - expected.0).abs() < 1e-9);
                    assert!((sample.x().hypot(sample.y()) - expected.1).abs() < 1e-9);
                    assert!((sample.distance_to(point(0.0, 0.0, 2.0)).unwrap() - 1.5).abs() < 1e-9);
                }
            }
        }
    }

    #[test]
    fn coaxial_sphere_cone_handles_tangent_rim_apex_and_disjoint_cases() {
        let cone = NurbsSurface::try_cone(frame(), 3.0, 4.0).unwrap();
        for (center, radius, expected_heights) in [
            (point(0.0, 0.0, 0.0), 2.0, vec![1.6]),
            (point(0.0, 0.0, 5.0), 3.0, vec![3.2]),
            (point(0.0, 0.0, 4.0), 3.0, vec![1.12, 4.0]),
        ] {
            let events = surface_surface_intersection_events(
                &sphere(center, radius),
                &cone,
                Tolerance::DEFAULT,
            )
            .unwrap();
            assert_eq!(events.len(), expected_heights.len());
            for (event, expected_height) in events.iter().zip(expected_heights) {
                let SurfaceSurfaceIntersectionEvent::Curve(circle) = event else {
                    panic!("expected a circular section, got {events:#?}")
                };
                assert!(
                    (circle.evaluate(*circle.domain().start()).unwrap().z() - expected_height)
                        .abs()
                        < 1e-9
                );
            }
        }
        let near_tangent = sphere(point(0.0, 0.0, 5.0), 3.0 + 1.0e-9);
        let events =
            surface_surface_intersection_events(&near_tangent, &cone, Tolerance::DEFAULT).unwrap();
        assert_eq!(events.len(), 2);
        for center in [point(0.0, 0.0, -2.0), point(0.0, 0.0, 10.0)] {
            let radius = if center.z() < 0.0 { 2.0 } else { 1.0 };
            assert!(
                surface_surface_intersection_events(
                    &sphere(center, radius),
                    &cone,
                    Tolerance::DEFAULT
                )
                .unwrap()
                .is_empty()
            );
        }
    }

    #[test]
    fn noncoaxial_sphere_cone_external_tangency_is_one_point() {
        let cone = NurbsSurface::try_cone(frame(), 3.0, 4.0).unwrap();
        let tangent = sphere(point(0.5, 0.0, 2.0), 0.8);
        let expected = point(1.14, 0.0, 1.52);
        for (first, second) in [(&tangent, &cone), (&cone, &tangent)] {
            let events =
                surface_surface_intersection_events(first, second, Tolerance::DEFAULT).unwrap();
            let [SurfaceSurfaceIntersectionEvent::Point(contact)] = events.as_slice() else {
                panic!("external sphere/cone tangency must create one point, got {events:#?}")
            };
            assert!(contact.distance_to(expected).unwrap() < 5e-9);
            assert!((contact.distance_to(point(0.5, 0.0, 2.0)).unwrap() - 0.8).abs() < 5e-9);
        }
        let shortened = NurbsSurface::try_cone(frame(), 0.75, 1.0).unwrap();
        assert!(
            surface_surface_intersection_events(&tangent, &shortened, Tolerance::DEFAULT)
                .unwrap()
                .is_empty()
        );
        assert!(
            surface_surface_intersection_events(
                &sphere(point(0.5, 0.0, 2.0), 0.7),
                &cone,
                Tolerance::DEFAULT,
            )
            .unwrap()
            .is_empty()
        );
        assert_eq!(
            surface_surface_intersection_events(
                &sphere(point(0.5, 0.0, 2.0), 0.9),
                &cone,
                Tolerance::DEFAULT,
            )
            .unwrap()
            .len(),
            2
        );
    }

    #[test]
    fn noncoaxial_sphere_cone_tangency_handles_negative_and_distant_rotated_frames() {
        let negative = NurbsSurface::try_cone(frame(), 3.0, -4.0).unwrap();
        let tangent = sphere(point(0.5, 0.0, -2.0), 0.8);
        let events =
            surface_surface_intersection_events(&tangent, &negative, Tolerance::DEFAULT).unwrap();
        let [SurfaceSurfaceIntersectionEvent::Point(contact)] = events.as_slice() else {
            panic!("negative cone must retain the tangent point")
        };
        assert!(contact.distance_to(point(1.14, 0.0, -1.52)).unwrap() < 5e-9);

        let rotated = Frame3::try_from_normal(
            point(1.0e8, -1.0e8, 1.0e8),
            Vector3::try_new(1.0, 2.0, 3.0).unwrap(),
            Tolerance::DEFAULT,
        )
        .unwrap();
        let center = rotated.point_at([0.3, 0.4, 2.0]).unwrap();
        let cone = NurbsSurface::try_cone(rotated, 3.0, 4.0).unwrap();
        let sphere = NurbsSurface::try_sphere(rotated.with_origin(center), 0.8).unwrap();
        for (first, second) in [(&sphere, &cone), (&cone, &sphere)] {
            let events =
                surface_surface_intersection_events(first, second, Tolerance::DEFAULT).unwrap();
            let [SurfaceSurfaceIntersectionEvent::Point(contact)] = events.as_slice() else {
                panic!("rotated distant tangency must produce one point, got {events:#?}")
            };
            let local = rotated.coordinates_of(*contact).unwrap();
            assert!((local[0] - 0.684).abs() < 4e-7);
            assert!((local[1] - 0.912).abs() < 4e-7);
            assert!((local[2] - 1.52).abs() < 4e-7);
            assert!((contact.distance_to(center).unwrap() - 0.8).abs() < 4e-7);
        }
        let secant = NurbsSurface::try_sphere(rotated.with_origin(center), 0.81).unwrap();
        assert_eq!(
            surface_surface_intersection_events(&secant, &cone, Tolerance::DEFAULT)
                .unwrap()
                .len(),
            2
        );
    }

    #[test]
    fn coaxial_sphere_cone_respects_negative_height_and_rotated_large_coordinates() {
        let negative_cone = NurbsSurface::try_cone(frame(), 3.0, -4.0).unwrap();
        let negative_sphere = sphere(point(0.0, 0.0, -2.0), 1.5);
        let events = surface_surface_intersection_events(
            &negative_sphere,
            &negative_cone,
            Tolerance::DEFAULT,
        )
        .unwrap();
        assert_eq!(events.len(), 2);
        for (event, expected_z) in events.iter().zip([-0.56, -2.0]) {
            let SurfaceSurfaceIntersectionEvent::Curve(circle) = event else {
                panic!("expected exact circular sections")
            };
            assert!(
                (circle.evaluate(*circle.domain().start()).unwrap().z() - expected_z).abs() < 1e-9
            );
        }

        let rotated = Frame3::try_from_normal(
            point(1.0e8, -1.0e8, 1.0e8),
            Vector3::try_new(1.0, 2.0, 3.0).unwrap(),
            Tolerance::DEFAULT,
        )
        .unwrap();
        let cone = NurbsSurface::try_cone(rotated, 3.0, 4.0).unwrap();
        let sphere = NurbsSurface::try_sphere(
            rotated.with_origin(rotated.point_at([0.0, 0.0, 2.0]).unwrap()),
            1.5,
        )
        .unwrap();
        assert!(cone.canonical_cone(Tolerance::DEFAULT).unwrap().is_some());
        assert!(
            sphere
                .canonical_sphere(Tolerance::DEFAULT)
                .unwrap()
                .is_some()
        );
        let events =
            surface_surface_intersection_events(&sphere, &cone, Tolerance::DEFAULT).unwrap();
        assert_eq!(events.len(), 2);
        for (event, expected_height) in events.iter().zip([0.56, 2.0]) {
            let SurfaceSurfaceIntersectionEvent::Curve(circle) = event else {
                panic!("expected exact circular sections")
            };
            let sample = circle.evaluate(*circle.domain().start()).unwrap();
            assert!((rotated.coordinates_of(sample).unwrap()[2] - expected_height).abs() < 1e-7);
        }
    }
}
