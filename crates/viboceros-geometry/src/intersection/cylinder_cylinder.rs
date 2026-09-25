//! Exact intersections of finite canonical cylinder walls in supported alignments.

use super::SurfaceSurfaceIntersectionEvent;
use crate::{Circle3, Frame3, GeometryError, NurbsCurve, Real, Tolerance};

mod crossed;

pub(super) fn cylinder_cylinder_intersection_events(
    (first_frame, first_radius, first_height): (Frame3, Real, Real),
    (second_frame, second_radius, second_height): (Frame3, Real, Real),
    tolerance: Tolerance,
) -> Result<Vec<SurfaceSurfaceIntersectionEvent>, GeometryError> {
    let coordinate_scale = first_frame
        .origin()
        .to_array()
        .into_iter()
        .chain(second_frame.origin().to_array())
        .map(Real::abs)
        .fold(0.0, Real::max);
    let coordinate_roundoff = 8.0 * Real::EPSILON * coordinate_scale;
    let axis_drift = first_frame
        .z_axis()
        .as_vector()
        .cross(second_frame.z_axis().as_vector())?
        .length()?
        * second_height;
    if axis_drift > (tolerance.angular() * second_height).max(coordinate_roundoff) {
        return crossed::intersect(
            (first_frame, first_radius, first_height),
            (second_frame, second_radius, second_height),
            tolerance,
        );
    }

    let [center_x, center_y, second_start] = first_frame.coordinates_of(second_frame.origin())?;
    let axis_sign = first_frame
        .z_axis()
        .as_vector()
        .dot(second_frame.z_axis().as_vector())?;
    let second_end = axis_sign.mul_add(second_height, second_start);
    let second_low = second_start.min(second_end);
    let second_high = second_start.max(second_end);
    let axial_tolerance = tolerance
        .absolute()
        .max(tolerance.relative() * first_height.max(second_height))
        .max(coordinate_roundoff);
    if second_low > first_height + axial_tolerance || second_high < -axial_tolerance {
        return Ok(Vec::new());
    }
    let overlap_start = second_low.max(0.0);
    let overlap_end = second_high.min(first_height);
    let radial_separation = center_x.hypot(center_y);
    let radial_tolerance = tolerance
        .absolute()
        .max(tolerance.relative() * first_radius.max(second_radius).max(radial_separation))
        .max(coordinate_roundoff);
    let radius_sum = first_radius + second_radius;
    let radius_difference = (first_radius - second_radius).abs();
    if radial_separation > radius_sum + radial_tolerance
        || radial_separation < radius_difference - radial_tolerance
    {
        return Ok(Vec::new());
    }

    let zero_separation =
        coordinate_roundoff.max(8.0 * Real::EPSILON * first_radius.max(second_radius));
    if radial_separation <= zero_separation {
        if radius_difference > zero_separation {
            return Ok(Vec::new());
        }
        if overlap_end - overlap_start > axial_tolerance {
            return Ok(Vec::new());
        }
        let axial = (overlap_start + overlap_end) * 0.5;
        let center = first_frame.point_at([0.0, 0.0, axial.clamp(0.0, first_height)])?;
        let circle = Circle3::try_from_frame(
            center,
            first_radius,
            first_frame.x_axis(),
            first_frame.z_axis(),
            tolerance,
        )?
        .to_nurbs()?;
        return Ok(vec![SurfaceSurfaceIntersectionEvent::Curve(circle)]);
    }

    let radial_axis_x = center_x / radial_separation;
    let radial_axis_y = center_y / radial_separation;
    let axial_distance =
        0.5 * (radial_separation + (first_radius - second_radius) * radius_sum / radial_separation);
    let chord_center_x = axial_distance * radial_axis_x;
    let chord_center_y = axial_distance * radial_axis_y;
    let half_chord_squared = (first_radius - axial_distance) * (first_radius + axial_distance);
    let half_chord = half_chord_squared.max(0.0).sqrt();
    let tangent = half_chord <= radial_tolerance;
    let offsets = if tangent {
        let multiplicity = if (radial_separation - radius_sum).abs()
            <= (radial_separation - radius_difference).abs()
        {
            2
        } else {
            4
        };
        vec![0.0; multiplicity]
    } else {
        vec![half_chord, -half_chord]
    };
    let mut events = Vec::with_capacity(offsets.len());
    for (index, offset) in offsets.into_iter().enumerate() {
        let x = (-radial_axis_y).mul_add(offset, chord_center_x);
        let y = radial_axis_x.mul_add(offset, chord_center_y);
        if overlap_end - overlap_start <= axial_tolerance {
            let axial = 0.5 * (overlap_start + overlap_end);
            events.push(SurfaceSurfaceIntersectionEvent::Point(
                first_frame.point_at([x, y, axial.clamp(0.0, first_height)])?,
            ));
        } else {
            let (start_axial, end_axial) = if tangent || index == 0 {
                (overlap_end, overlap_start)
            } else {
                (overlap_start, overlap_end)
            };
            let start = first_frame.point_at([x, y, start_axial])?;
            let end = first_frame.point_at([x, y, end_axial])?;
            let length = overlap_end - overlap_start;
            events.push(SurfaceSurfaceIntersectionEvent::Curve(NurbsCurve::try_new(
                1,
                vec![start, end],
                vec![0.0, 0.0, length, length],
            )?));
        }
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

    fn cylinder(origin: Point3, axis: Vector3, radius: Real, height: Real) -> NurbsSurface {
        let frame = Frame3::try_from_normal(origin, axis, Tolerance::DEFAULT).unwrap();
        NurbsSurface::try_cylinder(frame, radius, 0.0, height).unwrap()
    }

    fn z_axis() -> Vector3 {
        Vector3::try_new(0.0, 0.0, 1.0).unwrap()
    }

    #[test]
    fn parallel_cylinders_intersect_in_exact_clipped_generatrices() {
        let first = cylinder(point(0.0, 0.0, 0.0), z_axis(), 2.0, 5.0);
        let second = cylinder(point(2.0, 0.0, 1.0), z_axis(), 2.0, 3.0);
        for (left, right) in [(&first, &second), (&second, &first)] {
            let events =
                surface_surface_intersection_events(left, right, Tolerance::DEFAULT).unwrap();
            assert_eq!(events.len(), 2);
            let mut sides = Vec::new();
            for event in &events {
                let SurfaceSurfaceIntersectionEvent::Curve(line) = event else {
                    panic!("expected two generatrices, got {events:#?}")
                };
                assert_eq!(line.degree(), 1);
                assert!((line.length(Tolerance::DEFAULT).unwrap() - 3.0).abs() < 1e-9);
                let start = line.evaluate(*line.domain().start()).unwrap();
                let end = line.evaluate(*line.domain().end()).unwrap();
                assert!((start.x() - 1.0).abs() < 1e-9);
                assert!((end.x() - 1.0).abs() < 1e-9);
                assert!((start.y() - end.y()).abs() < 1e-9);
                assert!((start.z().min(end.z()) - 1.0).abs() < 1e-9);
                assert!((start.z().max(end.z()) - 4.0).abs() < 1e-9);
                sides.push(start.y());
            }
            sides.sort_by(Real::total_cmp);
            assert!((sides[0] + 3.0_f64.sqrt()).abs() < 1e-9);
            assert!((sides[1] - 3.0_f64.sqrt()).abs() < 1e-9);
        }
        let opposed = cylinder(
            point(2.0, 0.0, 4.0),
            Vector3::try_new(0.0, 0.0, -1.0).unwrap(),
            2.0,
            3.0,
        );
        assert_eq!(
            surface_surface_intersection_events(&first, &opposed, Tolerance::DEFAULT)
                .unwrap()
                .len(),
            2
        );
    }

    #[test]
    fn parallel_cylinder_tangencies_keep_rhino_line_multiplicity() {
        let first = cylinder(point(0.0, 0.0, 0.0), z_axis(), 2.0, 5.0);
        for (offset, radius, multiplicity) in [(4.0, 2.0, 2), (1.0, 1.0, 4)] {
            let other = cylinder(point(offset, 0.0, 1.0), z_axis(), radius, 3.0);
            let events =
                surface_surface_intersection_events(&first, &other, Tolerance::DEFAULT).unwrap();
            assert_eq!(events.len(), multiplicity);
            for event in events {
                let SurfaceSurfaceIntersectionEvent::Curve(line) = event else {
                    panic!("tangency must produce a line")
                };
                let start = line.evaluate(*line.domain().start()).unwrap();
                let end = line.evaluate(*line.domain().end()).unwrap();
                assert!((start.x() - 2.0).abs() < 1e-9);
                assert!(start.y().abs() < 1e-9);
                assert!((start.z() - 4.0).abs() < 1e-9);
                assert!((end.z() - 1.0).abs() < 1e-9);
            }
        }
        let near_tangent = cylinder(point(4.0 - 1.0e-9, 0.0, 1.0), z_axis(), 2.0, 3.0);
        let events =
            surface_surface_intersection_events(&first, &near_tangent, Tolerance::DEFAULT).unwrap();
        assert_eq!(events.len(), 2);
        let mut sides = events
            .iter()
            .map(|event| {
                let SurfaceSurfaceIntersectionEvent::Curve(line) = event else {
                    panic!("near tangency must produce two lines")
                };
                line.evaluate(*line.domain().start()).unwrap().y()
            })
            .collect::<Vec<_>>();
        sides.sort_by(Real::total_cmp);
        assert!(sides[0] < -1e-5 && sides[1] > 1e-5);
    }

    #[test]
    fn parallel_cylinders_report_shared_rims_points_and_no_hits() {
        let first = cylinder(point(0.0, 0.0, 0.0), z_axis(), 2.0, 2.0);
        let transverse = cylinder(point(2.0, 0.0, 2.0), z_axis(), 2.0, 3.0);
        let events =
            surface_surface_intersection_events(&first, &transverse, Tolerance::DEFAULT).unwrap();
        assert_eq!(events.len(), 2);
        for event in events {
            let SurfaceSurfaceIntersectionEvent::Point(location) = event else {
                panic!("rim-only intersection must produce points")
            };
            assert!((location.x() - 1.0).abs() < 1e-9);
            assert!((location.y().abs() - 3.0_f64.sqrt()).abs() < 1e-9);
            assert!((location.z() - 2.0).abs() < 1e-9);
        }
        let shared_rim = cylinder(point(0.0, 0.0, 2.0), z_axis(), 2.0, 3.0);
        let events =
            surface_surface_intersection_events(&first, &shared_rim, Tolerance::DEFAULT).unwrap();
        let [SurfaceSurfaceIntersectionEvent::Curve(circle)] = events.as_slice() else {
            panic!("coaxial cylinders should share a rim circle")
        };
        assert!(circle.is_closed().unwrap());
        assert!(
            (circle.length(Tolerance::DEFAULT).unwrap() - 4.0 * std::f64::consts::PI).abs() < 1e-8
        );

        let overlap = cylinder(point(0.0, 0.0, 1.0), z_axis(), 2.0, 3.0);
        assert!(
            surface_surface_intersection_events(&first, &overlap, Tolerance::DEFAULT)
                .unwrap()
                .is_empty()
        );
        for other in [
            cylinder(point(5.0, 0.0, 0.0), z_axis(), 2.0, 2.0),
            cylinder(point(2.0, 0.0, 3.0), z_axis(), 2.0, 2.0),
            cylinder(point(0.5, 0.0, 0.0), z_axis(), 1.0, 2.0),
        ] {
            assert!(
                surface_surface_intersection_events(&first, &other, Tolerance::DEFAULT)
                    .unwrap()
                    .is_empty()
            );
        }
        let skew = cylinder(
            point(1.0, 0.0, 0.0),
            Vector3::try_new(0.0, 1.0, 1.0).unwrap(),
            2.0,
            2.0,
        );
        assert!(matches!(
            surface_surface_intersection_events(&first, &skew, Tolerance::DEFAULT),
            Err(GeometryError::UnsupportedSurfaceSurfaceIntersection { .. })
        ));
    }

    #[test]
    fn parallel_cylinder_intersection_works_far_from_origin_with_rotated_axis() {
        let frame = Frame3::try_from_normal(
            point(1.0e8, -1.0e8, 1.0e8),
            Vector3::try_new(1.0, 2.0, 3.0).unwrap(),
            Tolerance::DEFAULT,
        )
        .unwrap();
        let first = NurbsSurface::try_cylinder(frame, 2.0, 0.0, 5.0).unwrap();
        let second = NurbsSurface::try_cylinder(
            frame.with_origin(frame.point_at([2.0, 0.0, 1.0]).unwrap()),
            2.0,
            0.0,
            3.0,
        )
        .unwrap();
        let events =
            surface_surface_intersection_events(&first, &second, Tolerance::DEFAULT).unwrap();
        assert_eq!(events.len(), 2);
        for event in events {
            let SurfaceSurfaceIntersectionEvent::Curve(line) = event else {
                panic!("expected exact lines")
            };
            assert!((line.length(Tolerance::DEFAULT).unwrap() - 3.0).abs() < 1e-7);
            for parameter in [*line.domain().start(), *line.domain().end()] {
                let local = frame
                    .coordinates_of(line.evaluate(parameter).unwrap())
                    .unwrap();
                assert!((local[0] - 1.0).abs() < 2e-7);
                assert!((local[1].abs() - 3.0_f64.sqrt()).abs() < 2e-7);
                assert!(local[2] >= 1.0 - 2e-7 && local[2] <= 4.0 + 2e-7);
            }
        }
    }
}
