//! Sphere/cone sections when the sphere passes through the cone apex.
//!
//! The generator equation factors into the apex root and the analytic root
//! t = 2(z₀ + k d cos θ)/(1+k²). Only the positive part of that second root
//! belongs to the finite cone. An apex-only contact produces no event.

use super::SurfaceSurfaceIntersectionEvent;
use crate::{Frame3, GeometryError, NurbsCurve, Point3, Real, Tolerance, Vector3};

const MAX_SEGMENTS: usize = 4096;
const TURN: Real = std::f64::consts::TAU;
const PI: Real = std::f64::consts::PI;

#[derive(Clone, Copy)]
struct Basis {
    frame: Frame3,
    slope: Real,
    axial_sign: Real,
    direction: [Real; 2],
    center_axial: Real,
    cosine_coefficient: Real,
    quadratic: Real,
}

pub(super) fn intersect(
    sphere_center: Point3,
    sphere_radius: Real,
    (cone_frame, cone_radius, signed_height): (Frame3, Real, Real),
    tolerance: Tolerance,
    coordinate_roundoff: Real,
) -> Result<Vec<SurfaceSurfaceIntersectionEvent>, GeometryError> {
    let [center_x, center_y, center_z] = cone_frame.coordinates_of(sphere_center)?;
    let offset = center_x.hypot(center_y);
    let height = signed_height.abs();
    let slope = cone_radius / height;
    let basis = Basis {
        frame: cone_frame,
        slope,
        axial_sign: signed_height.signum(),
        direction: [center_x / offset, center_y / offset],
        center_axial: center_z * signed_height.signum(),
        cosine_coefficient: slope * offset,
        quadratic: slope.mul_add(slope, 1.0),
    };
    let fit_tolerance = tolerance
        .absolute()
        .max(tolerance.relative() * cone_radius.max(height).max(sphere_radius))
        .max(coordinate_roundoff);
    if !fit_tolerance.is_finite() {
        return Err(GeometryError::UnsupportedSurfaceSurfaceIntersection {
            context: "apex sphere/cone fit tolerance is not finite",
        });
    }
    if basis.axial(0.0) <= fit_tolerance {
        return Ok(Vec::new());
    }
    let derivative_bound = fourth_derivative_bound(basis);
    if !derivative_bound.is_finite() {
        return Err(GeometryError::UnsupportedSurfaceSurfaceIntersection {
            context: "apex sphere/cone derivative bound is not finite",
        });
    }
    let cuts = angular_cuts(basis, height);
    let intervals = active_intervals(basis, &cuts, height);
    let mut events = Vec::new();
    for angle in cuts {
        let axial = basis.axial(angle);
        if axial > fit_tolerance
            && (axial - height).abs() <= fit_tolerance
            && !angle_in_intervals(angle, &intervals)
        {
            let point = basis.sample(angle)?.0;
            if !events.iter().any(|event| {
                matches!(event, SurfaceSurfaceIntersectionEvent::Point(existing)
                    if existing.distance_to(point).is_ok_and(|distance| distance <= fit_tolerance))
            }) {
                events.push(SurfaceSurfaceIntersectionEvent::Point(point));
            }
        }
    }
    for (start, end) in intervals {
        events.push(SurfaceSurfaceIntersectionEvent::Curve(fit_curve(
            basis,
            start,
            end,
            fit_tolerance,
            derivative_bound,
        )?));
    }
    Ok(events)
}

impl Basis {
    fn axial(self, angle: Real) -> Real {
        2.0 * self
            .cosine_coefficient
            .mul_add(angle.cos(), self.center_axial)
            / self.quadratic
    }

    fn sample(self, angle: Real) -> Result<(Point3, Vector3), GeometryError> {
        let (sine, cosine) = angle.sin_cos();
        let axial =
            2.0 * self.cosine_coefficient.mul_add(cosine, self.center_axial) / self.quadratic;
        let axial_derivative = -2.0 * self.cosine_coefficient * sine / self.quadratic;
        let radial = self.slope * axial;
        let radial_derivative = self.slope * axial_derivative;
        let [ux, uy] = self.direction;
        let local = [
            radial * cosine.mul_add(ux, -sine * uy),
            radial * cosine.mul_add(uy, sine * ux),
            self.axial_sign * axial,
        ];
        let derivative = [
            radial_derivative * cosine.mul_add(ux, -sine * uy)
                + radial * (-sine * ux - cosine * uy),
            radial_derivative * cosine.mul_add(uy, sine * ux) + radial * (-sine * uy + cosine * ux),
            self.axial_sign * axial_derivative,
        ];
        let scale = (self.center_axial.abs() + self.cosine_coefficient.abs()) / self.quadratic;
        let point = if axial.abs() <= 64.0 * Real::EPSILON * scale {
            self.frame.origin()
        } else {
            self.frame.point_at(local)?
        };
        Ok((point, self.frame.vector_at(derivative)?))
    }
}

fn fourth_derivative_bound(basis: Basis) -> Real {
    let t0 = 2.0 * (basis.center_axial.abs() + basis.cosine_coefficient) / basis.quadratic;
    let t1 = 2.0 * basis.cosine_coefficient / basis.quadratic;
    (basis.slope * (t0 + 15.0 * t1)).hypot(t1)
}

fn angular_cuts(basis: Basis, height: Real) -> Vec<Real> {
    let mut cuts = vec![0.0, PI, TURN];
    for axial in [0.0, height] {
        let cosine =
            (0.5 * basis.quadratic * axial - basis.center_axial) / basis.cosine_coefficient;
        if (-1.0..=1.0).contains(&cosine) {
            let angle = cosine.acos();
            cuts.push(angle);
            cuts.push(TURN - angle);
        }
    }
    cuts.sort_by(Real::total_cmp);
    cuts.dedup_by(|left, right| (*left - *right).abs() <= 32.0 * Real::EPSILON);
    cuts
}

fn active_intervals(basis: Basis, cuts: &[Real], height: Real) -> Vec<(Real, Real)> {
    let mut intervals: Vec<(Real, Real)> = Vec::new();
    for pair in cuts.windows(2) {
        let axial = basis.axial(0.5 * (pair[0] + pair[1]));
        if (0.0..=height).contains(&axial) {
            if let Some(last) = intervals.last_mut()
                && (last.1 - pair[0]).abs() <= 32.0 * Real::EPSILON
            {
                last.1 = pair[1];
            } else {
                intervals.push((pair[0], pair[1]));
            }
        }
    }
    if intervals.len() > 1
        && intervals[0].0 == 0.0
        && intervals.last().is_some_and(|last| last.1 == TURN)
    {
        let first = intervals.remove(0);
        let last = intervals.pop().expect("at least two intervals");
        intervals.insert(0, (last.0 - TURN, first.1));
    }
    intervals
}

fn angle_in_intervals(angle: Real, intervals: &[(Real, Real)]) -> bool {
    [-TURN, 0.0, TURN].into_iter().any(|shift| {
        intervals.iter().any(|(start, end)| {
            angle + shift >= *start - 32.0 * Real::EPSILON
                && angle + shift <= *end + 32.0 * Real::EPSILON
        })
    })
}

fn fit_curve(
    basis: Basis,
    start: Real,
    end: Real,
    fit_tolerance: Real,
    derivative_bound: Real,
) -> Result<NurbsCurve, GeometryError> {
    let max_span = (384.0 * fit_tolerance / derivative_bound).powf(0.25);
    if !max_span.is_finite() || max_span <= 0.0 {
        return Err(GeometryError::TooManyCurveFitControlPoints {
            maximum: 3 * MAX_SEGMENTS + 1,
        });
    }
    let mut breaks = vec![start];
    for node in [-PI, PI, 3.0 * PI] {
        if node > start + 32.0 * Real::EPSILON && node < end - 32.0 * Real::EPSILON {
            breaks.push(node);
        }
    }
    breaks.push(end);
    breaks.sort_by(Real::total_cmp);
    let mut counts = Vec::with_capacity(breaks.len() - 1);
    let mut segments = 0usize;
    for pair in breaks.windows(2) {
        let required = ((pair[1] - pair[0]) / max_span).ceil();
        if !required.is_finite() || required > (MAX_SEGMENTS - segments) as Real {
            return Err(GeometryError::TooManyCurveFitControlPoints {
                maximum: 3 * MAX_SEGMENTS + 1,
            });
        }
        let count = (required as usize).max(1);
        if count > MAX_SEGMENTS - segments {
            return Err(GeometryError::TooManyCurveFitControlPoints {
                maximum: 3 * MAX_SEGMENTS + 1,
            });
        }
        segments += count;
        counts.push(count);
    }
    let closed = (end - start - TURN).abs() <= 32.0 * Real::EPSILON
        || (basis.axial(start).abs() <= fit_tolerance && basis.axial(end).abs() <= fit_tolerance);
    let mut controls = Vec::with_capacity(3 * segments + 1);
    let mut knots = Vec::with_capacity(3 * segments + 5);
    knots.extend([start; 4]);
    let (first_point, mut previous_tangent) = basis.sample(start)?;
    controls.push(first_point);
    let mut previous_point = first_point;
    let mut previous_angle = start;
    for (span, &count) in breaks.windows(2).zip(&counts) {
        for segment in 1..=count {
            let angle = if segment == count {
                span[1]
            } else {
                span[0] + (span[1] - span[0]) * (segment as Real / count as Real)
            };
            let (mut point, tangent) = basis.sample(angle)?;
            if closed && angle == end {
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
    }
    knots.push(end);
    NurbsCurve::try_new(3, controls, knots)
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

    fn cone(height: Real) -> NurbsSurface {
        NurbsSurface::try_cone(frame(), 0.75 * height, height).unwrap()
    }

    fn sphere(center: Point3, radius: Real) -> NurbsSurface {
        NurbsSurface::try_sphere(frame().with_origin(center), radius).unwrap()
    }

    fn assert_on_walls(location: Point3, center: Point3, radius: Real, height: Real) {
        assert!((location.z() - 4.0 * location.x().hypot(location.y()) / 3.0).abs() < 5e-9);
        assert!((location.distance_to(center).unwrap() - radius).abs() < 5e-9);
        assert!((-5e-9..=height + 5e-9).contains(&location.z()));
    }

    #[test]
    fn sphere_on_cone_apex_makes_closed_loop_away_from_apex() {
        let center = point(0.5, 0.0, 2.0);
        let radius = 4.25_f64.sqrt();
        let sphere = sphere(center, radius);
        let cone = cone(4.0);
        for (first, second) in [(&sphere, &cone), (&cone, &sphere)] {
            let events =
                surface_surface_intersection_events(first, second, Tolerance::DEFAULT).unwrap();
            let [SurfaceSurfaceIntersectionEvent::Curve(curve)] = events.as_slice() else {
                panic!("sphere through apex must produce one loop, got {events:#?}")
            };
            assert_eq!(curve.degree(), 3);
            assert!(curve.is_closed().unwrap());
            for index in 0..=128 {
                assert_on_walls(
                    curve.evaluate(TURN * (index as Real / 128.0)).unwrap(),
                    center,
                    radius,
                    4.0,
                );
            }
        }
    }

    #[test]
    fn sphere_on_cone_apex_keeps_an_isolated_base_rim_contact() {
        let center = point(0.5, 0.0, 2.0);
        let radius = 4.25_f64.sqrt();
        let minimum_height = 2.0 * (2.0 - 0.75 * 0.5) / (1.0 + 0.75 * 0.75);
        let events = surface_surface_intersection_events(
            &sphere(center, radius),
            &cone(minimum_height),
            Tolerance::DEFAULT,
        )
        .unwrap();
        let [SurfaceSurfaceIntersectionEvent::Point(contact)] = events.as_slice() else {
            panic!("cone base must touch the lower radial turn once, got {events:#?}")
        };
        assert_on_walls(*contact, center, radius, minimum_height);
        assert!((contact.z() - minimum_height).abs() < 5e-9);
    }

    #[test]
    fn sphere_on_cone_apex_can_close_at_apex_or_clip_to_two_arcs() {
        let center = point(2.0, 0.0, 0.5);
        let radius = 4.25_f64.sqrt();
        let sphere = sphere(center, radius);
        let events =
            surface_surface_intersection_events(&sphere, &cone(4.0), Tolerance::DEFAULT).unwrap();
        let [SurfaceSurfaceIntersectionEvent::Curve(loop_curve)] = events.as_slice() else {
            panic!("generator section must close at the apex, got {events:#?}")
        };
        assert!(loop_curve.is_closed().unwrap());
        let domain = loop_curve.domain();
        assert!(
            loop_curve
                .evaluate(*domain.start())
                .unwrap()
                .distance_to(frame().origin())
                .unwrap()
                < 1e-12
        );
        assert!(
            loop_curve
                .evaluate(*domain.end())
                .unwrap()
                .distance_to(frame().origin())
                .unwrap()
                < 1e-12
        );
        for index in 0..=64 {
            let angle =
                *domain.start() + (*domain.end() - *domain.start()) * (index as Real / 64.0);
            assert_on_walls(loop_curve.evaluate(angle).unwrap(), center, radius, 4.0);
        }

        let events =
            surface_surface_intersection_events(&sphere, &cone(1.5), Tolerance::DEFAULT).unwrap();
        assert_eq!(events.len(), 2);
        for event in events {
            let SurfaceSurfaceIntersectionEvent::Curve(arc) = event else {
                panic!("finite cone must leave two apex-ending arcs")
            };
            assert!(!arc.is_closed().unwrap());
            let domain = arc.domain();
            let ends = [
                arc.evaluate(*domain.start()).unwrap(),
                arc.evaluate(*domain.end()).unwrap(),
            ];
            assert!(
                ends.iter()
                    .any(|location| location.distance_to(frame().origin()).unwrap() < 1e-12)
            );
            assert!(
                ends.iter()
                    .any(|location| (location.z() - 1.5).abs() < 5e-9)
            );
            for index in 0..=32 {
                let angle =
                    *domain.start() + (*domain.end() - *domain.start()) * (index as Real / 32.0);
                assert_on_walls(arc.evaluate(angle).unwrap(), center, radius, 1.5);
            }
        }
    }

    #[test]
    fn sphere_on_cone_apex_handles_cusp_and_apex_only_contact() {
        let center = point(2.0, 0.0, 1.5);
        let cusp_sphere = sphere(center, 2.5);
        let events =
            surface_surface_intersection_events(&cusp_sphere, &cone(4.0), Tolerance::DEFAULT)
                .unwrap();
        let [SurfaceSurfaceIntersectionEvent::Curve(curve)] = events.as_slice() else {
            panic!("apex cusp must remain on the intersection curve")
        };
        assert!(curve.is_closed().unwrap());
        assert!(
            curve
                .evaluate(PI)
                .unwrap()
                .distance_to(frame().origin())
                .unwrap()
                < 1e-12
        );
        for index in 0..=64 {
            assert_on_walls(
                curve.evaluate(TURN * (index as Real / 64.0)).unwrap(),
                center,
                2.5,
                4.0,
            );
        }
        let apex_only = sphere(point(0.5, 0.0, -2.0), 4.25_f64.sqrt());
        assert!(
            surface_surface_intersection_events(&apex_only, &cone(4.0), Tolerance::DEFAULT)
                .unwrap()
                .is_empty()
        );
    }

    #[test]
    fn sphere_on_cone_apex_handles_negative_and_distant_rotated_frames() {
        let radius = 4.25_f64.sqrt();
        let negative = NurbsSurface::try_cone(frame(), 3.0, -4.0).unwrap();
        let negative_sphere = sphere(point(0.5, 0.0, -2.0), radius);
        let events =
            surface_surface_intersection_events(&negative_sphere, &negative, Tolerance::DEFAULT)
                .unwrap();
        let [SurfaceSurfaceIntersectionEvent::Curve(curve)] = events.as_slice() else {
            panic!("negative cone height must keep one loop")
        };
        assert!(curve.is_closed().unwrap());
        for index in 0..=64 {
            let location = curve.evaluate(TURN * (index as Real / 64.0)).unwrap();
            assert!((location.z() + 4.0 * location.x().hypot(location.y()) / 3.0).abs() < 5e-9);
            assert!((location.distance_to(point(0.5, 0.0, -2.0)).unwrap() - radius).abs() < 5e-9);
        }

        let rotated = Frame3::try_from_normal(
            point(1.0e8, -1.0e8, 1.0e8),
            Vector3::try_new(1.0, 2.0, 3.0).unwrap(),
            Tolerance::DEFAULT,
        )
        .unwrap();
        let center = rotated.point_at([0.3, 0.4, 2.0]).unwrap();
        let cone = NurbsSurface::try_cone(rotated, 3.0, 4.0).unwrap();
        let sphere = NurbsSurface::try_sphere(rotated.with_origin(center), radius).unwrap();
        for (first, second) in [(&sphere, &cone), (&cone, &sphere)] {
            let events =
                surface_surface_intersection_events(first, second, Tolerance::DEFAULT).unwrap();
            let [SurfaceSurfaceIntersectionEvent::Curve(curve)] = events.as_slice() else {
                panic!("distant rotated sphere/cone must keep one loop")
            };
            for index in 0..=64 {
                let location = curve.evaluate(TURN * (index as Real / 64.0)).unwrap();
                let local = rotated.coordinates_of(location).unwrap();
                assert!((local[2] - 4.0 * local[0].hypot(local[1]) / 3.0).abs() < 4e-7);
                assert!((location.distance_to(center).unwrap() - radius).abs() < 4e-7);
            }
        }
    }
}
