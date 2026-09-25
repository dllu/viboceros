//! Parallel offset cones of equal slope intersect in an exact plane section.

use super::super::{SurfaceSurfaceIntersectionEvent, cone_plane};
use crate::{Frame3, GeometryError, NurbsSurface, Real, Tolerance};

pub(super) fn intersect(
    first_surface: &NurbsSurface,
    (first_frame, first_radius, first_signed_height): (Frame3, Real, Real),
    (second_frame, second_radius, second_signed_height): (Frame3, Real, Real),
    (offset_x, offset_y, offset_z): (Real, Real, Real),
    tolerance: Tolerance,
    spatial_tolerance: Real,
) -> Result<Vec<SurfaceSurfaceIntersectionEvent>, GeometryError> {
    let first_height = first_signed_height.abs();
    let second_height = second_signed_height.abs();
    let first_slope = first_radius / first_height;
    let second_slope = second_radius / second_height;
    let first_sign = first_signed_height.signum();
    let second_direction = first_frame
        .z_axis()
        .as_vector()
        .dot(second_frame.z_axis().as_vector())?
        * first_sign
        * second_signed_height.signum();
    let second_sign: Real = if second_direction < 0.0 { -1.0 } else { 1.0 };
    let second_apex = first_sign * offset_z;
    let second_base = second_sign.mul_add(second_height, second_apex);
    let low = second_apex.min(second_base).max(0.0);
    let high = second_apex.max(second_base).min(first_height);
    if high < low {
        return Ok(Vec::new());
    }
    let offset = offset_x.hypot(offset_y);
    let direction = [offset_x / offset, offset_y / offset];
    let transverse = [-direction[1], direction[0]];
    let second_low_radius = second_slope * second_sign * (low - second_apex);
    let second_high_radius = second_slope * second_sign * (high - second_apex);
    let second_max_radius = second_low_radius.max(second_high_radius);
    if offset > first_slope * high + second_max_radius + spatial_tolerance {
        return Ok(Vec::new());
    }

    if high - low <= spatial_tolerance {
        return isolated_rim_contacts(
            first_frame,
            first_sign,
            direction,
            transverse,
            first_slope * low,
            second_slope * second_sign * (low - second_apex),
            offset,
            low,
            spatial_tolerance,
        );
    }

    // Subtracting the squared wall equations cancels t² when the cone slopes
    // agree. In local coordinates along the apex offset, x = intercept + rate*t.
    let intercept = (offset - first_slope * second_apex.abs())
        * (offset + first_slope * second_apex.abs())
        / (2.0 * offset);
    let rate = first_slope * first_slope * second_apex / offset;
    let cross_bound = first_slope.mul_add(high, spatial_tolerance);
    let point_at = |axial: Real, cross: Real| {
        let along = rate.mul_add(axial, intercept);
        first_frame.point_at([
            direction[0].mul_add(along, transverse[0] * cross),
            direction[1].mul_add(along, transverse[1] * cross),
            first_sign * axial,
        ])
    };
    let patch = NurbsSurface::try_bilinear([
        point_at(low, -cross_bound)?,
        point_at(high, -cross_bound)?,
        point_at(high, cross_bound)?,
        point_at(low, cross_bound)?,
    ])?;
    let plane =
        patch
            .plane(tolerance)?
            .ok_or(GeometryError::UnsupportedSurfaceSurfaceIntersection {
                context: "parallel cone section plane is ill-conditioned",
            })?;
    cone_plane::cone_planar_surface_intersection_events(
        first_surface,
        first_frame,
        first_radius,
        first_signed_height,
        &patch,
        plane,
        tolerance,
    )
}

#[allow(clippy::too_many_arguments)]
fn isolated_rim_contacts(
    frame: Frame3,
    axial_sign: Real,
    direction: [Real; 2],
    transverse: [Real; 2],
    first_radius: Real,
    second_radius: Real,
    offset: Real,
    axial: Real,
    tolerance: Real,
) -> Result<Vec<SurfaceSurfaceIntersectionEvent>, GeometryError> {
    if first_radius <= tolerance || second_radius <= tolerance {
        return Ok(Vec::new());
    }
    if offset > first_radius + second_radius + tolerance
        || offset + first_radius < second_radius - tolerance
        || offset + second_radius < first_radius - tolerance
    {
        return Ok(Vec::new());
    }
    let along = ((first_radius - second_radius) * (first_radius + second_radius) + offset * offset)
        / (2.0 * offset);
    let cross_squared = (first_radius - along) * (first_radius + along);
    if cross_squared < -tolerance * first_radius.max(second_radius) {
        return Ok(Vec::new());
    }
    let cross = cross_squared.max(0.0).sqrt();
    let sides: &[Real] = if cross <= tolerance {
        &[1.0]
    } else {
        &[1.0, -1.0]
    };
    sides
        .iter()
        .map(|side| {
            Ok(SurfaceSurfaceIntersectionEvent::Point(frame.point_at([
                direction[0].mul_add(along, side * transverse[0] * cross),
                direction[1].mul_add(along, side * transverse[1] * cross),
                axial_sign * axial,
            ])?))
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Point3, Vector3, surface_surface_intersection_events};

    fn point(x: Real, y: Real, z: Real) -> Point3 {
        Point3::try_new(x, y, z).unwrap()
    }

    fn frame(origin: Point3, normal_z: Real) -> Frame3 {
        Frame3::try_from_normal(
            origin,
            Vector3::try_new(0.0, 0.0, normal_z).unwrap(),
            Tolerance::DEFAULT,
        )
        .unwrap()
    }

    fn cone(frame: Frame3, radius: Real, height: Real) -> NurbsSurface {
        NurbsSurface::try_cone(frame, radius, height).unwrap()
    }

    fn assert_on_both_walls(location: Point3, first: Frame3, second: Frame3, second_height: Real) {
        let first_local = first.coordinates_of(location).unwrap();
        let second_local = second.coordinates_of(location).unwrap();
        assert!((first_local[0].hypot(first_local[1]) - 0.75 * first_local[2]).abs() < 5e-8);
        assert!(
            (second_local[0].hypot(second_local[1])
                - 0.75 * second_local[2] * second_height.signum())
            .abs()
                < 5e-8
        );
        assert!((-5e-8..=4.0 + 5e-8).contains(&first_local[2]));
        assert!(
            (-5e-8..=second_height.abs() + 5e-8)
                .contains(&(second_local[2] * second_height.signum()))
        );
    }

    #[test]
    fn parallel_equal_slope_cones_follow_exact_plane_sections_in_both_orders() {
        let first_frame = frame(point(0.0, 0.0, 0.0), 1.0);
        let first = cone(first_frame, 3.0, 4.0);
        for (second_frame, second_radius, second_height) in [
            (frame(point(1.0, 0.0, 0.0), 1.0), 3.0, 4.0),
            (frame(point(1.0, 0.0, 1.0), 1.0), 1.5, 2.0),
            (frame(point(1.0, 0.0, 4.0), -1.0), 3.0, 4.0),
        ] {
            let second = cone(second_frame, second_radius, second_height);
            for (left, right) in [(&first, &second), (&second, &first)] {
                let events =
                    surface_surface_intersection_events(left, right, Tolerance::DEFAULT).unwrap();
                assert!(!events.is_empty(), "expected an offset cone section");
                assert!(
                    events
                        .iter()
                        .any(|event| matches!(event, SurfaceSurfaceIntersectionEvent::Curve(_)))
                );
                for event in events {
                    let SurfaceSurfaceIntersectionEvent::Curve(curve) = event else {
                        panic!("transverse offset section should be a curve")
                    };
                    for index in 0..=64 {
                        let domain = curve.domain();
                        let parameter = *domain.start()
                            + (*domain.end() - *domain.start()) * (index as Real / 64.0);
                        assert_on_both_walls(
                            curve.evaluate(parameter).unwrap(),
                            first_frame,
                            second_frame,
                            second_height,
                        );
                    }
                }
            }
        }
    }

    #[test]
    fn parallel_equal_slope_cones_keep_rim_points_and_disjoint_cases() {
        let first_frame = frame(point(0.0, 0.0, 0.0), 1.0);
        let first = cone(first_frame, 3.0, 4.0);
        let second_frame = frame(point(1.0, 0.0, 7.0), -1.0);
        let second = cone(second_frame, 2.25, 3.0);
        let events =
            surface_surface_intersection_events(&first, &second, Tolerance::DEFAULT).unwrap();
        assert_eq!(events.len(), 2);
        for event in events {
            let SurfaceSurfaceIntersectionEvent::Point(location) = event else {
                panic!("two base circles should meet in isolated points")
            };
            assert_on_both_walls(location, first_frame, second_frame, 3.0);
            assert!((location.z() - 4.0).abs() < 5e-9);
        }

        let distant = cone(frame(point(7.0, 0.0, 0.0), 1.0), 3.0, 4.0);
        assert!(
            surface_surface_intersection_events(&first, &distant, Tolerance::DEFAULT)
                .unwrap()
                .is_empty()
        );
        let tangent = cone(frame(point(6.0, 0.0, 0.0), 1.0), 3.0, 4.0);
        let contacts =
            surface_surface_intersection_events(&first, &tangent, Tolerance::DEFAULT).unwrap();
        let [SurfaceSurfaceIntersectionEvent::Point(contact)] = contacts.as_slice() else {
            panic!("cone bases should touch at one point, got {contacts:#?}")
        };
        assert!(contact.distance_to(point(3.0, 0.0, 4.0)).unwrap() < 5e-9);
    }

    #[test]
    fn parallel_equal_slope_cones_support_negative_and_distant_rotated_frames() {
        let origin = frame(point(0.0, 0.0, 0.0), 1.0);
        let shifted = origin.with_origin(point(1.0, 0.0, 0.0));
        let negative_first = cone(origin, 3.0, -4.0);
        let negative_second = cone(shifted, 3.0, -4.0);
        let negative = surface_surface_intersection_events(
            &negative_first,
            &negative_second,
            Tolerance::DEFAULT,
        )
        .unwrap();
        assert!(!negative.is_empty());
        for event in negative {
            let SurfaceSurfaceIntersectionEvent::Curve(curve) = event else {
                panic!("negative equal-slope cones should meet in curves")
            };
            for index in 0..=32 {
                let domain = curve.domain();
                let parameter =
                    *domain.start() + (*domain.end() - *domain.start()) * (index as Real / 32.0);
                let location = curve.evaluate(parameter).unwrap();
                assert!((location.x().hypot(location.y()) + 0.75 * location.z()).abs() < 5e-8);
                assert!(
                    ((location.x() - 1.0).hypot(location.y()) + 0.75 * location.z()).abs() < 5e-8
                );
            }
        }

        let rotated = Frame3::try_from_normal(
            point(1.0e8, -1.0e8, 1.0e8),
            Vector3::try_new(1.0, 2.0, 3.0).unwrap(),
            Tolerance::DEFAULT,
        )
        .unwrap();
        let other = rotated.with_origin(rotated.point_at([1.0, 0.0, 0.0]).unwrap());
        let first = cone(rotated, 3.0, 4.0);
        let second = cone(other, 3.0, 4.0);
        let events =
            surface_surface_intersection_events(&first, &second, Tolerance::DEFAULT).unwrap();
        assert!(!events.is_empty());
        for event in events {
            let SurfaceSurfaceIntersectionEvent::Curve(curve) = event else {
                panic!("rotated equal-slope cones should meet in curves")
            };
            for index in 0..=32 {
                let domain = curve.domain();
                let parameter =
                    *domain.start() + (*domain.end() - *domain.start()) * (index as Real / 32.0);
                let location = curve.evaluate(parameter).unwrap();
                for local in [
                    rotated.coordinates_of(location).unwrap(),
                    other.coordinates_of(location).unwrap(),
                ] {
                    assert!((local[0].hypot(local[1]) - 0.75 * local[2]).abs() < 4e-7);
                    assert!((-4e-7..=4.0 + 4e-7).contains(&local[2]));
                }
            }
        }
    }
}
