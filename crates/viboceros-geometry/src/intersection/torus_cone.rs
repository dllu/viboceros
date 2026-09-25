//! Exact circles where a finite coaxial cone wall cuts a canonical ring torus.

use super::SurfaceSurfaceIntersectionEvent;
use crate::{Circle3, Frame3, GeometryError, Real, Tolerance};

pub(super) fn intersect(
    (torus_frame, major_radius, minor_radius): (Frame3, Real, Real),
    (cone_frame, cone_radius, cone_height): (Frame3, Real, Real),
    tolerance: Tolerance,
) -> Result<Vec<SurfaceSurfaceIntersectionEvent>, GeometryError> {
    let coordinate_scale = torus_frame
        .origin()
        .to_array()
        .into_iter()
        .chain(cone_frame.origin().to_array())
        .map(Real::abs)
        .fold(0.0, Real::max);
    let shape_scale = (major_radius + minor_radius)
        .max(cone_radius)
        .max(cone_height.abs());
    let spatial_tolerance = tolerance
        .absolute()
        .max(tolerance.relative() * shape_scale)
        .max(8.0 * Real::EPSILON * coordinate_scale);
    let torus_axis = torus_frame.z_axis().as_vector();
    let cone_axis = cone_frame.z_axis().as_vector();
    if torus_axis.cross(cone_axis)?.length()? * shape_scale > spatial_tolerance {
        return Err(GeometryError::UnsupportedSurfaceSurfaceIntersection {
            context: "nonparallel torus/cone axes",
        });
    }
    let [offset_x, offset_y, torus_height] = cone_frame.coordinates_of(torus_frame.origin())?;
    if offset_x.hypot(offset_y) > spatial_tolerance {
        return Err(GeometryError::UnsupportedSurfaceSurfaceIntersection {
            context: "noncoaxial torus/cone walls",
        });
    }

    // In the cone's (radial distance, height) plane its wall is rho = slope*z.
    // Intersect this line with the torus's meridian circle, then clip to the
    // cone's signed axial interval.
    let slope = cone_radius / cone_height;
    let direction_squared = slope.mul_add(slope, 1.0);
    let signed_offset = slope.mul_add(torus_height, -major_radius);
    let meridian_distance = signed_offset.abs() / direction_squared.sqrt();
    if meridian_distance > minor_radius + spatial_tolerance {
        return Ok(Vec::new());
    }
    let middle_height = slope.mul_add(major_radius, torus_height) / direction_squared;
    let half_height = ((minor_radius - meridian_distance) * (minor_radius + meridian_distance)
        / direction_squared)
        .max(0.0)
        .sqrt();
    let heights: &[Real] = if half_height <= spatial_tolerance {
        &[middle_height]
    } else {
        &[middle_height + half_height, middle_height - half_height]
    };
    let axial_low = cone_height.min(0.0);
    let axial_high = cone_height.max(0.0);
    let mut events = Vec::new();
    for &height in heights {
        if height < axial_low - spatial_tolerance || height > axial_high + spatial_tolerance {
            continue;
        }
        let radius = slope * height;
        if radius <= spatial_tolerance {
            continue;
        }
        let center = cone_frame.point_at([0.0, 0.0, height])?;
        let circle = Circle3::try_from_frame(
            center,
            radius,
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
    use crate::{NurbsSurface, Point3, Vector3, surface_surface_intersection_events};

    fn point(x: Real, y: Real, z: Real) -> Point3 {
        Point3::try_new(x, y, z).unwrap()
    }

    fn frame(center: Point3, normal: Vector3) -> Frame3 {
        Frame3::try_from_normal(center, normal, Tolerance::DEFAULT).unwrap()
    }

    fn vertical_frame(height: Real) -> Frame3 {
        frame(
            point(0.0, 0.0, height),
            Vector3::try_new(0.0, 0.0, 1.0).unwrap(),
        )
    }

    #[test]
    fn coaxial_torus_cone_intersects_in_two_exact_circles_in_both_orders() {
        let cone = NurbsSurface::try_cone(vertical_frame(0.0), 6.0, 8.0).unwrap();
        let torus = NurbsSurface::try_torus(vertical_frame(4.0), 3.0, 1.0).unwrap();
        for (left, right) in [(&torus, &cone), (&cone, &torus)] {
            let events =
                surface_surface_intersection_events(left, right, Tolerance::DEFAULT).unwrap();
            assert_eq!(events.len(), 2);
            for event in events {
                let SurfaceSurfaceIntersectionEvent::Curve(circle) = event else {
                    panic!("coaxial torus/cone section should be a circle")
                };
                assert_eq!(circle.degree(), 2);
                assert!(circle.is_closed().unwrap());
                for index in 0..=16 {
                    let domain = circle.domain();
                    let parameter = *domain.start()
                        + (*domain.end() - *domain.start()) * (index as Real / 16.0);
                    let location = circle.evaluate(parameter).unwrap();
                    let radial = location.x().hypot(location.y());
                    assert!(((radial - 3.0).hypot(location.z() - 4.0) - 1.0).abs() < 5e-9);
                    assert!((radial - 0.75 * location.z()).abs() < 5e-9);
                }
            }
        }
    }

    #[test]
    fn coaxial_torus_cone_handles_tangency_rims_and_negative_height() {
        let cone = NurbsSurface::try_cone(vertical_frame(0.0), 6.0, 8.0).unwrap();
        let tangent_torus = NurbsSurface::try_torus(vertical_frame(3.4), 3.8, 1.0).unwrap();
        assert_eq!(
            surface_surface_intersection_events(&tangent_torus, &cone, Tolerance::DEFAULT)
                .unwrap()
                .len(),
            1
        );
        let short_cone = NurbsSurface::try_cone(vertical_frame(0.0), 3.0, 4.0).unwrap();
        let torus = NurbsSurface::try_torus(vertical_frame(4.0), 3.0, 1.0).unwrap();
        assert_eq!(
            surface_surface_intersection_events(&torus, &short_cone, Tolerance::DEFAULT)
                .unwrap()
                .len(),
            1
        );
        let negative_cone = NurbsSurface::try_cone(vertical_frame(0.0), 6.0, -8.0).unwrap();
        let negative_torus = NurbsSurface::try_torus(vertical_frame(-4.0), 3.0, 1.0).unwrap();
        assert_eq!(
            surface_surface_intersection_events(
                &negative_torus,
                &negative_cone,
                Tolerance::DEFAULT
            )
            .unwrap()
            .len(),
            2
        );
        let distant_torus = NurbsSurface::try_torus(vertical_frame(4.0), 6.0, 1.0).unwrap();
        assert!(
            surface_surface_intersection_events(&distant_torus, &cone, Tolerance::DEFAULT)
                .unwrap()
                .is_empty()
        );
    }

    #[test]
    fn coaxial_torus_cone_handles_distant_rotated_frames() {
        let cone_frame = frame(
            point(1.0e8, -1.0e8, 1.0e8),
            Vector3::try_new(1.0, 2.0, 3.0).unwrap(),
        );
        let torus_frame = frame(
            cone_frame.point_at([0.0, 0.0, 4.0]).unwrap(),
            cone_frame.z_axis().as_vector(),
        );
        let cone = NurbsSurface::try_cone(cone_frame, 6.0, 8.0).unwrap();
        let torus = NurbsSurface::try_torus(torus_frame, 3.0, 1.0).unwrap();
        let events =
            surface_surface_intersection_events(&torus, &cone, Tolerance::DEFAULT).unwrap();
        assert_eq!(events.len(), 2);
        for event in events {
            let SurfaceSurfaceIntersectionEvent::Curve(circle) = event else {
                panic!("rotated coaxial torus/cone section should be a circle")
            };
            for index in 0..=16 {
                let domain = circle.domain();
                let parameter =
                    *domain.start() + (*domain.end() - *domain.start()) * (index as Real / 16.0);
                let local = cone_frame
                    .coordinates_of(circle.evaluate(parameter).unwrap())
                    .unwrap();
                let radial = local[0].hypot(local[1]);
                assert!(((radial - 3.0).hypot(local[2] - 4.0) - 1.0).abs() < 4e-7);
                assert!((radial - 0.75 * local[2]).abs() < 4e-7);
            }
        }
    }
}
