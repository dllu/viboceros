//! Cubic, tolerance-bounded intersections of a sphere and an offset cylinder.
//!
//! The smooth branches exist where the sphere's axial square root stays
//! positive for every cylinder angle. Their cubic Hermite interpolation error
//! is bounded by the fourth derivative of that exact parameterization.

use super::SurfaceSurfaceIntersectionEvent;
use crate::{Frame3, GeometryError, NurbsCurve, Point3, Real, Tolerance, Vector3};

const MAX_SEGMENTS_PER_BRANCH: usize = 1024;

pub(super) fn intersect(
    sphere_center: Point3,
    sphere_radius: Real,
    cylinder_frame: Frame3,
    cylinder_radius: Real,
    cylinder_height: Real,
    tolerance: Tolerance,
) -> Result<Vec<SurfaceSurfaceIntersectionEvent>, GeometryError> {
    let [center_x, center_y, center_z] = cylinder_frame.coordinates_of(sphere_center)?;
    let radial_offset = center_x.hypot(center_y);
    let radial_axis = [center_x / radial_offset, center_y / radial_offset];
    let constant = (sphere_radius - cylinder_radius) * (sphere_radius + cylinder_radius)
        - radial_offset * radial_offset;
    let cosine_coefficient = 2.0 * cylinder_radius * radial_offset;
    let minimum_radicand = constant - cosine_coefficient;
    let maximum_radicand = constant + cosine_coefficient;
    if maximum_radicand < 0.0 {
        return Ok(Vec::new());
    }
    if maximum_radicand == 0.0 {
        if (0.0..=cylinder_height).contains(&center_z) {
            return Ok(vec![SurfaceSurfaceIntersectionEvent::Point(
                cylinder_frame.point_at([
                    cylinder_radius * radial_axis[0],
                    cylinder_radius * radial_axis[1],
                    center_z,
                ])?,
            )]);
        }
        return Ok(Vec::new());
    }
    if minimum_radicand < 0.0 {
        return super::sphere_cylinder_turning::intersect(
            sphere_center,
            sphere_radius,
            cylinder_frame,
            cylinder_radius,
            cylinder_height,
            tolerance,
            radial_axis,
            center_z,
            maximum_radicand,
            cosine_coefficient,
        );
    }
    if minimum_radicand == 0.0 {
        return Err(GeometryError::UnsupportedSurfaceSurfaceIntersection {
            context: "sphere/cylinder branches cross at a radial singularity",
        });
    }

    let coordinate_scale = cylinder_frame
        .origin()
        .to_array()
        .into_iter()
        .chain(sphere_center.to_array())
        .map(Real::abs)
        .fold(0.0, Real::max);
    let fit_tolerance = tolerance
        .absolute()
        .max(tolerance.relative() * sphere_radius.max(cylinder_radius).max(cylinder_height))
        .max(8.0 * Real::EPSILON * coordinate_scale);
    let fourth_derivative_bound =
        fourth_derivative_bound(cylinder_radius, cosine_coefficient, minimum_radicand);
    if !fourth_derivative_bound.is_finite() {
        return Err(GeometryError::UnsupportedSurfaceSurfaceIntersection {
            context: "sphere/cylinder curve fit is ill-conditioned",
        });
    }

    let mut events = Vec::new();
    for sign in [-1.0, 1.0] {
        let intervals = active_angular_intervals(
            center_z,
            sign,
            constant,
            cosine_coefficient,
            cylinder_height,
        );
        for angle in [0.0, std::f64::consts::PI] {
            let z = center_z + sign * (constant + cosine_coefficient * angle.cos()).sqrt();
            let touches_rim =
                z.abs() <= fit_tolerance || (z - cylinder_height).abs() <= fit_tolerance;
            if touches_rim && !angle_in_intervals(angle, &intervals) {
                let (point, _) = branch_sample(
                    cylinder_frame,
                    cylinder_radius,
                    center_z,
                    radial_axis,
                    constant,
                    cosine_coefficient,
                    sign,
                    angle,
                )?;
                events.push(SurfaceSurfaceIntersectionEvent::Point(point));
            }
        }
        for (start, end) in intervals {
            let curve = fit_branch(
                cylinder_frame,
                cylinder_radius,
                center_z,
                radial_axis,
                constant,
                cosine_coefficient,
                sign,
                start,
                end,
                fit_tolerance,
                fourth_derivative_bound,
            )?;
            events.push(SurfaceSurfaceIntersectionEvent::Curve(curve));
        }
    }
    Ok(events)
}

fn angle_in_intervals(angle: Real, intervals: &[(Real, Real)]) -> bool {
    let turn = std::f64::consts::TAU;
    [-turn, 0.0, turn].into_iter().any(|shift| {
        intervals.iter().any(|(start, end)| {
            angle + shift >= *start - 16.0 * Real::EPSILON
                && angle + shift <= *end + 16.0 * Real::EPSILON
        })
    })
}

fn fourth_derivative_bound(radius: Real, coefficient: Real, minimum: Real) -> Real {
    let b = coefficient.abs();
    let root = minimum.sqrt();
    let root3 = minimum * root;
    let root5 = minimum * root3;
    let root7 = minimum * root5;
    let vertical = b / (2.0 * root)
        + 7.0 * b * b / (4.0 * root3)
        + 9.0 * b * b * b / (4.0 * root5)
        + 15.0 * b.powi(4) / (16.0 * root7);
    radius.hypot(vertical)
}

fn active_angular_intervals(
    center_z: Real,
    sign: Real,
    constant: Real,
    coefficient: Real,
    height: Real,
) -> Vec<(Real, Real)> {
    let full_turn = std::f64::consts::TAU;
    let mut angles = vec![0.0, full_turn];
    for boundary in [0.0, height] {
        let delta = boundary - center_z;
        if sign * delta <= 0.0 {
            continue;
        }
        let cosine = (delta * delta - constant) / coefficient;
        if (-1.0..=1.0).contains(&cosine) {
            let angle = cosine.acos();
            angles.push(angle);
            angles.push(full_turn - angle);
        }
    }
    angles.sort_by(Real::total_cmp);
    angles.dedup_by(|left, right| (*left - *right).abs() <= 16.0 * Real::EPSILON);
    let mut intervals: Vec<(Real, Real)> = Vec::new();
    for pair in angles.windows(2) {
        let middle = 0.5 * (pair[0] + pair[1]);
        let z = center_z + sign * (constant + coefficient * middle.cos()).sqrt();
        if (0.0..=height).contains(&z) {
            if let Some(last) = intervals.last_mut()
                && (last.1 - pair[0]).abs() <= 16.0 * Real::EPSILON
            {
                last.1 = pair[1];
            } else {
                intervals.push((pair[0], pair[1]));
            }
        }
    }
    if intervals.len() > 1
        && intervals[0].0 == 0.0
        && intervals.last().is_some_and(|last| last.1 == full_turn)
    {
        let first = intervals.remove(0);
        let last = intervals.pop().expect("at least two intervals");
        intervals.insert(0, (last.0 - full_turn, first.1));
    }
    intervals
}

#[allow(clippy::too_many_arguments)]
fn fit_branch(
    frame: Frame3,
    radius: Real,
    center_z: Real,
    radial_axis: [Real; 2],
    constant: Real,
    coefficient: Real,
    sign: Real,
    start: Real,
    end: Real,
    fit_tolerance: Real,
    derivative_bound: Real,
) -> Result<NurbsCurve, GeometryError> {
    let max_span = (384.0 * fit_tolerance / derivative_bound).powf(0.25);
    let required = ((end - start) / max_span).ceil();
    if !required.is_finite() || required > MAX_SEGMENTS_PER_BRANCH as Real {
        return Err(GeometryError::TooManyCurveFitControlPoints {
            maximum: 3 * MAX_SEGMENTS_PER_BRANCH + 1,
        });
    }
    let closed = (end - start - std::f64::consts::TAU).abs() <= 16.0 * Real::EPSILON;
    let segments = (required as usize).max(if closed { 4 } else { 1 });
    let mut controls = Vec::with_capacity(3 * segments + 1);
    let mut knots = Vec::with_capacity(3 * segments + 5);
    knots.extend([start; 4]);
    let (first_point, mut previous_tangent) = branch_sample(
        frame,
        radius,
        center_z,
        radial_axis,
        constant,
        coefficient,
        sign,
        start,
    )?;
    controls.push(first_point);
    let mut previous_point = first_point;
    let mut previous_angle = start;
    for segment in 1..=segments {
        let angle = start + (end - start) * (segment as Real / segments as Real);
        let (mut point, tangent) = branch_sample(
            frame,
            radius,
            center_z,
            radial_axis,
            constant,
            coefficient,
            sign,
            angle,
        )?;
        if closed && segment == segments {
            point = first_point;
        }
        let handle = (angle - previous_angle) / 3.0;
        controls.push(previous_point.translated(previous_tangent.scaled(handle)?)?);
        controls.push(point.translated(tangent.scaled(-handle)?)?);
        controls.push(point);
        knots.extend([angle; 3]);
        previous_point = point;
        previous_tangent = tangent;
        previous_angle = angle;
    }
    knots.push(end);
    NurbsCurve::try_new(3, controls, knots)
}

#[allow(clippy::too_many_arguments)]
fn branch_sample(
    frame: Frame3,
    radius: Real,
    center_z: Real,
    radial_axis: [Real; 2],
    constant: Real,
    coefficient: Real,
    sign: Real,
    angle: Real,
) -> Result<(Point3, Vector3), GeometryError> {
    let (sine, cosine) = angle.sin_cos();
    let radicand = constant + coefficient * cosine;
    let root = radicand.sqrt();
    let [axis_x, axis_y] = radial_axis;
    let x = radius * (cosine * axis_x - sine * axis_y);
    let y = radius * (cosine * axis_y + sine * axis_x);
    let z = center_z + sign * root;
    let derivative_x = radius * (-sine * axis_x - cosine * axis_y);
    let derivative_y = radius * (-sine * axis_y + cosine * axis_x);
    let derivative_z = -sign * coefficient * sine / (2.0 * root);
    Ok((
        frame.point_at([x, y, z])?,
        frame.vector_at([derivative_x, derivative_y, derivative_z])?,
    ))
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

    #[test]
    fn offset_sphere_cylinder_intersection_clips_smooth_branches_at_both_rims() {
        let sphere =
            NurbsSurface::try_sphere(frame().with_origin(point(0.5, 0.0, 2.5)), 2.5).unwrap();
        for (first_height, last_height, clipped_height) in [(0.0, 4.5, 4.5), (0.5, 5.0, 0.5)] {
            let cylinder =
                NurbsSurface::try_cylinder(frame(), 1.5, first_height, last_height).unwrap();
            let events =
                surface_surface_intersection_events(&sphere, &cylinder, Tolerance::DEFAULT)
                    .unwrap();
            assert_eq!(events.len(), 2);
            let mut closed_count = 0;
            for event in events {
                let SurfaceSurfaceIntersectionEvent::Curve(curve) = event else {
                    panic!("offset sections should be curves")
                };
                assert_eq!(curve.degree(), 3);
                if curve.is_closed().unwrap() {
                    closed_count += 1;
                } else {
                    for parameter in [*curve.domain().start(), *curve.domain().end()] {
                        let end = curve.evaluate(parameter).unwrap();
                        assert!((end.z() - clipped_height).abs() < 1e-9);
                    }
                }
                let domain = curve.domain();
                for index in 0..=64 {
                    let parameter = *domain.start()
                        + (*domain.end() - *domain.start()) * (index as Real / 64.0);
                    let sample = curve.evaluate(parameter).unwrap();
                    assert!((sample.x().hypot(sample.y()) - 1.5).abs() < 2e-9);
                    assert!((sample.distance_to(point(0.5, 0.0, 2.5)).unwrap() - 2.5).abs() < 2e-9);
                    assert!(sample.z() >= first_height - 1e-9);
                    assert!(sample.z() <= last_height + 1e-9);
                }
            }
            assert_eq!(closed_count, 1);
        }
    }

    #[test]
    fn offset_sphere_cylinder_intersection_keeps_isolated_rim_tangency() {
        let sphere =
            NurbsSurface::try_sphere(frame().with_origin(point(0.5, 0.0, 2.5)), 2.5).unwrap();
        let cylinder = NurbsSurface::try_cylinder(frame(), 1.5, 0.0, 4.0).unwrap();
        let events =
            surface_surface_intersection_events(&sphere, &cylinder, Tolerance::DEFAULT).unwrap();
        assert_eq!(events.len(), 2);
        assert!(
            matches!(&events[0], SurfaceSurfaceIntersectionEvent::Curve(curve) if curve.is_closed().unwrap())
        );
        let SurfaceSurfaceIntersectionEvent::Point(location) = events[1] else {
            panic!("top rim tangency must be an isolated point")
        };
        assert!(location.distance_to(point(-1.5, 0.0, 4.0)).unwrap() < 1e-9);
    }

    #[test]
    fn offset_sphere_cylinder_narrow_height_band_yields_two_open_arcs() {
        let sphere =
            NurbsSurface::try_sphere(frame().with_origin(point(0.5, 0.0, 2.5)), 2.5).unwrap();
        let cylinder = NurbsSurface::try_cylinder(frame(), 1.5, 4.2, 4.5).unwrap();
        let events =
            surface_surface_intersection_events(&sphere, &cylinder, Tolerance::DEFAULT).unwrap();
        assert_eq!(events.len(), 2);
        for event in events {
            let SurfaceSurfaceIntersectionEvent::Curve(curve) = event else {
                panic!("height band must leave two arcs")
            };
            assert!(!curve.is_closed().unwrap());
            for parameter in [*curve.domain().start(), *curve.domain().end()] {
                let end = curve.evaluate(parameter).unwrap();
                assert!((end.z() - 4.2).abs() < 1e-9 || (end.z() - 4.5).abs() < 1e-9);
            }
        }
    }

    #[test]
    fn offset_sphere_cylinder_fit_works_far_from_origin_in_a_rotated_frame() {
        let frame = Frame3::try_from_normal(
            point(1e8, -1e8, 1e8),
            Vector3::try_new(1.0, 2.0, 3.0).unwrap(),
            Tolerance::DEFAULT,
        )
        .unwrap();
        let sphere = NurbsSurface::try_sphere(
            frame.with_origin(frame.point_at([0.5, 0.0, 2.5]).unwrap()),
            2.5,
        )
        .unwrap();
        let cylinder = NurbsSurface::try_cylinder(frame, 1.5, 0.0, 5.0).unwrap();
        let events =
            surface_surface_intersection_events(&sphere, &cylinder, Tolerance::DEFAULT).unwrap();
        assert_eq!(events.len(), 2);
        let center = frame.point_at([0.5, 0.0, 2.5]).unwrap();
        for event in events {
            let SurfaceSurfaceIntersectionEvent::Curve(curve) = event else {
                panic!("offset intersections must be fitted curves")
            };
            for index in 0..=32 {
                let domain = curve.domain();
                let parameter =
                    *domain.start() + (*domain.end() - *domain.start()) * (index as Real / 32.0);
                let sample = curve.evaluate(parameter).unwrap();
                let local = frame.coordinates_of(sample).unwrap();
                assert!((local[0].hypot(local[1]) - 1.5).abs() < 3e-7);
                assert!((sample.distance_to(center).unwrap() - 2.5).abs() < 3e-7);
            }
        }
    }
}
