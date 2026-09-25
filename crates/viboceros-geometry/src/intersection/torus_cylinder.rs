//! Exact circles where a finite coaxial cylinder cuts a canonical ring torus.

use super::SurfaceSurfaceIntersectionEvent;
use crate::{Circle3, Frame3, GeometryError, Real, Tolerance};

pub(super) fn intersect(
    (torus_frame, major_radius, minor_radius): (Frame3, Real, Real),
    (cylinder_frame, cylinder_radius, cylinder_height): (Frame3, Real, Real),
    tolerance: Tolerance,
) -> Result<Vec<SurfaceSurfaceIntersectionEvent>, GeometryError> {
    let coordinate_scale = torus_frame
        .origin()
        .to_array()
        .into_iter()
        .chain(cylinder_frame.origin().to_array())
        .map(Real::abs)
        .fold(0.0, Real::max);
    let coordinate_roundoff = 8.0 * Real::EPSILON * coordinate_scale;
    let spatial_tolerance = tolerance
        .absolute()
        .max(
            tolerance.relative()
                * (major_radius + minor_radius)
                    .max(cylinder_radius)
                    .max(cylinder_height),
        )
        .max(coordinate_roundoff);
    let torus_axis = torus_frame.z_axis().as_vector();
    let cylinder_axis = cylinder_frame.z_axis().as_vector();
    let axis_drift = torus_axis.cross(cylinder_axis)?.length()?
        * (major_radius + minor_radius).max(cylinder_height);
    if axis_drift > spatial_tolerance {
        return Err(GeometryError::UnsupportedSurfaceSurfaceIntersection {
            context: "nonparallel torus/cylinder axes",
        });
    }
    let [offset_x, offset_y, cylinder_start] =
        torus_frame.coordinates_of(cylinder_frame.origin())?;
    if offset_x.hypot(offset_y) > spatial_tolerance {
        return Err(GeometryError::UnsupportedSurfaceSurfaceIntersection {
            context: "noncoaxial torus/cylinder walls",
        });
    }
    let cylinder_end = torus_axis
        .dot(cylinder_axis)?
        .mul_add(cylinder_height, cylinder_start);
    let axial_low = cylinder_start.min(cylinder_end);
    let axial_high = cylinder_start.max(cylinder_end);
    if axial_low > minor_radius + spatial_tolerance
        || axial_high < -minor_radius - spatial_tolerance
    {
        return Ok(Vec::new());
    }
    let radial_difference = (cylinder_radius - major_radius).abs();
    if radial_difference > minor_radius + spatial_tolerance {
        return Ok(Vec::new());
    }
    let root = ((minor_radius - radial_difference) * (minor_radius + radial_difference))
        .max(0.0)
        .sqrt();
    let axial_sections: &[Real] = if root <= spatial_tolerance {
        &[0.0]
    } else {
        &[root, -root]
    };
    let mut events = Vec::new();
    for &axial in axial_sections {
        if axial < axial_low - spatial_tolerance || axial > axial_high + spatial_tolerance {
            continue;
        }
        let center = torus_frame.point_at([0.0, 0.0, axial])?;
        let circle = Circle3::try_from_frame(
            center,
            cylinder_radius,
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
    use crate::{NurbsSurface, Point3, Vector3, surface_surface_intersection_events};

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

    fn cylinder(radius: Real, low: Real, high: Real) -> NurbsSurface {
        NurbsSurface::try_cylinder(frame(), radius, low, high).unwrap()
    }

    #[test]
    fn coaxial_torus_cylinder_sections_are_exact_circles_in_both_orders() {
        let torus = torus();
        for (radius, heights) in [
            (4.0, vec![1.0, -1.0]),
            (4.6, vec![0.8, -0.8]),
            (5.0, vec![0.0]),
            (3.0, vec![0.0]),
        ] {
            let cylinder = cylinder(radius, -2.0, 2.0);
            for (left, right) in [(&torus, &cylinder), (&cylinder, &torus)] {
                let events =
                    surface_surface_intersection_events(left, right, Tolerance::DEFAULT).unwrap();
                assert_eq!(events.len(), heights.len());
                for (event, height) in events.iter().zip(&heights) {
                    let SurfaceSurfaceIntersectionEvent::Curve(circle) = event else {
                        panic!("coaxial torus/cylinder section should be a circle")
                    };
                    assert_eq!(circle.degree(), 2);
                    assert!(circle.is_closed().unwrap());
                    for index in 0..=16 {
                        let domain = circle.domain();
                        let parameter = *domain.start()
                            + (*domain.end() - *domain.start()) * (index as Real / 16.0);
                        let location = circle.evaluate(parameter).unwrap();
                        assert!((location.x().hypot(location.y()) - radius).abs() < 5e-9);
                        assert!((location.z() - height).abs() < 5e-9);
                        assert!(
                            ((location.x().hypot(location.y()) - 4.0).hypot(location.z()) - 1.0)
                                .abs()
                                < 5e-9
                        );
                    }
                }
            }
        }
    }

    #[test]
    fn coaxial_torus_cylinder_respects_finite_rims_and_no_hits() {
        let torus = torus();
        let upper = cylinder(4.6, 0.8, 2.0);
        let events =
            surface_surface_intersection_events(&torus, &upper, Tolerance::DEFAULT).unwrap();
        let [SurfaceSurfaceIntersectionEvent::Curve(circle)] = events.as_slice() else {
            panic!("upper cylinder rim should retain one circle")
        };
        assert!((circle.evaluate(*circle.domain().start()).unwrap().z() - 0.8).abs() < 5e-9);
        let opposed_frame = Frame3::try_from_normal(
            point(0.0, 0.0, 1.0),
            Vector3::try_new(0.0, 0.0, -1.0).unwrap(),
            Tolerance::DEFAULT,
        )
        .unwrap();
        let opposed = NurbsSurface::try_cylinder(opposed_frame, 4.6, 0.0, 2.0).unwrap();
        assert_eq!(
            surface_surface_intersection_events(&torus, &opposed, Tolerance::DEFAULT)
                .unwrap()
                .len(),
            2
        );
        for other in [cylinder(2.0, -2.0, 2.0), cylinder(4.6, -0.5, 0.5)] {
            assert!(
                surface_surface_intersection_events(&torus, &other, Tolerance::DEFAULT)
                    .unwrap()
                    .is_empty()
            );
        }
    }

    #[test]
    fn coaxial_torus_cylinder_handles_distant_rotated_frames() {
        let rotated = Frame3::try_from_normal(
            point(1.0e8, -1.0e8, 1.0e8),
            Vector3::try_new(1.0, 2.0, 3.0).unwrap(),
            Tolerance::DEFAULT,
        )
        .unwrap();
        let torus = NurbsSurface::try_torus(rotated, 4.0, 1.0).unwrap();
        let cylinder = NurbsSurface::try_cylinder(rotated, 4.6, -2.0, 2.0).unwrap();
        for (left, right) in [(&torus, &cylinder), (&cylinder, &torus)] {
            let events =
                surface_surface_intersection_events(left, right, Tolerance::DEFAULT).unwrap();
            assert_eq!(events.len(), 2);
            for event in events {
                let SurfaceSurfaceIntersectionEvent::Curve(circle) = event else {
                    panic!("rotated coaxial section should be a circle")
                };
                for index in 0..=16 {
                    let domain = circle.domain();
                    let parameter = *domain.start()
                        + (*domain.end() - *domain.start()) * (index as Real / 16.0);
                    let local = rotated
                        .coordinates_of(circle.evaluate(parameter).unwrap())
                        .unwrap();
                    assert!((local[0].hypot(local[1]) - 4.6).abs() < 4e-7);
                    assert!(((local[0].hypot(local[1]) - 4.0).hypot(local[2]) - 1.0).abs() < 4e-7);
                }
            }
        }
    }
}
