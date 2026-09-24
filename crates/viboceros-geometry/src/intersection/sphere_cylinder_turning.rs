//! Smooth offset sphere/cylinder loops whose axial branches meet at two turns.
//!
//! With `Q(θ) = C + B cos θ`, `Qmin < 0 < Qmax` gives a single smooth
//! loop. The substitution `z = z₀ + sqrt(Qmax) cos t` and
//! `θ = 2 asin(sqrt(Qmax/(2B)) sin t)` removes the square-root endpoints.

use super::SurfaceSurfaceIntersectionEvent;
use crate::{Frame3, GeometryError, NurbsCurve, Point3, Real, Tolerance, Vector3};

const MAX_SEGMENTS: usize = 2048;

#[allow(clippy::too_many_arguments)]
pub(super) fn intersect(
    sphere_center: Point3,
    sphere_radius: Real,
    frame: Frame3,
    radius: Real,
    height: Real,
    tolerance: Tolerance,
    radial_axis: [Real; 2],
    center_z: Real,
    maximum_radicand: Real,
    coefficient: Real,
) -> Result<Vec<SurfaceSurfaceIntersectionEvent>, GeometryError> {
    let vertical_radius = maximum_radicand.sqrt();
    let k = (maximum_radicand / (2.0 * coefficient)).sqrt();
    let derivative_bound = fourth_derivative_bound(radius, vertical_radius, k);
    if !derivative_bound.is_finite() {
        return Err(GeometryError::UnsupportedSurfaceSurfaceIntersection {
            context: "sphere/cylinder turning curve is ill-conditioned",
        });
    }
    let coordinate_scale = frame
        .origin()
        .to_array()
        .into_iter()
        .chain(sphere_center.to_array())
        .map(Real::abs)
        .fold(0.0, Real::max);
    let fit_tolerance = tolerance
        .absolute()
        .max(tolerance.relative() * sphere_radius.max(radius).max(height))
        .max(8.0 * Real::EPSILON * coordinate_scale);

    let intervals = active_intervals(center_z, vertical_radius, height);
    let mut events = Vec::new();
    for angle in [0.0, std::f64::consts::PI] {
        let z = center_z + vertical_radius * angle.cos();
        if (z.abs() <= fit_tolerance || (z - height).abs() <= fit_tolerance)
            && !angle_in_intervals(angle, &intervals)
        {
            events.push(SurfaceSurfaceIntersectionEvent::Point(
                sample(
                    frame,
                    radius,
                    radial_axis,
                    center_z,
                    vertical_radius,
                    k,
                    angle,
                )?
                .0,
            ));
        }
    }
    for (start, end) in intervals {
        events.push(SurfaceSurfaceIntersectionEvent::Curve(fit_curve(
            frame,
            radius,
            radial_axis,
            center_z,
            vertical_radius,
            k,
            start,
            end,
            fit_tolerance,
            derivative_bound,
        )?));
    }
    Ok(events)
}

fn fourth_derivative_bound(radius: Real, vertical_radius: Real, k: Real) -> Real {
    let m = 1.0 - k * k;
    let root = m.sqrt();
    let f1 = 1.0 / root;
    let f2 = k / (m * root);
    let f3 = (1.0 + 2.0 * k * k) / (m * m * root);
    let f4 = k * (9.0 + 6.0 * k * k) / (m * m * m * root);
    let t1 = 2.0 * f1 * k;
    let t2 = 2.0 * (f2 * k * k + f1 * k);
    let t3 = 2.0 * (f3 * k.powi(3) + 3.0 * f2 * k * k + f1 * k);
    let t4 = 2.0 * (f4 * k.powi(4) + 6.0 * f3 * k.powi(3) + 7.0 * f2 * k * k + f1 * k);
    let horizontal =
        radius * (t4 + 4.0 * t1 * t3 + 3.0 * t2 * t2 + 6.0 * t1 * t1 * t2 + t1.powi(4));
    horizontal.hypot(vertical_radius)
}

fn active_intervals(center_z: Real, vertical_radius: Real, height: Real) -> Vec<(Real, Real)> {
    let turn = std::f64::consts::TAU;
    let mut angles = vec![0.0, turn];
    for boundary in [0.0, height] {
        let cosine = (boundary - center_z) / vertical_radius;
        if (-1.0..=1.0).contains(&cosine) {
            let angle = cosine.acos();
            angles.push(angle);
            angles.push(turn - angle);
        }
    }
    angles.sort_by(Real::total_cmp);
    angles.dedup_by(|left, right| (*left - *right).abs() <= 16.0 * Real::EPSILON);
    let mut intervals: Vec<(Real, Real)> = Vec::new();
    for pair in angles.windows(2) {
        let z = center_z + vertical_radius * (0.5 * (pair[0] + pair[1])).cos();
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
        && intervals.last().is_some_and(|last| last.1 == turn)
    {
        let first = intervals.remove(0);
        let last = intervals.pop().expect("at least two intervals");
        intervals.insert(0, (last.0 - turn, first.1));
    }
    intervals
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

#[allow(clippy::too_many_arguments)]
fn fit_curve(
    frame: Frame3,
    radius: Real,
    radial_axis: [Real; 2],
    center_z: Real,
    vertical_radius: Real,
    k: Real,
    start: Real,
    end: Real,
    fit_tolerance: Real,
    derivative_bound: Real,
) -> Result<NurbsCurve, GeometryError> {
    // The uniform cubic Hermite remainder is at most M h^4 / 384.
    let max_span = (384.0 * fit_tolerance / derivative_bound).powf(0.25);
    let required = ((end - start) / max_span).ceil();
    if !required.is_finite() || required > MAX_SEGMENTS as Real {
        return Err(GeometryError::TooManyCurveFitControlPoints {
            maximum: 3 * MAX_SEGMENTS + 1,
        });
    }
    let closed = (end - start - std::f64::consts::TAU).abs() <= 16.0 * Real::EPSILON;
    let segments = (required as usize).max(if closed { 4 } else { 1 });
    let mut controls = Vec::with_capacity(3 * segments + 1);
    let mut knots = Vec::with_capacity(3 * segments + 5);
    knots.extend([start; 4]);
    let (first_point, mut previous_tangent) = sample(
        frame,
        radius,
        radial_axis,
        center_z,
        vertical_radius,
        k,
        start,
    )?;
    controls.push(first_point);
    let mut previous_point = first_point;
    let mut previous_angle = start;
    for segment in 1..=segments {
        let angle = start + (end - start) * (segment as Real / segments as Real);
        let (mut point, tangent) = sample(
            frame,
            radius,
            radial_axis,
            center_z,
            vertical_radius,
            k,
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
fn sample(
    frame: Frame3,
    radius: Real,
    radial_axis: [Real; 2],
    center_z: Real,
    vertical_radius: Real,
    k: Real,
    angle: Real,
) -> Result<(Point3, Vector3), GeometryError> {
    let (sine, cosine) = angle.sin_cos();
    let theta = 2.0 * (k * sine).asin();
    let theta_tangent = 2.0 * k * cosine / (1.0 - k * k * sine * sine).sqrt();
    let (theta_sine, theta_cosine) = theta.sin_cos();
    let [axis_x, axis_y] = radial_axis;
    let x = radius * (theta_cosine * axis_x - theta_sine * axis_y);
    let y = radius * (theta_cosine * axis_y + theta_sine * axis_x);
    let z = center_z + vertical_radius * cosine;
    let derivative_x = -radius * (theta_sine * axis_x + theta_cosine * axis_y) * theta_tangent;
    let derivative_y = radius * (theta_cosine * axis_x - theta_sine * axis_y) * theta_tangent;
    let derivative_z = -vertical_radius * sine;
    Ok((
        frame.point_at([x, y, z])?,
        frame.vector_at([derivative_x, derivative_y, derivative_z])?,
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{NurbsSurface, surface_surface_intersection_events};

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
    fn turning_sphere_cylinder_intersection_is_one_closed_loop() {
        let center = point(2.0, 0.0, 2.5);
        let sphere = NurbsSurface::try_sphere(frame().with_origin(center), 2.5).unwrap();
        let cylinder = NurbsSurface::try_cylinder(frame(), 1.5, 0.0, 5.0).unwrap();
        let events =
            surface_surface_intersection_events(&sphere, &cylinder, Tolerance::DEFAULT).unwrap();
        assert_eq!(events.len(), 1);
        let SurfaceSurfaceIntersectionEvent::Curve(curve) = &events[0] else {
            panic!("turning intersection must be a closed curve")
        };
        assert_eq!(curve.degree(), 3);
        assert!(curve.is_closed().unwrap());
        let domain = curve.domain();
        for index in 0..=128 {
            let parameter =
                *domain.start() + (*domain.end() - *domain.start()) * (index as Real / 128.0);
            let sample = curve.evaluate(parameter).unwrap();
            assert!((sample.x().hypot(sample.y()) - 1.5).abs() < 2e-9);
            assert!((sample.distance_to(center).unwrap() - 2.5).abs() < 2e-9);
        }
    }

    #[test]
    fn turning_sphere_cylinder_clips_to_two_open_arcs() {
        let sphere =
            NurbsSurface::try_sphere(frame().with_origin(point(2.0, 0.0, 2.5)), 2.5).unwrap();
        let cylinder = NurbsSurface::try_cylinder(frame(), 1.5, 2.0, 3.0).unwrap();
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
                assert!((end.z() - 2.0).abs() < 1e-9 || (end.z() - 3.0).abs() < 1e-9);
            }
            let domain = curve.domain();
            for index in 0..=32 {
                let parameter =
                    *domain.start() + (*domain.end() - *domain.start()) * (index as Real / 32.0);
                let location = curve.evaluate(parameter).unwrap();
                assert!((2.0 - 1e-9..=3.0 + 1e-9).contains(&location.z()));
                assert!((location.x().hypot(location.y()) - 1.5).abs() < 2e-9);
            }
        }
    }

    #[test]
    fn external_radial_tangency_is_an_isolated_point() {
        let sphere =
            NurbsSurface::try_sphere(frame().with_origin(point(2.0, 0.0, 2.5)), 0.5).unwrap();
        let cylinder = NurbsSurface::try_cylinder(frame(), 1.5, 0.0, 5.0).unwrap();
        let events =
            surface_surface_intersection_events(&sphere, &cylinder, Tolerance::DEFAULT).unwrap();
        assert_eq!(events.len(), 1);
        let SurfaceSurfaceIntersectionEvent::Point(location) = events[0] else {
            panic!("radial tangency must be an isolated point")
        };
        assert!(location.distance_to(point(1.5, 0.0, 2.5)).unwrap() < 1e-9);
    }
}
