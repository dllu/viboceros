//! Finite sections of two canonical cones with a shared axis.

mod parallel_equal_slope;
mod parallel_unequal_slope;

use super::SurfaceSurfaceIntersectionEvent;
use crate::{Circle3, Frame3, GeometryError, NurbsSurface, Real, Tolerance};

pub(super) fn cone_cone_intersection_events(
    first_surface: &NurbsSurface,
    (first_frame, first_radius, first_signed_height): (Frame3, Real, Real),
    (second_frame, second_radius, second_signed_height): (Frame3, Real, Real),
    tolerance: Tolerance,
) -> Result<Vec<SurfaceSurfaceIntersectionEvent>, GeometryError> {
    let first_height = first_signed_height.abs();
    let second_height = second_signed_height.abs();
    let first_axis = first_frame.z_axis().as_vector();
    let second_axis = second_frame.z_axis().as_vector();
    let size = first_radius
        .max(second_radius)
        .max(first_height)
        .max(second_height);
    let coordinate_scale = first_frame
        .origin()
        .to_array()
        .into_iter()
        .chain(second_frame.origin().to_array())
        .map(Real::abs)
        .fold(0.0, Real::max);
    let coordinate_roundoff = 8.0 * Real::EPSILON * coordinate_scale;
    let spatial_tolerance = tolerance
        .absolute()
        .max(tolerance.relative() * size)
        .max(coordinate_roundoff);
    let angular_drift = first_axis.cross(second_axis)?.length()? * first_height.max(second_height);
    if angular_drift > spatial_tolerance {
        return Err(GeometryError::UnsupportedSurfaceSurfaceIntersection {
            context: "nonparallel cone walls",
        });
    }

    let [offset_x, offset_y, offset_z] = first_frame.coordinates_of(second_frame.origin())?;
    let offset = offset_x.hypot(offset_y);
    if offset > spatial_tolerance {
        let first_slope = first_radius / first_height;
        let second_slope = second_radius / second_height;
        if (first_slope - second_slope).abs() * first_height.max(second_height) <= spatial_tolerance
        {
            return parallel_equal_slope::intersect(
                first_surface,
                (first_frame, first_radius, first_signed_height),
                (second_frame, second_radius, second_signed_height),
                (offset_x, offset_y, offset_z),
                tolerance,
                spatial_tolerance,
            );
        }
        return parallel_unequal_slope::intersect(
            (first_frame, first_radius, first_signed_height),
            (second_frame, second_radius, second_signed_height),
            (offset_x, offset_y, offset_z),
            tolerance,
            spatial_tolerance,
        );
    }
    let first_sign = first_signed_height.signum();
    let second_direction =
        first_axis.dot(second_axis)? * first_sign * second_signed_height.signum();
    let second_sign: Real = if second_direction < 0.0 { -1.0 } else { 1.0 };
    let second_apex = first_sign * offset_z;
    let second_base = second_sign.mul_add(second_height, second_apex);
    let low = second_apex.min(second_base).max(0.0);
    let high = second_apex.max(second_base).min(first_height);
    if high < low {
        return Ok(Vec::new());
    }

    let first_slope = first_radius / first_height;
    let second_slope = second_radius / second_height;
    let slope_difference = first_slope - second_sign * second_slope;
    let radius_tolerance = tolerance
        .absolute()
        .max(tolerance.relative() * first_radius.max(second_radius))
        .max(coordinate_roundoff);
    let radius_difference =
        |axial: Real| (first_slope * axial) - second_slope * (second_sign * (axial - second_apex));
    if slope_difference.abs() * (high - low) <= radius_tolerance {
        let middle = 0.5 * (low + high);
        if radius_difference(middle).abs() > radius_tolerance {
            return Ok(Vec::new());
        }
        if high - low > spatial_tolerance {
            return Err(GeometryError::UnsupportedSurfaceSurfaceIntersection {
                context: "coincident cone wall regions",
            });
        }
        return circle_at(
            first_frame,
            first_sign,
            first_slope,
            middle,
            radius_tolerance,
            tolerance,
        );
    }

    let axial =
        second_sign * second_slope * second_apex / (second_sign * second_slope - first_slope);
    if !axial.is_finite() {
        return Err(GeometryError::UnsupportedSurfaceSurfaceIntersection {
            context: "coaxial cone section is ill-conditioned",
        });
    }
    if axial < low - spatial_tolerance || axial > high + spatial_tolerance {
        return Ok(Vec::new());
    }
    let axial = axial.clamp(low, high);
    if radius_difference(axial).abs() > radius_tolerance {
        return Ok(Vec::new());
    }
    circle_at(
        first_frame,
        first_sign,
        first_slope,
        axial,
        radius_tolerance,
        tolerance,
    )
}

fn circle_at(
    frame: Frame3,
    sign: Real,
    slope: Real,
    axial: Real,
    radius_tolerance: Real,
    tolerance: Tolerance,
) -> Result<Vec<SurfaceSurfaceIntersectionEvent>, GeometryError> {
    let radius = slope * axial;
    // Rhino's surface/surface result omits a contact only at a cone apex.
    if radius <= radius_tolerance {
        return Ok(Vec::new());
    }
    let center = frame.point_at([0.0, 0.0, sign * axial])?;
    let circle =
        Circle3::try_from_frame(center, radius, frame.x_axis(), frame.z_axis(), tolerance)?
            .to_nurbs()?;
    Ok(vec![SurfaceSurfaceIntersectionEvent::Curve(circle)])
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{NurbsSurface, Point3, Vector3, surface_surface_intersection_events};

    fn point(x: Real, y: Real, z: Real) -> Point3 {
        Point3::try_new(x, y, z).unwrap()
    }

    fn frame(origin: Point3, normal: Vector3) -> Frame3 {
        Frame3::try_from_normal(origin, normal, Tolerance::DEFAULT).unwrap()
    }

    fn normal(z: Real) -> Vector3 {
        Vector3::try_new(0.0, 0.0, z).unwrap()
    }

    fn cone(frame: Frame3, radius: Real, height: Real) -> NurbsSurface {
        NurbsSurface::try_cone(frame, radius, height).unwrap()
    }

    fn assert_circle_on_cones(
        first: &NurbsSurface,
        second: &NurbsSurface,
        expected_z: Real,
        expected_radius: Real,
    ) {
        for (left, right) in [(first, second), (second, first)] {
            let events =
                surface_surface_intersection_events(left, right, Tolerance::DEFAULT).unwrap();
            let [SurfaceSurfaceIntersectionEvent::Curve(circle)] = events.as_slice() else {
                panic!("expected one exact coaxial circle, got {events:#?}")
            };
            assert_eq!(circle.degree(), 2);
            assert!(circle.is_closed().unwrap());
            for index in 0..=16 {
                let domain = circle.domain();
                let parameter =
                    *domain.start() + (*domain.end() - *domain.start()) * (index as Real / 16.0);
                let location = circle.evaluate(parameter).unwrap();
                assert!((location.z() - expected_z).abs() < 5e-9);
                assert!((location.x().hypot(location.y()) - expected_radius).abs() < 5e-9);
            }
        }
    }

    #[test]
    fn coaxial_cones_with_opposed_or_shared_directions_meet_in_exact_circles() {
        let first = cone(frame(point(0.0, 0.0, 0.0), normal(1.0)), 3.0, 4.0);
        let opposed = cone(frame(point(0.0, 0.0, 4.0), normal(-1.0)), 3.0, 4.0);
        assert_circle_on_cones(&first, &opposed, 2.0, 1.5);

        let shared = cone(frame(point(0.0, 0.0, 1.0), normal(1.0)), 4.5, 3.0);
        assert_circle_on_cones(&first, &shared, 2.0, 1.5);

        let rim = cone(frame(point(0.0, 0.0, 7.0), normal(-1.0)), 3.0, 3.0);
        assert_circle_on_cones(&first, &rim, 4.0, 3.0);
    }

    #[test]
    fn coaxial_cones_handle_apex_only_disjoint_and_coincident_regions() {
        let origin = frame(point(0.0, 0.0, 0.0), normal(1.0));
        let first = cone(origin, 3.0, 4.0);
        let different_slope = cone(origin, 1.5, 4.0);
        assert!(
            surface_surface_intersection_events(&first, &different_slope, Tolerance::DEFAULT)
                .unwrap()
                .is_empty()
        );
        let translated = cone(frame(point(0.0, 0.0, 1.0), normal(1.0)), 3.0, 4.0);
        assert!(
            surface_surface_intersection_events(&first, &translated, Tolerance::DEFAULT)
                .unwrap()
                .is_empty()
        );
        let separated = cone(frame(point(0.0, 0.0, 10.0), normal(-1.0)), 3.0, 4.0);
        assert!(
            surface_surface_intersection_events(&first, &separated, Tolerance::DEFAULT)
                .unwrap()
                .is_empty()
        );
        assert!(matches!(
            surface_surface_intersection_events(&first, &first, Tolerance::DEFAULT),
            Err(GeometryError::UnsupportedSurfaceSurfaceIntersection { .. })
        ));
    }

    #[test]
    fn coaxial_cones_support_negative_height_and_distant_rotated_frames() {
        let first = cone(frame(point(0.0, 0.0, 0.0), normal(1.0)), 3.0, -4.0);
        let opposed = cone(frame(point(0.0, 0.0, -4.0), normal(1.0)), 3.0, 4.0);
        assert_circle_on_cones(&first, &opposed, -2.0, 1.5);

        let rotated = frame(
            point(1.0e8, -1.0e8, 1.0e8),
            Vector3::try_new(1.0, 2.0, 3.0).unwrap(),
        );
        let second_origin = rotated.point_at([0.0, 0.0, 4.0]).unwrap();
        let second_normal = rotated.z_axis().as_vector().scaled(-1.0).unwrap();
        let first = cone(rotated, 3.0, 4.0);
        let second = cone(frame(second_origin, second_normal), 3.0, 4.0);
        for (left, right) in [(&first, &second), (&second, &first)] {
            let events =
                surface_surface_intersection_events(left, right, Tolerance::DEFAULT).unwrap();
            let [SurfaceSurfaceIntersectionEvent::Curve(circle)] = events.as_slice() else {
                panic!("rotated coaxial cones should meet in a circle")
            };
            for index in 0..=16 {
                let domain = circle.domain();
                let parameter =
                    *domain.start() + (*domain.end() - *domain.start()) * (index as Real / 16.0);
                let local = rotated
                    .coordinates_of(circle.evaluate(parameter).unwrap())
                    .unwrap();
                assert!((local[2] - 2.0).abs() < 4e-7);
                assert!((local[0].hypot(local[1]) - 1.5).abs() < 4e-7);
            }
        }
    }
}
