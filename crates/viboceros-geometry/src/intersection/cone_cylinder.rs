//! Intersections of finite canonical cone and cylinder walls.

mod parallel_offset;
mod perpendicular_apex;

use super::SurfaceSurfaceIntersectionEvent;
use crate::{Circle3, Frame3, GeometryError, Real, Tolerance};

pub(super) fn cone_cylinder_intersection_events(
    (cone_frame, cone_radius, signed_cone_height): (Frame3, Real, Real),
    (cylinder_frame, cylinder_radius, cylinder_height): (Frame3, Real, Real),
    tolerance: Tolerance,
) -> Result<Vec<SurfaceSurfaceIntersectionEvent>, GeometryError> {
    let coordinate_scale = cone_frame
        .origin()
        .to_array()
        .into_iter()
        .chain(cylinder_frame.origin().to_array())
        .map(Real::abs)
        .fold(0.0, Real::max);
    let coordinate_roundoff = 8.0 * Real::EPSILON * coordinate_scale;
    let cone_axis = cone_frame.z_axis().as_vector();
    let cylinder_axis = cylinder_frame.z_axis().as_vector();
    let axis_drift = cone_axis.cross(cylinder_axis)?.length()? * cylinder_height;
    let spatial_tolerance = tolerance.absolute().max(
        tolerance.relative()
            * cone_radius
                .max(cylinder_radius)
                .max(signed_cone_height.abs())
                .max(cylinder_height),
    );
    if axis_drift > spatial_tolerance.max(coordinate_roundoff) {
        let [origin_x, origin_y, origin_z] = cone_frame.coordinates_of(cylinder_frame.origin())?;
        let cylinder_direction = [
            cone_frame.x_axis().as_vector().dot(cylinder_axis)?,
            cone_frame.y_axis().as_vector().dot(cylinder_axis)?,
            cone_axis.dot(cylinder_axis)?,
        ];
        let angular_drift = cylinder_direction[2].abs()
            * cone_radius
                .max(signed_cone_height.abs())
                .max(cylinder_height);
        let line_offset = (origin_y * cylinder_direction[2] - origin_z * cylinder_direction[1])
            .hypot(origin_z * cylinder_direction[0] - origin_x * cylinder_direction[2])
            .hypot(origin_x * cylinder_direction[1] - origin_y * cylinder_direction[0]);
        if angular_drift <= spatial_tolerance.max(coordinate_roundoff)
            && line_offset <= spatial_tolerance.max(coordinate_roundoff)
        {
            return perpendicular_apex::intersect(
                (cone_frame, cone_radius, signed_cone_height),
                (cylinder_radius, cylinder_height),
                (
                    origin_x,
                    origin_y,
                    cylinder_direction[0],
                    cylinder_direction[1],
                ),
                tolerance,
                coordinate_roundoff,
            );
        }
        return Err(GeometryError::UnsupportedSurfaceSurfaceIntersection {
            context: "nonparallel cone and cylinder walls",
        });
    }

    let [radial_x, radial_y, cylinder_start] =
        cone_frame.coordinates_of(cylinder_frame.origin())?;
    let axis_dot = cone_axis.dot(cylinder_axis)?;
    let cylinder_end = axis_dot.mul_add(cylinder_height, cylinder_start);
    let cylinder_low = cylinder_start.min(cylinder_end);
    let cylinder_high = cylinder_start.max(cylinder_end);
    let cone_low = signed_cone_height.min(0.0);
    let cone_high = signed_cone_height.max(0.0);
    let axial_tolerance = tolerance
        .absolute()
        .max(tolerance.relative() * signed_cone_height.abs().max(cylinder_height))
        .max(coordinate_roundoff);
    if cylinder_low > cone_high + axial_tolerance || cylinder_high < cone_low - axial_tolerance {
        return Ok(Vec::new());
    }

    let radial_offset = radial_x.hypot(radial_y);
    let radial_tolerance = tolerance
        .absolute()
        .max(tolerance.relative() * cone_radius.max(cylinder_radius).max(radial_offset));
    let coaxial_tolerance = radial_tolerance.max(coordinate_roundoff);
    if radial_offset > cone_radius + cylinder_radius + coaxial_tolerance
        || cylinder_radius > radial_offset + cone_radius + coaxial_tolerance
    {
        return Ok(Vec::new());
    }
    if radial_offset > coaxial_tolerance {
        return parallel_offset::intersect(
            (cone_frame, cone_radius, signed_cone_height),
            (cylinder_radius, cylinder_height),
            (radial_x, radial_y, cylinder_start, axis_dot),
            tolerance,
            coordinate_roundoff,
        );
    }
    if cylinder_radius > cone_radius {
        return Ok(Vec::new());
    }

    let section_axial = signed_cone_height * (cylinder_radius / cone_radius);
    if section_axial < cylinder_low - axial_tolerance
        || section_axial > cylinder_high + axial_tolerance
    {
        return Ok(Vec::new());
    }
    let center = cone_frame.point_at([0.0, 0.0, section_axial])?;
    let circle = Circle3::try_from_frame(
        center,
        cylinder_radius,
        cone_frame.x_axis(),
        cone_frame.z_axis(),
        tolerance,
    )?
    .to_nurbs()?;
    if axis_dot < 0.0 {
        let domain = circle.domain();
        let middle = 0.5 * (*domain.start() + *domain.end());
        return Ok(vec![
            SurfaceSurfaceIntersectionEvent::Curve(circle.try_subcurve(*domain.start(), middle)?),
            SurfaceSurfaceIntersectionEvent::Curve(circle.try_subcurve(middle, *domain.end())?),
        ]);
    }
    Ok(vec![SurfaceSurfaceIntersectionEvent::Curve(circle)])
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

    #[test]
    fn coaxial_cone_cylinder_intersection_returns_exact_circle_in_both_orders() {
        let cone = NurbsSurface::try_cone(frame(), 3.0, 4.0).unwrap();
        let cylinder = NurbsSurface::try_cylinder(frame(), 1.5, 0.0, 4.0).unwrap();
        for (first, second) in [(&cone, &cylinder), (&cylinder, &cone)] {
            let events =
                surface_surface_intersection_events(first, second, Tolerance::DEFAULT).unwrap();
            let [SurfaceSurfaceIntersectionEvent::Curve(circle)] = events.as_slice() else {
                panic!("expected one exact circle, got {events:#?}")
            };
            assert_eq!(circle.degree(), 2);
            assert!(circle.is_closed().unwrap());
            assert!(
                (circle.length(Tolerance::DEFAULT).unwrap() - 3.0 * std::f64::consts::PI).abs()
                    < 1e-8
            );
            let domain = circle.domain();
            for fraction in [0.0, 0.125, 0.33, 0.75] {
                let sample = circle
                    .evaluate(*domain.start() + fraction * (*domain.end() - *domain.start()))
                    .unwrap();
                assert!((sample.z() - 2.0).abs() < 1e-9);
                assert!((sample.x().hypot(sample.y()) - 1.5).abs() < 1e-9);
            }
        }
    }

    #[test]
    fn coaxial_cone_cylinder_intersection_respects_rims_orientation_and_no_hits() {
        let cone = NurbsSurface::try_cone(frame(), 3.0, 4.0).unwrap();
        let rim_cylinder = NurbsSurface::try_cylinder(frame(), 1.5, 2.0, 4.0).unwrap();
        assert_eq!(
            surface_surface_intersection_events(&cone, &rim_cylinder, Tolerance::DEFAULT)
                .unwrap()
                .len(),
            1
        );
        let base_cylinder = NurbsSurface::try_cylinder(frame(), 3.0, 0.0, 4.0).unwrap();
        assert_eq!(
            surface_surface_intersection_events(&cone, &base_cylinder, Tolerance::DEFAULT)
                .unwrap()
                .len(),
            1
        );
        let opposed_frame = Frame3::try_from_normal(
            point(0.0, 0.0, 4.0),
            Vector3::try_new(0.0, 0.0, -1.0).unwrap(),
            Tolerance::DEFAULT,
        )
        .unwrap();
        let opposed = NurbsSurface::try_cylinder(opposed_frame, 1.5, 0.0, 4.0).unwrap();
        let events =
            surface_surface_intersection_events(&cone, &opposed, Tolerance::DEFAULT).unwrap();
        assert_eq!(events.len(), 2);
        for event in events {
            let SurfaceSurfaceIntersectionEvent::Curve(arc) = event else {
                panic!("opposed axes should create semicircular arcs")
            };
            assert_eq!(arc.degree(), 2);
            assert!(!arc.is_closed().unwrap());
            assert!(
                (arc.length(Tolerance::DEFAULT).unwrap() - 1.5 * std::f64::consts::PI).abs() < 1e-8
            );
        }
        for cylinder in [
            NurbsSurface::try_cylinder(frame(), 1.5, 2.5, 4.0).unwrap(),
            NurbsSurface::try_cylinder(frame(), 4.0, 0.0, 4.0).unwrap(),
        ] {
            assert!(
                surface_surface_intersection_events(&cone, &cylinder, Tolerance::DEFAULT)
                    .unwrap()
                    .is_empty()
            );
        }
    }

    #[test]
    fn coaxial_cone_cylinder_intersection_handles_negative_height_and_large_coordinates() {
        let negative_cone = NurbsSurface::try_cone(frame(), 3.0, -4.0).unwrap();
        let negative_cylinder = NurbsSurface::try_cylinder(frame(), 1.5, -4.0, 0.0).unwrap();
        let events = surface_surface_intersection_events(
            &negative_cone,
            &negative_cylinder,
            Tolerance::DEFAULT,
        )
        .unwrap();
        let [SurfaceSurfaceIntersectionEvent::Curve(circle)] = events.as_slice() else {
            panic!("negative cone height should produce a circle")
        };
        assert!((circle.evaluate(*circle.domain().start()).unwrap().z() + 2.0).abs() < 1e-9);

        let rotated = Frame3::try_from_normal(
            point(1.0e8, -1.0e8, 1.0e8),
            Vector3::try_new(1.0, 2.0, 3.0).unwrap(),
            Tolerance::DEFAULT,
        )
        .unwrap();
        let cone = NurbsSurface::try_cone(rotated, 3.0, 4.0).unwrap();
        let cylinder = NurbsSurface::try_cylinder(rotated, 1.5, 0.0, 4.0).unwrap();
        let events =
            surface_surface_intersection_events(&cone, &cylinder, Tolerance::DEFAULT).unwrap();
        let [SurfaceSurfaceIntersectionEvent::Curve(circle)] = events.as_slice() else {
            panic!("rotated distant coaxial primitives should meet in one circle")
        };
        let sample = circle.evaluate(*circle.domain().start()).unwrap();
        let local = rotated.coordinates_of(sample).unwrap();
        assert!((local[2] - 2.0).abs() < 1e-7);
        assert!((local[0].hypot(local[1]) - 1.5).abs() < 1e-7);
    }
}
