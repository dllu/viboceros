//! Exact coaxial circles and fitted parallel offset torus/cylinder sections.

mod offset_parallel;
mod perpendicular_centered;

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
                    .max(cylinder_height.abs()),
        )
        .max(coordinate_roundoff);
    let torus_axis = torus_frame.z_axis().as_vector();
    let cylinder_axis = cylinder_frame.z_axis().as_vector();
    let axis_drift = torus_axis.cross(cylinder_axis)?.length()?
        * (major_radius + minor_radius).max(cylinder_height.abs());
    if axis_drift > spatial_tolerance {
        let [offset_x, offset_y, offset_z] = torus_frame.coordinates_of(cylinder_frame.origin())?;
        let direction_x = cylinder_axis.dot(torus_frame.x_axis().as_vector())?;
        let direction_y = cylinder_axis.dot(torus_frame.y_axis().as_vector())?;
        let transverse = direction_x.mul_add(-offset_y, direction_y * offset_x);
        let centered_perpendicular = torus_axis.dot(cylinder_axis)?.abs()
            * (major_radius + minor_radius).max(cylinder_height.abs())
            <= spatial_tolerance
            && offset_z.abs() <= spatial_tolerance
            && transverse.abs() <= spatial_tolerance;
        if centered_perpendicular {
            let outer_radius = major_radius + minor_radius;
            if cylinder_radius > outer_radius + spatial_tolerance {
                return Ok(Vec::new());
            }
            if (cylinder_radius - outer_radius).abs() <= spatial_tolerance {
                let direction_length = direction_x.hypot(direction_y);
                let direction = [
                    direction_x / direction_length,
                    direction_y / direction_length,
                ];
                let start = offset_x.mul_add(direction[0], offset_y * direction[1]);
                let end = start + cylinder_height * direction_length;
                if start.min(end) > spatial_tolerance || start.max(end) < -spatial_tolerance {
                    return Ok(Vec::new());
                }
                return [1.0, -1.0]
                    .into_iter()
                    .map(|side| {
                        torus_frame
                            .point_at([
                                -side * outer_radius * direction[1],
                                side * outer_radius * direction[0],
                                0.0,
                            ])
                            .map(SurfaceSurfaceIntersectionEvent::Point)
                    })
                    .collect();
            }
        }
        if centered_perpendicular
            && ((cylinder_radius + 4.0 * spatial_tolerance < major_radius - minor_radius
                && ((cylinder_radius - minor_radius).abs() > 4.0 * spatial_tolerance
                    || cylinder_radius == minor_radius))
                || (cylinder_radius > (major_radius - minor_radius) + 4.0 * spatial_tolerance
                    && cylinder_radius + 4.0 * spatial_tolerance < major_radius + minor_radius
                    && cylinder_radius > minor_radius + 4.0 * spatial_tolerance)
                || (cylinder_radius == major_radius - minor_radius
                    && cylinder_radius > minor_radius + 4.0 * spatial_tolerance)
                || (cylinder_radius > major_radius - minor_radius + 4.0 * spatial_tolerance
                    && cylinder_radius + 4.0 * spatial_tolerance < minor_radius)
                || (cylinder_radius == major_radius - minor_radius
                    && cylinder_radius + 4.0 * spatial_tolerance < minor_radius))
        {
            return perpendicular_centered::intersect(
                (torus_frame, major_radius, minor_radius),
                (cylinder_radius, cylinder_height),
                (offset_x, offset_y, direction_x, direction_y),
                spatial_tolerance,
            );
        }
        return Err(GeometryError::UnsupportedSurfaceSurfaceIntersection {
            context: "nonparallel torus/cylinder axes",
        });
    }
    let [offset_x, offset_y, cylinder_start] =
        torus_frame.coordinates_of(cylinder_frame.origin())?;
    if offset_x.hypot(offset_y) > spatial_tolerance {
        return offset_parallel::intersect(
            (torus_frame, major_radius, minor_radius),
            (cylinder_frame, cylinder_radius, cylinder_height),
            [offset_x, offset_y, cylinder_start],
            spatial_tolerance,
        );
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

    fn perpendicular_cylinder(radius: Real, low: Real, high: Real) -> NurbsSurface {
        let axis = Frame3::try_from_normal(
            point(0.0, 0.0, 0.0),
            Vector3::try_new(1.0, 0.0, 0.0).unwrap(),
            Tolerance::DEFAULT,
        )
        .unwrap();
        NurbsSurface::try_cylinder(axis, radius, low, high).unwrap()
    }

    #[test]
    fn centered_perpendicular_narrow_cylinder_has_four_full_sections() {
        let torus = torus();
        let cylinder = perpendicular_cylinder(0.4, -6.0, 6.0);
        for (left, right) in [(&torus, &cylinder), (&cylinder, &torus)] {
            let events =
                surface_surface_intersection_events(left, right, Tolerance::DEFAULT).unwrap();
            assert_eq!(events.len(), 4);
            for event in events {
                let SurfaceSurfaceIntersectionEvent::Curve(curve) = event else {
                    panic!("narrow perpendicular cylinder should give four loops")
                };
                assert_eq!(curve.degree(), 3);
                assert!(curve.is_closed().unwrap());
                let domain = curve.domain();
                for index in 0..=64 {
                    let parameter = *domain.start()
                        + (*domain.end() - *domain.start()) * (index as Real / 64.0);
                    let location = curve.evaluate(parameter).unwrap();
                    let torus_residual =
                        ((location.x().hypot(location.y()) - 4.0).hypot(location.z()) - 1.0).abs();
                    let cylinder_residual = (location.y().hypot(location.z()) - 0.4).abs();
                    assert!(torus_residual < 5e-9, "torus residual {torus_residual}");
                    assert!(
                        cylinder_residual < 5e-9,
                        "cylinder residual {cylinder_residual}"
                    );
                }
            }
        }
    }

    #[test]
    fn centered_perpendicular_radius_regimes_have_four_sections() {
        let torus = torus();
        for radius in [0.999, 1.0, 1.001, 1.5, 2.5, 2.99] {
            let cylinder = perpendicular_cylinder(radius, -6.0, 6.0);
            for (left, right) in [(&torus, &cylinder), (&cylinder, &torus)] {
                let events =
                    surface_surface_intersection_events(left, right, Tolerance::DEFAULT).unwrap();
                assert_eq!(events.len(), 4, "radius {radius}");
                for event in events {
                    let SurfaceSurfaceIntersectionEvent::Curve(curve) = event else {
                        panic!("perpendicular cylinder should give four curves")
                    };
                    assert_eq!(curve.degree(), 3);
                    assert!(curve.is_closed().unwrap());
                    let domain = curve.domain();
                    for index in 0..=64 {
                        let parameter = *domain.start()
                            + (*domain.end() - *domain.start()) * index as Real / 64.0;
                        let location = curve.evaluate(parameter).unwrap();
                        assert!(
                            ((location.x().hypot(location.y()) - 4.0).hypot(location.z()) - 1.0)
                                .abs()
                                < 5e-9
                        );
                        assert!((location.y().hypot(location.z()) - radius).abs() < 5e-9);
                    }
                }
            }
        }
    }

    #[test]
    fn centered_perpendicular_wide_cylinder_clips_at_both_rims() {
        let events = surface_surface_intersection_events(
            &torus(),
            &perpendicular_cylinder(2.0, 3.0, 4.0),
            Tolerance::DEFAULT,
        )
        .unwrap();
        assert_eq!(events.len(), 4);
        for event in events {
            let SurfaceSurfaceIntersectionEvent::Curve(curve) = event else {
                panic!("finite wide cylinder should give four arcs")
            };
            assert!(!curve.is_closed().unwrap());
            let domain = curve.domain();
            for index in 0..=32 {
                let parameter =
                    *domain.start() + (*domain.end() - *domain.start()) * index as Real / 32.0;
                let location = curve.evaluate(parameter).unwrap();
                assert!(location.x() >= 3.0 - 5e-9 && location.x() <= 4.0 + 5e-9);
                assert!(
                    ((location.x().hypot(location.y()) - 4.0).hypot(location.z()) - 1.0).abs()
                        < 5e-9
                );
                assert!((location.y().hypot(location.z()) - 2.0).abs() < 5e-9);
            }
        }
    }

    #[test]
    fn centered_perpendicular_turning_sections_form_two_loops() {
        let torus = torus();
        for radius in [3.0001, 3.1, 4.0, 4.9, 4.9999] {
            let cylinder = perpendicular_cylinder(radius, -6.0, 6.0);
            for (left, right) in [(&torus, &cylinder), (&cylinder, &torus)] {
                let events =
                    surface_surface_intersection_events(left, right, Tolerance::DEFAULT).unwrap();
                assert_eq!(events.len(), 2, "radius {radius}");
                for event in events {
                    let SurfaceSurfaceIntersectionEvent::Curve(curve) = event else {
                        panic!("perpendicular turning section should be a loop")
                    };
                    assert_eq!(curve.degree(), 3);
                    assert!(curve.is_closed().unwrap());
                    let domain = curve.domain();
                    for index in 0..=64 {
                        let parameter = *domain.start()
                            + (*domain.end() - *domain.start()) * index as Real / 64.0;
                        let location = curve.evaluate(parameter).unwrap();
                        assert!(
                            ((location.x().hypot(location.y()) - 4.0).hypot(location.z()) - 1.0)
                                .abs()
                                < 5e-9
                        );
                        assert!((location.y().hypot(location.z()) - radius).abs() < 5e-9);
                    }
                }
            }
        }
        let reversed_axis = Frame3::try_from_normal(
            point(6.0, 0.0, 0.0),
            Vector3::try_new(-1.0, 0.0, 0.0).unwrap(),
            Tolerance::DEFAULT,
        )
        .unwrap();
        let reversed = NurbsSurface::try_cylinder(reversed_axis, 4.0, 0.0, 12.0).unwrap();
        let reversed_events =
            surface_surface_intersection_events(&torus, &reversed, Tolerance::DEFAULT).unwrap();
        assert_eq!(reversed_events.len(), 2);
        assert!(reversed_events.iter().all(|event| matches!(
            event,
            SurfaceSurfaceIntersectionEvent::Curve(curve) if curve.is_closed().unwrap()
        )));
    }

    #[test]
    fn centered_perpendicular_inner_rim_has_two_crossing_loops() {
        let torus = torus();
        let cylinder = perpendicular_cylinder(3.0, -6.0, 6.0);
        for (left, right) in [(&torus, &cylinder), (&cylinder, &torus)] {
            let events =
                surface_surface_intersection_events(left, right, Tolerance::DEFAULT).unwrap();
            assert_eq!(events.len(), 2);
            for event in events {
                let SurfaceSurfaceIntersectionEvent::Curve(curve) = event else {
                    panic!("critical inner rim should give crossing loops")
                };
                assert_eq!(curve.degree(), 3);
                assert!(curve.is_closed().unwrap());
                let domain = curve.domain();
                for index in 0..=128 {
                    let parameter =
                        *domain.start() + (*domain.end() - *domain.start()) * index as Real / 128.0;
                    let location = curve.evaluate(parameter).unwrap();
                    assert!(
                        ((location.x().hypot(location.y()) - 4.0).hypot(location.z()) - 1.0).abs()
                            < 5e-9
                    );
                    assert!((location.y().hypot(location.z()) - 3.0).abs() < 5e-9);
                }
                let first_crossing = curve
                    .evaluate(*domain.start() + 0.25 * (*domain.end() - *domain.start()))
                    .unwrap();
                let second_crossing = curve
                    .evaluate(*domain.start() + 0.75 * (*domain.end() - *domain.start()))
                    .unwrap();
                assert!(first_crossing.distance_to(second_crossing).unwrap() < 5e-9);
            }
        }
        let arcs = surface_surface_intersection_events(
            &torus,
            &perpendicular_cylinder(3.0, 1.0, 2.0),
            Tolerance::DEFAULT,
        )
        .unwrap();
        assert_eq!(arcs.len(), 4);
        for event in arcs {
            let SurfaceSurfaceIntersectionEvent::Curve(curve) = event else {
                panic!("critical inner rim clipping should produce arcs")
            };
            assert!(!curve.is_closed().unwrap());
            for index in 0..=32 {
                let domain = curve.domain();
                let parameter =
                    *domain.start() + (*domain.end() - *domain.start()) * index as Real / 32.0;
                let location = curve.evaluate(parameter).unwrap();
                assert!(location.x() >= 1.0 - 5e-9 && location.x() <= 2.0 + 5e-9);
                assert!(
                    ((location.x().hypot(location.y()) - 4.0).hypot(location.z()) - 1.0).abs()
                        < 5e-9
                );
                assert!((location.y().hypot(location.z()) - 3.0).abs() < 5e-9);
            }
        }
    }

    #[test]
    fn centered_perpendicular_turning_sections_clip_to_arcs_and_points() {
        let torus = torus();
        let arcs = surface_surface_intersection_events(
            &torus,
            &perpendicular_cylinder(4.0, 1.0, 2.0),
            Tolerance::DEFAULT,
        )
        .unwrap();
        assert_eq!(arcs.len(), 4);
        for event in arcs {
            let SurfaceSurfaceIntersectionEvent::Curve(curve) = event else {
                panic!("finite turning section should produce arcs")
            };
            assert!(!curve.is_closed().unwrap());
            for index in 0..=32 {
                let domain = curve.domain();
                let parameter =
                    *domain.start() + (*domain.end() - *domain.start()) * index as Real / 32.0;
                let location = curve.evaluate(parameter).unwrap();
                assert!(location.x() >= 1.0 - 5e-9 && location.x() <= 2.0 + 5e-9);
                assert!(
                    ((location.x().hypot(location.y()) - 4.0).hypot(location.z()) - 1.0).abs()
                        < 5e-9
                );
                assert!((location.y().hypot(location.z()) - 4.0).abs() < 5e-9);
            }
        }
        let points = surface_surface_intersection_events(
            &torus,
            &perpendicular_cylinder(4.0, 3.0, 6.0),
            Tolerance::DEFAULT,
        )
        .unwrap();
        assert_eq!(points.len(), 2);
        for event in points {
            let SurfaceSurfaceIntersectionEvent::Point(location) = event else {
                panic!("turning section tangent rim should produce points")
            };
            assert!((location.x() - 3.0).abs() < 5e-9);
            assert!((location.y().abs() - 4.0).abs() < 5e-9);
            assert!(location.z().abs() < 5e-9);
        }
    }

    #[test]
    fn centered_perpendicular_outer_tangency_and_disjoint_cylinders() {
        let torus = torus();
        for (low, high, expected) in [(-6.0, 6.0, 2), (0.0, 6.0, 2), (1.0, 6.0, 0)] {
            let events = surface_surface_intersection_events(
                &torus,
                &perpendicular_cylinder(5.0, low, high),
                Tolerance::DEFAULT,
            )
            .unwrap();
            assert_eq!(events.len(), expected);
            for event in events {
                let SurfaceSurfaceIntersectionEvent::Point(location) = event else {
                    panic!("outer tangency should give points")
                };
                assert!(location.x().abs() < 5e-9);
                assert!((location.y().abs() - 5.0).abs() < 5e-9);
                assert!(location.z().abs() < 5e-9);
            }
        }
        assert!(
            surface_surface_intersection_events(
                &torus,
                &perpendicular_cylinder(6.0, -6.0, 6.0),
                Tolerance::DEFAULT,
            )
            .unwrap()
            .is_empty()
        );
    }

    #[test]
    fn fat_ring_torus_and_perpendicular_cylinder_have_outer_and_inner_loops() {
        let torus = NurbsSurface::try_torus(frame(), 1.5, 1.0).unwrap();
        let cylinder = perpendicular_cylinder(0.75, -3.0, 3.0);
        for (left, right) in [(&torus, &cylinder), (&cylinder, &torus)] {
            let events =
                surface_surface_intersection_events(left, right, Tolerance::DEFAULT).unwrap();
            assert_eq!(events.len(), 4);
            for event in events {
                let SurfaceSurfaceIntersectionEvent::Curve(curve) = event else {
                    panic!("fat ring torus should give four loops")
                };
                assert!(curve.is_closed().unwrap());
                for index in 0..=64 {
                    let domain = curve.domain();
                    let parameter =
                        *domain.start() + (*domain.end() - *domain.start()) * index as Real / 64.0;
                    let location = curve.evaluate(parameter).unwrap();
                    assert!(
                        ((location.x().hypot(location.y()) - 1.5).hypot(location.z()) - 1.0).abs()
                            < 5e-9
                    );
                    assert!((location.y().hypot(location.z()) - 0.75).abs() < 5e-9);
                }
            }
        }
        for radius in [0.5001, 0.9999] {
            let events = surface_surface_intersection_events(
                &torus,
                &perpendicular_cylinder(radius, -3.0, 3.0),
                Tolerance::DEFAULT,
            )
            .unwrap();
            assert_eq!(events.len(), 4, "radius {radius}");
            for event in events {
                let SurfaceSurfaceIntersectionEvent::Curve(curve) = event else {
                    panic!("near-critical fat ring section should be a curve")
                };
                assert!(curve.is_closed().unwrap());
                for index in 0..=64 {
                    let domain = curve.domain();
                    let parameter =
                        *domain.start() + (*domain.end() - *domain.start()) * index as Real / 64.0;
                    let location = curve.evaluate(parameter).unwrap();
                    assert!(
                        ((location.x().hypot(location.y()) - 1.5).hypot(location.z()) - 1.0).abs()
                            < 5e-9
                    );
                    assert!((location.y().hypot(location.z()) - radius).abs() < 5e-9);
                }
            }
        }
        let arcs = surface_surface_intersection_events(
            &torus,
            &perpendicular_cylinder(0.75, 0.2, 0.5),
            Tolerance::DEFAULT,
        )
        .unwrap();
        assert_eq!(arcs.len(), 4);
        for event in arcs {
            let SurfaceSurfaceIntersectionEvent::Curve(curve) = event else {
                panic!("finite cylinder should clip the two inner loops to four arcs")
            };
            assert!(!curve.is_closed().unwrap());
            for index in 0..=32 {
                let domain = curve.domain();
                let parameter =
                    *domain.start() + (*domain.end() - *domain.start()) * index as Real / 32.0;
                let location = curve.evaluate(parameter).unwrap();
                assert!(location.x() >= 0.2 - 5e-9 && location.x() <= 0.5 + 5e-9);
                assert!(
                    ((location.x().hypot(location.y()) - 1.5).hypot(location.z()) - 1.0).abs()
                        < 5e-9
                );
                assert!((location.y().hypot(location.z()) - 0.75).abs() < 5e-9);
            }
        }
    }

    #[test]
    fn fat_ring_inner_rim_cylinder_has_crossing_curves_and_clipped_arcs() {
        let torus = NurbsSurface::try_torus(frame(), 1.5, 1.0).unwrap();
        let cylinder = perpendicular_cylinder(0.5, -3.0, 3.0);
        for (left, right) in [(&torus, &cylinder), (&cylinder, &torus)] {
            let events =
                surface_surface_intersection_events(left, right, Tolerance::DEFAULT).unwrap();
            assert_eq!(events.len(), 4);
            let [
                _,
                _,
                SurfaceSurfaceIntersectionEvent::Curve(inner_positive),
                SurfaceSurfaceIntersectionEvent::Curve(inner_negative),
            ] = events.as_slice()
            else {
                panic!("critical inner branches should be the final two curves")
            };
            for fraction in [0.0, 0.5] {
                let domain = inner_positive.domain();
                let parameter = *domain.start() + (*domain.end() - *domain.start()) * fraction;
                let first = inner_positive.evaluate(parameter).unwrap();
                let second = inner_negative.evaluate(parameter).unwrap();
                assert!(first.distance_to(second).unwrap() < 5e-9);
            }
            for event in events {
                let SurfaceSurfaceIntersectionEvent::Curve(curve) = event else {
                    panic!("inner-rim contact should retain crossing curves")
                };
                assert!(curve.is_closed().unwrap());
                for index in 0..=64 {
                    let domain = curve.domain();
                    let parameter =
                        *domain.start() + (*domain.end() - *domain.start()) * index as Real / 64.0;
                    let location = curve.evaluate(parameter).unwrap();
                    assert!(
                        ((location.x().hypot(location.y()) - 1.5).hypot(location.z()) - 1.0).abs()
                            < 5e-9
                    );
                    assert!((location.y().hypot(location.z()) - 0.5).abs() < 5e-9);
                }
            }
        }
        let arcs = surface_surface_intersection_events(
            &torus,
            &perpendicular_cylinder(0.5, 0.2, 0.5),
            Tolerance::DEFAULT,
        )
        .unwrap();
        assert_eq!(arcs.len(), 4);
        assert!(arcs.iter().all(|event| matches!(
            event,
            SurfaceSurfaceIntersectionEvent::Curve(curve) if !curve.is_closed().unwrap()
        )));
    }

    #[test]
    fn centered_perpendicular_narrow_cylinder_clips_at_both_rims() {
        let torus = torus();
        let finite = perpendicular_cylinder(0.4, 4.94, 4.97);
        let events =
            surface_surface_intersection_events(&torus, &finite, Tolerance::DEFAULT).unwrap();
        assert_eq!(events.len(), 4);
        for event in events {
            let SurfaceSurfaceIntersectionEvent::Curve(curve) = event else {
                panic!("finite perpendicular cylinder should give arcs")
            };
            assert!(!curve.is_closed().unwrap());
            let domain = curve.domain();
            for index in 0..=32 {
                let parameter =
                    *domain.start() + (*domain.end() - *domain.start()) * (index as Real / 32.0);
                let location = curve.evaluate(parameter).unwrap();
                assert!(location.x() >= 4.94 - 5e-9 && location.x() <= 4.97 + 5e-9);
                assert!(
                    ((location.x().hypot(location.y()) - 4.0).hypot(location.z()) - 1.0).abs()
                        < 5e-9
                );
                assert!((location.y().hypot(location.z()) - 0.4).abs() < 5e-9);
            }
        }
        assert!(
            surface_surface_intersection_events(
                &torus,
                &perpendicular_cylinder(0.4, -1.0, 1.0),
                Tolerance::DEFAULT,
            )
            .unwrap()
            .is_empty()
        );
    }

    #[test]
    fn centered_perpendicular_narrow_cylinder_returns_isolated_rim_contacts() {
        let outer_maximum = (25.0_f64 - 0.4_f64 * 0.4).sqrt();
        let events = surface_surface_intersection_events(
            &torus(),
            &perpendicular_cylinder(0.4, outer_maximum, 6.0),
            Tolerance::DEFAULT,
        )
        .unwrap();
        assert_eq!(events.len(), 2);
        for event in events {
            let SurfaceSurfaceIntersectionEvent::Point(location) = event else {
                panic!("tangent finite rim should leave an isolated contact")
            };
            assert!((location.x() - outer_maximum).abs() < 5e-9);
            assert!((location.y().abs() - 0.4).abs() < 5e-9);
            assert!(location.z().abs() < 5e-9);
        }
    }

    #[test]
    fn centered_perpendicular_narrow_cylinder_handles_distant_rotated_frames() {
        let rotated = Frame3::try_from_normal(
            point(1.0e8, -1.0e8, 1.0e8),
            Vector3::try_new(1.0, 2.0, 3.0).unwrap(),
            Tolerance::DEFAULT,
        )
        .unwrap();
        let cylinder_frame = Frame3::try_from_normal(
            rotated.point_at([-6.0, 0.0, 0.0]).unwrap(),
            rotated.x_axis().as_vector(),
            Tolerance::DEFAULT,
        )
        .unwrap();
        let torus = NurbsSurface::try_torus(rotated, 4.0, 1.0).unwrap();
        let cylinder = NurbsSurface::try_cylinder(cylinder_frame, 0.4, 0.0, 12.0).unwrap();
        let events =
            surface_surface_intersection_events(&torus, &cylinder, Tolerance::DEFAULT).unwrap();
        assert_eq!(events.len(), 4);
        for event in events {
            let SurfaceSurfaceIntersectionEvent::Curve(curve) = event else {
                panic!("rotated perpendicular cylinder should give loops")
            };
            for index in 0..=32 {
                let domain = curve.domain();
                let parameter =
                    *domain.start() + (*domain.end() - *domain.start()) * index as Real / 32.0;
                let local = rotated
                    .coordinates_of(curve.evaluate(parameter).unwrap())
                    .unwrap();
                assert!(((local[0].hypot(local[1]) - 4.0).hypot(local[2]) - 1.0).abs() < 4e-7);
                assert!((local[1].hypot(local[2]) - 0.4).abs() < 4e-7);
            }
        }
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
