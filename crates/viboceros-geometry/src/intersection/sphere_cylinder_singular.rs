//! Exact rational quartic intersection at the offset sphere/cylinder crossing.
//!
//! When `Qmin = 0`, `θ = 2t` and `z = z₀ + sqrt(Qmax) cos t` trace
//! a closed figure-eight curve. Setting `u = tan(t/2)/(1 + tan(t/2))` on
//! each half-turn makes every coordinate rational with a quartic denominator.
//! Each half-turn is split once so all Bernstein weights stay positive.

use super::SurfaceSurfaceIntersectionEvent;
use crate::{Frame3, GeometryError, NurbsCurve, Real, Tolerance, WeightedPoint3};

// Bernstein coefficients of D², cos(2t)D², sin(2t)D², and cos(t)D²,
// where D = (1-u)² + u². Rows cover u in [0, 1/2] and [1/2, 1].
const WEIGHTS: [[Real; 5]; 2] = [
    [1.0, 0.5, 1.0 / 3.0, 0.25, 0.25],
    [0.25, 0.25, 1.0 / 3.0, 0.5, 1.0],
];
const COS_DOUBLE: [[Real; 5]; 2] = [[1.0, 0.5, 0.0, -0.25, -0.25], [-0.25, -0.25, 0.0, 0.5, 1.0]];
const SIN_DOUBLE: [[Real; 5]; 2] = [[0.0, 0.5, 0.5, 0.25, 0.0], [0.0, -0.25, -0.5, -0.5, 0.0]];
const COS_SINGLE: [[Real; 5]; 2] = [
    [1.0, 0.5, 0.25, 0.125, 0.0],
    [0.0, -0.125, -0.25, -0.5, -1.0],
];

pub(super) fn intersect(
    frame: Frame3,
    radius: Real,
    height: Real,
    radial_axis: [Real; 2],
    center_z: Real,
    vertical_radius: Real,
    tolerance: Tolerance,
) -> Result<Vec<SurfaceSurfaceIntersectionEvent>, GeometryError> {
    let curve = exact_crossing_curve(frame, radius, radial_axis, center_z, vertical_radius)?;
    let intervals = active_intervals(center_z, vertical_radius, height);
    let coordinate_scale = frame
        .origin()
        .to_array()
        .into_iter()
        .map(Real::abs)
        .fold(0.0, Real::max);
    let rim_tolerance = tolerance
        .absolute()
        .max(tolerance.relative() * radius.max(height).max(vertical_radius))
        .max(8.0 * Real::EPSILON * coordinate_scale);
    let mut events = Vec::new();
    for parameter in [0.0, 2.0] {
        let z = center_z + vertical_radius * if parameter == 0.0 { 1.0 } else { -1.0 };
        if (z.abs() <= rim_tolerance || (z - height).abs() <= rim_tolerance)
            && !parameter_in_intervals(parameter, &intervals)
        {
            events.push(SurfaceSurfaceIntersectionEvent::Point(
                curve.evaluate(parameter)?,
            ));
        }
    }
    for (start, end) in intervals {
        events.push(SurfaceSurfaceIntersectionEvent::Curve(
            if start == 0.0 && end == 4.0 {
                curve.clone()
            } else {
                curve.try_subcurve(start, end)?
            },
        ));
    }
    Ok(events)
}

fn exact_crossing_curve(
    frame: Frame3,
    radius: Real,
    radial_axis: [Real; 2],
    center_z: Real,
    vertical_radius: Real,
) -> Result<NurbsCurve, GeometryError> {
    let mut controls = Vec::with_capacity(17);
    let mut knots = vec![0.0; 5];
    for half in 0..2 {
        for quarter in 0..2 {
            for index in 0..5 {
                if !controls.is_empty() && index == 0 {
                    continue;
                }
                let weight = WEIGHTS[quarter][index];
                let cosine = COS_DOUBLE[quarter][index] / weight;
                let sine = SIN_DOUBLE[quarter][index] / weight;
                let sign = if half == 0 { 1.0 } else { -1.0 };
                let z = center_z + sign * vertical_radius * COS_SINGLE[quarter][index] / weight;
                let [axis_x, axis_y] = radial_axis;
                let point = frame.point_at([
                    radius * (cosine * axis_x - sine * axis_y),
                    radius * (cosine * axis_y + sine * axis_x),
                    z,
                ])?;
                controls.push(WeightedPoint3::try_new(point, weight)?);
            }
            let knot = (2 * half + quarter + 1) as Real;
            let multiplicity = if half == 1 && quarter == 1 { 5 } else { 4 };
            knots.extend(std::iter::repeat_n(knot, multiplicity));
        }
    }
    *controls.last_mut().expect("four spans have controls") = controls[0];
    NurbsCurve::try_new_rational(4, controls, knots)
}

fn active_intervals(center_z: Real, vertical_radius: Real, height: Real) -> Vec<(Real, Real)> {
    let mut parameters = vec![0.0, 4.0];
    for boundary in [0.0, height] {
        let cosine = (boundary - center_z) / vertical_radius;
        if (-1.0..=1.0).contains(&cosine) {
            let sine_half = ((1.0 - cosine) / 2.0).sqrt();
            let cosine_half = ((1.0 + cosine) / 2.0).sqrt();
            let u = sine_half / (sine_half + cosine_half);
            parameters.push(2.0 * u);
            parameters.push(4.0 - 2.0 * u);
        }
    }
    parameters.sort_by(Real::total_cmp);
    parameters.dedup_by(|left, right| (*left - *right).abs() <= 16.0 * Real::EPSILON);
    let mut intervals: Vec<(Real, Real)> = Vec::new();
    for pair in parameters.windows(2) {
        let midpoint = 0.5 * (pair[0] + pair[1]);
        let u = if midpoint < 2.0 {
            midpoint / 2.0
        } else {
            (midpoint - 2.0) / 2.0
        };
        let numerator = 1.0 - 2.0 * u;
        let denominator = (1.0 - u).powi(2) + u * u;
        let cosine = if midpoint < 2.0 {
            numerator / denominator
        } else {
            -numerator / denominator
        };
        let z = center_z + vertical_radius * cosine;
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
        && intervals.last().is_some_and(|last| last.1 == 4.0)
    {
        let first = intervals.remove(0);
        let last = intervals.pop().expect("at least two intervals");
        intervals.insert(0, (last.0, first.1));
    }
    intervals
}

fn parameter_in_intervals(parameter: Real, intervals: &[(Real, Real)]) -> bool {
    intervals.iter().any(|(start, end)| {
        if start <= end {
            (*start..=*end).contains(&parameter)
        } else {
            parameter >= *start || parameter <= *end
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{NurbsSurface, Point3, Tolerance, Vector3, surface_surface_intersection_events};

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

    fn sphere() -> NurbsSurface {
        NurbsSurface::try_sphere(frame().with_origin(point(1.0, 0.0, 2.5)), 2.0).unwrap()
    }

    #[test]
    fn singular_sphere_cylinder_intersection_is_exact_closed_figure_eight() {
        let cylinder = NurbsSurface::try_cylinder(frame(), 1.0, 0.0, 5.0).unwrap();
        let events =
            surface_surface_intersection_events(&sphere(), &cylinder, Tolerance::DEFAULT).unwrap();
        assert_eq!(events.len(), 1);
        let SurfaceSurfaceIntersectionEvent::Curve(curve) = &events[0] else {
            panic!("singular section must be a curve")
        };
        assert_eq!(curve.degree(), 4);
        assert!(curve.is_rational());
        assert!(curve.is_closed().unwrap());
        assert!(
            curve
                .evaluate(1.0)
                .unwrap()
                .distance_to(curve.evaluate(3.0).unwrap())
                .unwrap()
                < 1e-13
        );
        assert!(
            curve
                .evaluate(1.0)
                .unwrap()
                .distance_to(point(-1.0, 0.0, 2.5))
                .unwrap()
                < 1e-13
        );
        for index in 0..=256 {
            let parameter = 4.0 * (index as Real / 256.0);
            let sample = curve.evaluate(parameter).unwrap();
            assert!((sample.x().hypot(sample.y()) - 1.0).abs() < 1e-12);
            assert!((sample.distance_to(point(1.0, 0.0, 2.5)).unwrap() - 2.0).abs() < 1e-12);
        }
    }

    #[test]
    fn singular_sphere_cylinder_section_trims_to_two_exact_arcs() {
        let cylinder = NurbsSurface::try_cylinder(frame(), 1.0, 2.0, 3.0).unwrap();
        let events =
            surface_surface_intersection_events(&sphere(), &cylinder, Tolerance::DEFAULT).unwrap();
        assert_eq!(events.len(), 2);
        for event in events {
            let SurfaceSurfaceIntersectionEvent::Curve(curve) = event else {
                panic!("height band must leave two curves")
            };
            assert_eq!(curve.degree(), 4);
            assert!(curve.is_rational());
            assert!(!curve.is_closed().unwrap());
            let domain = curve.domain();
            for index in 0..=32 {
                let parameter =
                    *domain.start() + (*domain.end() - *domain.start()) * (index as Real / 32.0);
                let location = curve.evaluate(parameter).unwrap();
                assert!((2.0 - 1e-12..=3.0 + 1e-12).contains(&location.z()));
                assert!((location.distance_to(point(1.0, 0.0, 2.5)).unwrap() - 2.0).abs() < 1e-11);
            }
        }
    }

    #[test]
    fn singular_sphere_cylinder_section_keeps_isolated_rim_contact() {
        let cylinder = NurbsSurface::try_cylinder(frame(), 1.0, 4.5, 5.0).unwrap();
        let events =
            surface_surface_intersection_events(&sphere(), &cylinder, Tolerance::DEFAULT).unwrap();
        assert_eq!(events.len(), 1);
        let SurfaceSurfaceIntersectionEvent::Point(location) = events[0] else {
            panic!("isolated rim touch must be a point")
        };
        assert!(location.distance_to(point(1.0, 0.0, 4.5)).unwrap() < 1e-12);
    }

    #[test]
    fn singular_sphere_cylinder_section_trims_across_curve_seam() {
        let cylinder = NurbsSurface::try_cylinder(frame(), 1.0, 3.0, 5.0).unwrap();
        let events =
            surface_surface_intersection_events(&sphere(), &cylinder, Tolerance::DEFAULT).unwrap();
        assert_eq!(events.len(), 1);
        let SurfaceSurfaceIntersectionEvent::Curve(curve) = &events[0] else {
            panic!("upper band must leave one seam-spanning arc")
        };
        assert!(!curve.is_closed().unwrap());
        let domain = curve.domain();
        for index in 0..=64 {
            let parameter =
                *domain.start() + (*domain.end() - *domain.start()) * (index as Real / 64.0);
            let location = curve.evaluate(parameter).unwrap();
            assert!((3.0 - 1e-12..=5.0 + 1e-12).contains(&location.z()));
            assert!((location.distance_to(point(1.0, 0.0, 2.5)).unwrap() - 2.0).abs() < 1e-11);
        }
    }
}
