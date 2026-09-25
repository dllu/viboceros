//! The nodal quartic at an internal skew-cylinder axis tangency.
//!
//! At axis separation R-r, the wall radical has a double zero at one small-
//! cylinder angle. One smooth parametrization traverses both radical signs
//! over two turns, passing through the node halfway around the closed curve.

use super::{SurfaceSurfaceIntersectionEvent, skew_clip};
use crate::{GeometryError, NurbsCurve, Point3, Real, Tolerance, Vector3};

const MAX_SEGMENTS: usize = 4096;
const TURN: Real = std::f64::consts::TAU;
const PERIOD: Real = 2.0 * TURN;
type CylinderAxis = (Vector3, Real, Real, Real, Point3);

#[derive(Clone, Copy)]
struct Basis {
    closest: Point3,
    small_axis: Vector3,
    plane_axis: Vector3,
    normal: Vector3,
    cosine: Real,
    sine: Real,
    small: Real,
    big: Real,
    miss: Real,
    miss_sign: Real,
}

#[derive(Clone, Copy)]
struct Clip {
    small_center: Real,
    small_height: Real,
    big_center: Real,
    big_height: Real,
}

struct AngularClip {
    intervals: Vec<(Real, Real)>,
    cuts: Vec<Real>,
}

pub(super) fn intersect(
    first: CylinderAxis,
    second: CylinderAxis,
    miss: Real,
    tolerance: Tolerance,
    coordinate_roundoff: Real,
) -> Result<Vec<SurfaceSurfaceIntersectionEvent>, GeometryError> {
    let (
        small_axis,
        small,
        small_height,
        small_center,
        small_closest,
        big_axis,
        big,
        big_height,
        big_center,
    ) = if first.1 <= second.1 {
        (
            first.0, first.1, first.2, first.3, first.4, second.0, second.1, second.2, second.3,
        )
    } else {
        (
            second.0, second.1, second.2, second.3, second.4, first.0, first.1, first.2, first.3,
        )
    };
    let cosine = small_axis.dot(big_axis)?;
    let cross = small_axis.cross(big_axis)?;
    let sine = cross.length()?;
    if sine <= tolerance.angular() {
        return Err(GeometryError::UnsupportedSurfaceSurfaceIntersection {
            context: "nearly parallel pinched skew cylinder axes",
        });
    }
    let small_vector = small_axis.to_array();
    let big_vector = big_axis.to_array();
    let plane_axis = Vector3::try_new(
        (big_vector[0] - cosine * small_vector[0]) / sine,
        (big_vector[1] - cosine * small_vector[1]) / sine,
        (big_vector[2] - cosine * small_vector[2]) / sine,
    )?;
    let basis = Basis {
        closest: small_closest,
        small_axis,
        plane_axis,
        normal: cross.scaled(1.0 / sine)?,
        cosine,
        sine,
        small,
        big,
        miss: miss.signum() * (big - small),
        miss_sign: miss.signum(),
    };
    let clip = Clip {
        small_center,
        small_height,
        big_center,
        big_height,
    };
    let fit_tolerance = tolerance
        .absolute()
        .max(tolerance.relative() * big.max(small_height).max(big_height))
        .max(coordinate_roundoff);
    let derivative_bound = fourth_derivative_bound(basis);
    if !derivative_bound.is_finite() {
        return Err(GeometryError::UnsupportedSurfaceSurfaceIntersection {
            context: "pinched skew cylinder fit is ill-conditioned",
        });
    }
    let AngularClip { intervals, cuts } = active_intervals(basis, clip, fit_tolerance)?;
    let mut events = Vec::new();
    for parameter in cuts {
        let (small_axial, big_axial) = basis.axials(parameter);
        let small_axial = small_center + small_axial;
        let big_axial = big_center + big_axial;
        let inside = (-fit_tolerance..=small_height + fit_tolerance).contains(&small_axial)
            && (-fit_tolerance..=big_height + fit_tolerance).contains(&big_axial);
        let on_rim = small_axial.abs() <= fit_tolerance
            || (small_axial - small_height).abs() <= fit_tolerance
            || big_axial.abs() <= fit_tolerance
            || (big_axial - big_height).abs() <= fit_tolerance;
        if inside && on_rim && !angle_in_intervals(parameter, &intervals) {
            let point = basis.sample(parameter)?.0;
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
    fn coordinates(self, parameter: Real) -> (Real, Real, Real, Real, Real) {
        let phi = parameter - std::f64::consts::FRAC_PI_2;
        let y = self.small * phi.cos();
        let z = self.miss_sign * self.small * phi.sin();
        let factor =
            (2.0 * self.small * (self.big + self.miss.abs() - self.small * phi.sin())).sqrt();
        let w = (0.5 * parameter).sin() * factor;
        let x = (self.cosine * y - w) / self.sine;
        (x, y, z, w, factor)
    }

    fn axials(self, parameter: Real) -> (Real, Real) {
        let (x, y, _, w, _) = self.coordinates(parameter);
        (x, (y - self.cosine * w) / self.sine)
    }

    fn sample(self, parameter: Real) -> Result<(Point3, Vector3), GeometryError> {
        let phi = parameter - std::f64::consts::FRAC_PI_2;
        let (x, y, z, _, factor) = self.coordinates(parameter);
        let derivative_y = -self.small * phi.sin();
        let derivative_z = self.miss_sign * self.small * phi.cos();
        let derivative_factor = -self.small * self.small * phi.cos() / factor;
        let derivative_w =
            0.5 * (0.5 * parameter).cos() * factor + (0.5 * parameter).sin() * derivative_factor;
        let derivative_x = (self.cosine * derivative_y - derivative_w) / self.sine;
        let point = self
            .closest
            .translated(self.small_axis.scaled(x)?)?
            .translated(self.plane_axis.scaled(y)?)?
            .translated(self.normal.scaled(z)?)?;
        let a = self.small_axis.to_array();
        let e = self.plane_axis.to_array();
        let n = self.normal.to_array();
        let tangent = Vector3::try_new(
            a[0] * derivative_x + e[0] * derivative_y + n[0] * derivative_z,
            a[1] * derivative_x + e[1] * derivative_y + n[1] * derivative_z,
            a[2] * derivative_x + e[2] * derivative_y + n[2] * derivative_z,
        )?;
        Ok((point, tangent))
    }
}

fn fourth_derivative_bound(basis: Basis) -> Real {
    let scale = 2.0 * basis.small;
    let lower = scale * (basis.big + basis.miss.abs() - basis.small);
    let upper = scale * (basis.big + basis.miss.abs() + basis.small);
    let derivative = scale * basis.small;
    let root = lower.sqrt();
    let lower3 = lower * root;
    let lower5 = lower * lower3;
    let lower7 = lower * lower5;
    let g0 = upper.sqrt();
    let g1 = derivative / (2.0 * root);
    let g2 = derivative / (2.0 * root) + derivative * derivative / (4.0 * lower3);
    let g3 = derivative / (2.0 * root)
        + 0.75 * derivative * derivative / lower3
        + 0.375 * derivative.powi(3) / lower5;
    let g4 = derivative / (2.0 * root)
        + 1.75 * derivative * derivative / lower3
        + 2.25 * derivative.powi(3) / lower5
        + 0.9375 * derivative.powi(4) / lower7;
    let w4 = g4 + 2.0 * g3 + 1.5 * g2 + 0.5 * g1 + g0 / 16.0;
    2.0 * basis.small + (basis.cosine.abs() * basis.small + w4) / basis.sine
}

fn active_intervals(
    basis: Basis,
    clip: Clip,
    fit_tolerance: Real,
) -> Result<AngularClip, GeometryError> {
    let mut angles = (0..=8)
        .map(|index| PERIOD * index as Real / 8.0)
        .collect::<Vec<_>>();
    let coefficients = [
        (
            (basis.cosine * basis.small / basis.sine, -1.0 / basis.sine),
            clip.small_center,
            clip.small_height,
        ),
        (
            (basis.small / basis.sine, -basis.cosine / basis.sine),
            clip.big_center,
            clip.big_height,
        ),
    ];
    for ((linear, root), center, height) in coefficients {
        for boundary in [0.0, height] {
            for sign in [1.0, -1.0] {
                for angle in skew_clip::boundary_angles(
                    (basis.small, basis.big, basis.miss),
                    (linear, sign * root),
                    boundary - center,
                    fit_tolerance,
                )? {
                    let phase =
                        (basis.miss_sign * angle + std::f64::consts::FRAC_PI_2).rem_euclid(TURN);
                    angles.push(if sign > 0.0 { phase } else { phase + TURN });
                }
            }
        }
    }
    angles.sort_by(Real::total_cmp);
    angles.dedup_by(|left, right| (*left - *right).abs() <= 32.0 * Real::EPSILON);
    let mut intervals: Vec<(Real, Real)> = Vec::new();
    for pair in angles.windows(2) {
        let (small_axial, big_axial) = basis.axials(0.5 * (pair[0] + pair[1]));
        if (0.0..=clip.small_height).contains(&(clip.small_center + small_axial))
            && (0.0..=clip.big_height).contains(&(clip.big_center + big_axial))
        {
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
        && intervals.last().is_some_and(|last| last.1 == PERIOD)
    {
        let first = intervals.remove(0);
        let last = intervals.pop().expect("at least two intervals");
        intervals.insert(0, (last.0 - PERIOD, first.1));
    }
    Ok(AngularClip {
        intervals,
        cuts: angles,
    })
}

fn angle_in_intervals(parameter: Real, intervals: &[(Real, Real)]) -> bool {
    [-PERIOD, 0.0, PERIOD].into_iter().any(|shift| {
        intervals.iter().any(|(start, end)| {
            parameter + shift >= *start - 32.0 * Real::EPSILON
                && parameter + shift <= *end + 32.0 * Real::EPSILON
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
    let closed = (end - start - PERIOD).abs() <= 32.0 * Real::EPSILON;
    let mut breaks = vec![start];
    for index in -4..=4 {
        let node = index as Real * TURN;
        if node > start + 32.0 * Real::EPSILON && node < end - 32.0 * Real::EPSILON {
            breaks.push(node);
        }
    }
    breaks.push(end);
    breaks.sort_by(Real::total_cmp);
    let mut spans = Vec::with_capacity(breaks.len() - 1);
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
        spans.push(count);
    }
    let mut controls = Vec::with_capacity(3 * segments + 1);
    let mut knots = Vec::with_capacity(3 * segments + 5);
    knots.extend([start; 4]);
    let (first_point, mut previous_tangent) = basis.sample(start)?;
    controls.push(first_point);
    let mut previous_point = first_point;
    let mut previous_parameter = start;
    for (span, &count) in breaks.windows(2).zip(&spans) {
        for segment in 1..=count {
            let parameter = if segment == count {
                span[1]
            } else {
                span[0] + (span[1] - span[0]) * (segment as Real / count as Real)
            };
            let (mut point, tangent) = basis.sample(parameter)?;
            if closed && parameter == end {
                point = first_point;
            }
            let handle = (parameter - previous_parameter) / 3.0;
            controls.push(previous_point.translated(previous_tangent.scaled(handle)?)?);
            controls.push(point.translated(tangent.scaled(-handle)?)?);
            controls.push(point);
            knots.extend([parameter; 3]);
            previous_point = point;
            previous_tangent = tangent;
            previous_parameter = parameter;
        }
    }
    knots.push(end);
    NurbsCurve::try_new(3, controls, knots)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Frame3, NurbsSurface, surface_surface_intersection_events};

    fn point(x: Real, y: Real, z: Real) -> Point3 {
        Point3::try_new(x, y, z).unwrap()
    }

    fn cylinders(
        angle: Real,
        miss: Real,
        first_start: Real,
        first_height: Real,
        second_start: Real,
        second_height: Real,
    ) -> ((NurbsSurface, Frame3), (NurbsSurface, Frame3)) {
        let (sine, cosine) = angle.sin_cos();
        let first_frame = Frame3::try_from_normal(
            point(0.0, 0.0, first_start),
            Vector3::try_new(0.0, 0.0, 1.0).unwrap(),
            Tolerance::DEFAULT,
        )
        .unwrap();
        let second_frame = Frame3::try_from_normal(
            point(second_start * sine, miss, second_start * cosine),
            Vector3::try_new(sine, 0.0, cosine).unwrap(),
            Tolerance::DEFAULT,
        )
        .unwrap();
        (
            (
                NurbsSurface::try_cylinder(first_frame, 1.0, 0.0, first_height).unwrap(),
                first_frame,
            ),
            (
                NurbsSurface::try_cylinder(second_frame, 2.0, 0.0, second_height).unwrap(),
                second_frame,
            ),
        )
    }

    fn assert_on_walls(location: Point3, first: Frame3, second: Frame3) {
        for (frame, radius) in [(first, 1.0), (second, 2.0)] {
            let local = frame.coordinates_of(location).unwrap();
            assert!((local[0].hypot(local[1]) - radius).abs() < 2e-9);
        }
    }

    #[test]
    fn pinched_skew_cylinders_make_one_nodal_closed_curve() {
        for angle in [
            std::f64::consts::FRAC_PI_2,
            std::f64::consts::FRAC_PI_3,
            2.0 * std::f64::consts::FRAC_PI_3,
        ] {
            for miss in [-1.0, 1.0] {
                let ((first, first_frame), (second, second_frame)) =
                    cylinders(angle, miss, -4.0, 8.0, -4.0, 8.0);
                for (left, right) in [(&first, &second), (&second, &first)] {
                    let events =
                        surface_surface_intersection_events(left, right, Tolerance::DEFAULT)
                            .unwrap();
                    assert_eq!(events.len(), 1);
                    let SurfaceSurfaceIntersectionEvent::Curve(curve) = &events[0] else {
                        panic!("internal skew tangency must make a nodal curve")
                    };
                    assert_eq!(curve.degree(), 3);
                    assert!(curve.is_closed().unwrap());
                    let domain = curve.domain();
                    assert!((*domain.end() - *domain.start() - PERIOD).abs() < 1e-12);
                    let node = curve.evaluate(*domain.start()).unwrap();
                    assert!(node.distance_to(curve.evaluate(TURN).unwrap()).unwrap() < 1e-9);
                    assert!(
                        node.distance_to(curve.evaluate(*domain.end()).unwrap())
                            .unwrap()
                            < 1e-9
                    );
                    let first_tangent = curve.derivative_at(*domain.start()).unwrap();
                    let crossing_tangent = curve.derivative_at(TURN).unwrap();
                    assert!(
                        first_tangent
                            .cross(crossing_tangent)
                            .unwrap()
                            .length()
                            .unwrap()
                            > 0.1
                    );
                    for index in 0..=128 {
                        let parameter = *domain.start()
                            + (*domain.end() - *domain.start()) * (index as Real / 128.0);
                        assert_on_walls(
                            curve.evaluate(parameter).unwrap(),
                            first_frame,
                            second_frame,
                        );
                    }
                }
            }
        }
    }

    #[test]
    fn pinched_skew_cylinders_clip_to_two_open_arcs() {
        let ((first, first_frame), (second, second_frame)) =
            cylinders(std::f64::consts::FRAC_PI_2, 1.0, 0.2, 1.3, -4.0, 8.0);
        let events =
            surface_surface_intersection_events(&first, &second, Tolerance::DEFAULT).unwrap();
        assert_eq!(events.len(), 2);
        for event in events {
            let SurfaceSurfaceIntersectionEvent::Curve(curve) = event else {
                panic!("the finite height band must leave two open arcs")
            };
            assert!(!curve.is_closed().unwrap());
            let domain = curve.domain();
            for index in 0..=32 {
                let parameter = if index == 32 {
                    *domain.end()
                } else {
                    *domain.start() + (*domain.end() - *domain.start()) * (index as Real / 32.0)
                };
                let location = curve.evaluate(parameter).unwrap();
                assert_on_walls(location, first_frame, second_frame);
                assert!((0.2 - 1e-9..=1.5 + 1e-9).contains(&location.z()));
            }
            for parameter in [*domain.start(), *domain.end()] {
                let axial = curve.evaluate(parameter).unwrap().z();
                assert!((axial - 0.2).abs() < 1e-9 || (axial - 1.5).abs() < 1e-9);
            }
        }
    }

    #[test]
    fn pinched_skew_cylinders_clip_exactly_at_the_node() {
        let ((first, first_frame), (second, second_frame)) =
            cylinders(std::f64::consts::FRAC_PI_2, 1.0, 0.0, 2.0, -4.0, 8.0);
        let events =
            surface_surface_intersection_events(&first, &second, Tolerance::DEFAULT).unwrap();
        assert_eq!(events.len(), 1);
        let SurfaceSurfaceIntersectionEvent::Curve(curve) = &events[0] else {
            panic!("a height cut at the node must retain one lobe")
        };
        let domain = curve.domain();
        assert!(
            curve
                .evaluate(*domain.start())
                .unwrap()
                .distance_to(curve.evaluate(*domain.end()).unwrap())
                .unwrap()
                < 1e-9
        );
        for index in 0..=32 {
            let parameter = if index == 32 {
                *domain.end()
            } else {
                *domain.start() + (*domain.end() - *domain.start()) * (index as Real / 32.0)
            };
            let location = curve.evaluate(parameter).unwrap();
            assert_on_walls(location, first_frame, second_frame);
            assert!((-1e-9..=2.0 + 1e-9).contains(&location.z()));
        }
    }

    #[test]
    fn pinched_skew_cylinders_keep_isolated_rim_tangency() {
        let ((first, first_frame), (second, second_frame)) =
            cylinders(std::f64::consts::FRAC_PI_2, 1.0, -3.0, 1.0, -4.0, 8.0);
        let events =
            surface_surface_intersection_events(&first, &second, Tolerance::DEFAULT).unwrap();
        assert_eq!(events.len(), 1);
        let SurfaceSurfaceIntersectionEvent::Point(location) = events[0] else {
            panic!("the small cylinder rim must touch in one point")
        };
        assert_on_walls(location, first_frame, second_frame);
        assert!((location.z() + 2.0).abs() < 1e-9);
    }

    #[test]
    fn pinched_skew_cylinders_work_far_from_origin_in_rotated_frames() {
        let closest = point(1.0e8, -1.0e8, 1.0e8);
        let base = Frame3::try_from_normal(
            closest,
            Vector3::try_new(1.0, 2.0, 3.0).unwrap(),
            Tolerance::DEFAULT,
        )
        .unwrap();
        let (sine, cosine) = std::f64::consts::FRAC_PI_3.sin_cos();
        let first_axis = base.z_axis().as_vector();
        let transverse = base.x_axis().as_vector();
        let second_axis = Vector3::try_new(
            cosine * first_axis.x() + sine * transverse.x(),
            cosine * first_axis.y() + sine * transverse.y(),
            cosine * first_axis.z() + sine * transverse.z(),
        )
        .unwrap();
        let normal = first_axis
            .cross(second_axis)
            .unwrap()
            .normalized_nonzero()
            .unwrap()
            .as_vector();
        let first_frame = Frame3::try_from_normal(
            closest
                .translated(first_axis.scaled(-4.0).unwrap())
                .unwrap(),
            first_axis,
            Tolerance::DEFAULT,
        )
        .unwrap();
        let second_frame = Frame3::try_from_normal(
            closest
                .translated(normal.scaled(1.0).unwrap())
                .unwrap()
                .translated(second_axis.scaled(-4.0).unwrap())
                .unwrap(),
            second_axis,
            Tolerance::DEFAULT,
        )
        .unwrap();
        let first = NurbsSurface::try_cylinder(first_frame, 1.0, 0.0, 8.0).unwrap();
        let second = NurbsSurface::try_cylinder(second_frame, 2.0, 0.0, 8.0).unwrap();
        for (left, right) in [(&first, &second), (&second, &first)] {
            let events =
                surface_surface_intersection_events(left, right, Tolerance::DEFAULT).unwrap();
            assert_eq!(events.len(), 1);
            let SurfaceSurfaceIntersectionEvent::Curve(curve) = &events[0] else {
                panic!("translated internal skew tangency must make a curve")
            };
            assert!(curve.is_closed().unwrap());
            let domain = curve.domain();
            for index in 0..=64 {
                let parameter =
                    *domain.start() + (*domain.end() - *domain.start()) * (index as Real / 64.0);
                let location = curve.evaluate(parameter).unwrap();
                for (frame, radius) in [(first_frame, 1.0), (second_frame, 2.0)] {
                    let local = frame.coordinates_of(location).unwrap();
                    assert!((local[0].hypot(local[1]) - radius).abs() < 3e-7);
                }
            }
        }
    }
}
