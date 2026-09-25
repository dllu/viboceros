//! Coaxial circles, matching-tube parallel offsets, and equal centered crossed tori.

mod crossed_equal;
mod parallel_equal;
mod parallel_equal_minor;

use super::SurfaceSurfaceIntersectionEvent;
use crate::{Circle3, Frame3, GeometryError, Real, Tolerance};

pub(super) fn intersect(
    (first_frame, first_major, first_minor): (Frame3, Real, Real),
    (second_frame, second_major, second_minor): (Frame3, Real, Real),
    tolerance: Tolerance,
) -> Result<Vec<SurfaceSurfaceIntersectionEvent>, GeometryError> {
    let coordinate_scale = first_frame
        .origin()
        .to_array()
        .into_iter()
        .chain(second_frame.origin().to_array())
        .map(Real::abs)
        .fold(0.0, Real::max);
    let shape_scale = (first_major + first_minor).max(second_major + second_minor);
    let spatial_tolerance = tolerance
        .absolute()
        .max(tolerance.relative() * shape_scale)
        .max(8.0 * Real::EPSILON * coordinate_scale);
    let first_axis = first_frame.z_axis().as_vector();
    let second_axis = second_frame.z_axis().as_vector();
    if first_axis.cross(second_axis)?.length()? * shape_scale > spatial_tolerance {
        if first_frame.origin().distance_to(second_frame.origin())? <= spatial_tolerance
            && (first_major - second_major).abs() <= spatial_tolerance
            && (first_minor - second_minor).abs() <= spatial_tolerance
        {
            return crossed_equal::intersect(
                (first_frame, first_major, first_minor),
                second_frame,
                tolerance,
            );
        }
        return Err(GeometryError::UnsupportedSurfaceSurfaceIntersection {
            context: "nonparallel torus axes",
        });
    }
    let [offset_x, offset_y, second_height] = first_frame.coordinates_of(second_frame.origin())?;
    if offset_x.hypot(offset_y) > spatial_tolerance {
        if second_height.abs() <= spatial_tolerance
            && (first_minor - second_minor).abs() <= spatial_tolerance
        {
            if (first_major - second_major).abs() <= spatial_tolerance {
                return parallel_equal::intersect(
                    (first_frame, first_major, first_minor),
                    [offset_x, offset_y],
                    tolerance,
                    spatial_tolerance,
                );
            }
            return parallel_equal_minor::intersect(
                (first_frame, first_major, first_minor),
                second_major,
                [offset_x, offset_y],
                tolerance,
                spatial_tolerance,
            );
        }
        return Err(GeometryError::UnsupportedSurfaceSurfaceIntersection {
            context: "noncoaxial tori",
        });
    }

    // Each torus contributes a meridian circle in (radial distance, height).
    let radial_delta = second_major - first_major;
    let center_distance = radial_delta.hypot(second_height);
    if center_distance <= spatial_tolerance {
        if (first_minor - second_minor).abs() <= spatial_tolerance {
            return Err(GeometryError::UnsupportedSurfaceSurfaceIntersection {
                context: "coincident tori",
            });
        }
        return Ok(Vec::new());
    }
    if center_distance > first_minor + second_minor + spatial_tolerance
        || center_distance + first_minor + spatial_tolerance < second_minor
        || center_distance + second_minor + spatial_tolerance < first_minor
    {
        return Ok(Vec::new());
    }
    let along = ((first_minor - second_minor) * (first_minor + second_minor)
        + center_distance * center_distance)
        / (2.0 * center_distance);
    let transverse = ((first_minor - along) * (first_minor + along))
        .max(0.0)
        .sqrt();
    let radial_base = first_major + along * radial_delta / center_distance;
    let height_base = along * second_height / center_distance;
    let radial_step = -transverse * second_height / center_distance;
    let height_step = transverse * radial_delta / center_distance;
    let mut sections = if transverse <= spatial_tolerance {
        vec![(radial_base, height_base)]
    } else {
        vec![
            (radial_base + radial_step, height_base + height_step),
            (radial_base - radial_step, height_base - height_step),
        ]
    };
    sections.sort_by(|left, right| {
        right
            .1
            .total_cmp(&left.1)
            .then_with(|| right.0.total_cmp(&left.0))
    });
    let mut events = Vec::with_capacity(sections.len());
    for (radius, height) in sections {
        let center = first_frame.point_at([0.0, 0.0, height])?;
        let circle = Circle3::try_from_frame(
            center,
            radius,
            first_frame.x_axis(),
            first_frame.z_axis(),
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
    use crate::{NurbsSurface, Point3, Vector3, surface_surface_intersection_events};

    fn point(x: Real, y: Real, z: Real) -> Point3 {
        Point3::try_new(x, y, z).unwrap()
    }

    fn frame(center: Point3, normal: Vector3) -> Frame3 {
        Frame3::try_from_normal(center, normal, Tolerance::DEFAULT).unwrap()
    }

    fn torus(major: Real, minor: Real, height: Real) -> NurbsSurface {
        let frame = frame(
            point(0.0, 0.0, height),
            Vector3::try_new(0.0, 0.0, 1.0).unwrap(),
        );
        NurbsSurface::try_torus(frame, major, minor).unwrap()
    }

    #[test]
    fn coaxial_tori_intersect_in_exact_circles_in_both_orders() {
        let first = torus(4.0, 1.0, 0.0);
        for (major, height, expected) in [
            (5.0, 0.0, 2),
            (4.0, 1.0, 2),
            (6.0, 0.0, 1),
            (4.0, 2.0, 1),
            (7.0, 0.0, 0),
        ] {
            let second = torus(major, 1.0, height);
            for (left, right) in [(&first, &second), (&second, &first)] {
                let events =
                    surface_surface_intersection_events(left, right, Tolerance::DEFAULT).unwrap();
                assert_eq!(events.len(), expected);
                for event in events {
                    let SurfaceSurfaceIntersectionEvent::Curve(circle) = event else {
                        panic!("coaxial tori should intersect in circles")
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
                        assert!(((radial - major).hypot(location.z() - height) - 1.0).abs() < 5e-9);
                    }
                }
            }
        }
    }

    #[test]
    fn opposed_axes_and_coincident_tori() {
        let first = torus(4.0, 1.0, 0.0);
        let opposed_frame = frame(
            point(0.0, 0.0, 1.0),
            Vector3::try_new(0.0, 0.0, -1.0).unwrap(),
        );
        let opposed = NurbsSurface::try_torus(opposed_frame, 4.0, 1.0).unwrap();
        assert_eq!(
            surface_surface_intersection_events(&first, &opposed, Tolerance::DEFAULT)
                .unwrap()
                .len(),
            2
        );
        let coincident = torus(4.0, 1.0, 0.0);
        assert!(matches!(
            surface_surface_intersection_events(&first, &coincident, Tolerance::DEFAULT),
            Err(GeometryError::UnsupportedSurfaceSurfaceIntersection { .. })
        ));
        let offset_frame = frame(
            point(0.1, 0.0, 0.0),
            Vector3::try_new(0.0, 0.0, 1.0).unwrap(),
        );
        let offset = NurbsSurface::try_torus(offset_frame, 4.0, 1.0).unwrap();
        assert_eq!(
            surface_surface_intersection_events(&first, &offset, Tolerance::DEFAULT)
                .unwrap()
                .len(),
            4
        );
    }

    #[test]
    fn separated_tori_with_different_axes_have_no_events() {
        let first = torus(4.0, 1.0, 0.0);
        let second_frame = frame(
            point(20.0, 0.0, 0.0),
            Vector3::try_new(1.0, 0.0, 0.0).unwrap(),
        );
        let second = NurbsSurface::try_torus(second_frame, 4.0, 1.0).unwrap();
        for (left, right) in [(&first, &second), (&second, &first)] {
            assert!(
                surface_surface_intersection_events(left, right, Tolerance::DEFAULT)
                    .unwrap()
                    .is_empty()
            );
        }
    }

    #[test]
    fn coaxial_tori_handle_distant_rotated_frames() {
        let first_frame = frame(
            point(1.0e8, -1.0e8, 1.0e8),
            Vector3::try_new(1.0, 2.0, 3.0).unwrap(),
        );
        let second_frame = frame(
            first_frame.point_at([0.0, 0.0, 1.0]).unwrap(),
            first_frame.z_axis().as_vector(),
        );
        let first = NurbsSurface::try_torus(first_frame, 4.0, 1.0).unwrap();
        let second = NurbsSurface::try_torus(second_frame, 4.0, 1.0).unwrap();
        let events =
            surface_surface_intersection_events(&first, &second, Tolerance::DEFAULT).unwrap();
        assert_eq!(events.len(), 2);
        for event in events {
            let SurfaceSurfaceIntersectionEvent::Curve(circle) = event else {
                panic!("rotated coaxial tori should intersect in circles")
            };
            for index in 0..=16 {
                let domain = circle.domain();
                let parameter =
                    *domain.start() + (*domain.end() - *domain.start()) * (index as Real / 16.0);
                let location = circle.evaluate(parameter).unwrap();
                let local = first_frame.coordinates_of(location).unwrap();
                let radial = local[0].hypot(local[1]);
                assert!(((radial - 4.0).hypot(local[2]) - 1.0).abs() < 4e-7);
                assert!(((radial - 4.0).hypot(local[2] - 1.0) - 1.0).abs() < 4e-7);
            }
        }
    }
}
