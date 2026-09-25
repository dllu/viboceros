//! Two separated sphere/cone loops when both axial roots are positive.
//!
//! The generator discriminant stays positive at every cone angle. Each root
//! is smooth, so a fourth-derivative bound controls cubic Hermite fitting.

use super::SurfaceSurfaceIntersectionEvent;
use crate::{Frame3, GeometryError, NurbsCurve, Point3, Real, Tolerance, Vector3};

const MAX_SEGMENTS: usize = 4096;
const TURN: Real = std::f64::consts::TAU;

#[derive(Clone, Copy)]
struct Basis {
    frame: Frame3,
    slope: Real,
    axial_sign: Real,
    direction: [Real; 2],
    center_axial: Real,
    cosine_coefficient: Real,
    quadratic: Real,
    constant: Real,
    minimum_linear: Real,
    discriminant_floor: Real,
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
    let center_axial = center_z * signed_height.signum();
    let slope = cone_radius / height;
    let quadratic = slope.mul_add(slope, 1.0);
    let apex_distance = offset.hypot(center_axial);
    let constant = (apex_distance - sphere_radius) * (apex_distance + sphere_radius);
    let cosine_coefficient = slope * offset;
    let minimum_linear = center_axial - cosine_coefficient;
    let discriminant_floor = minimum_linear * minimum_linear - quadratic * constant;
    let basis = Basis {
        frame: cone_frame,
        slope,
        axial_sign: signed_height.signum(),
        direction: [center_x / offset, center_y / offset],
        center_axial,
        cosine_coefficient,
        quadratic,
        constant,
        minimum_linear,
        discriminant_floor,
    };
    let fit_tolerance = tolerance
        .absolute()
        .max(tolerance.relative() * cone_radius.max(height).max(sphere_radius))
        .max(coordinate_roundoff);
    if !fit_tolerance.is_finite() || !discriminant_floor.is_finite() || discriminant_floor <= 0.0 {
        return Err(GeometryError::UnsupportedSurfaceSurfaceIntersection {
            context: "separated sphere/cone fit is ill-conditioned",
        });
    }
    let derivative_bound = fourth_derivative_bound(basis);
    if !derivative_bound.is_finite() {
        return Err(GeometryError::UnsupportedSurfaceSurfaceIntersection {
            context: "separated sphere/cone derivative bound is not finite",
        });
    }
    let mut events = Vec::new();
    for sign in [-1.0, 1.0] {
        let axial_zero = basis.axial(0.0, sign);
        let axial_opposite = basis.axial(std::f64::consts::PI, sign);
        let minimum = axial_zero.min(axial_opposite);
        let maximum = axial_zero.max(axial_opposite);
        if minimum > height + fit_tolerance {
            continue;
        }
        if (height - minimum).abs() <= fit_tolerance {
            let angle = if axial_zero < axial_opposite {
                0.0
            } else {
                std::f64::consts::PI
            };
            events.push(SurfaceSurfaceIntersectionEvent::Point(
                basis.sample(angle, sign)?.0,
            ));
            continue;
        }
        let mut cuts = vec![0.0, std::f64::consts::PI, TURN];
        if minimum < height && height < maximum {
            let required_linear = (quadratic * height * height + constant) / (2.0 * height);
            let cosine = (required_linear - center_axial) / cosine_coefficient;
            let angle = cosine.clamp(-1.0, 1.0).acos();
            cuts.push(angle);
            cuts.push(TURN - angle);
        }
        cuts.sort_by(Real::total_cmp);
        cuts.dedup_by(|left, right| (*left - *right).abs() <= 32.0 * Real::EPSILON);
        for (start, end) in active_intervals(basis, &cuts, sign, height) {
            events.push(SurfaceSurfaceIntersectionEvent::Curve(fit_curve(
                basis,
                start,
                end,
                sign,
                fit_tolerance,
                derivative_bound,
            )?));
        }
    }
    Ok(events)
}

impl Basis {
    fn linear(self, angle: Real) -> Real {
        self.cosine_coefficient
            .mul_add(angle.cos(), self.center_axial)
    }

    fn root(self, linear: Real) -> Real {
        (self.discriminant_floor + (linear - self.minimum_linear) * (linear + self.minimum_linear))
            .sqrt()
    }

    fn axial(self, angle: Real, sign: Real) -> Real {
        let linear = self.linear(angle);
        let root = self.root(linear);
        if sign < 0.0 {
            self.constant / (linear + root)
        } else {
            (linear + root) / self.quadratic
        }
    }

    fn sample(self, angle: Real, sign: Real) -> Result<(Point3, Vector3), GeometryError> {
        let (sine, cosine) = angle.sin_cos();
        let linear = self.cosine_coefficient.mul_add(cosine, self.center_axial);
        let root = self.root(linear);
        let axial = if sign < 0.0 {
            self.constant / (linear + root)
        } else {
            (linear + root) / self.quadratic
        };
        let axial_derivative = -self.cosine_coefficient * sine * axial / (sign * root);
        let [ux, uy] = self.direction;
        let radial = self.slope * axial;
        let radial_derivative = self.slope * axial_derivative;
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
        Ok((
            self.frame.point_at(local)?,
            self.frame.vector_at(derivative)?,
        ))
    }
}

fn fourth_derivative_bound(basis: Basis) -> Real {
    let b0 = basis.center_axial.abs();
    let b1 = basis.cosine_coefficient.abs();
    let linear_max = b0 + b1;
    let first_harmonic = 2.0 * b0 * b1;
    let second_harmonic = b1 * b1 / 2.0;
    let q1 = first_harmonic + 2.0 * second_harmonic;
    let q2 = first_harmonic + 4.0 * second_harmonic;
    let q3 = first_harmonic + 8.0 * second_harmonic;
    let q4 = first_harmonic + 16.0 * second_harmonic;
    let lower = basis.discriminant_floor;
    let root = lower.sqrt();
    let lower3 = lower * root;
    let lower5 = lower * lower3;
    let lower7 = lower * lower5;
    let g0 = basis.root(linear_max);
    let g1 = q1 / (2.0 * root);
    let g2 = q2 / (2.0 * root) + q1 * q1 / (4.0 * lower3);
    let g3 = q3 / (2.0 * root) + 3.0 * q1 * q2 / (4.0 * lower3) + 3.0 * q1.powi(3) / (8.0 * lower5);
    let g4 = q4 / (2.0 * root)
        + (4.0 * q1 * q3 + 3.0 * q2 * q2) / (4.0 * lower3)
        + 9.0 * q1 * q1 * q2 / (4.0 * lower5)
        + 15.0 * q1.powi(4) / (16.0 * lower7);
    let t0 = (linear_max + g0) / basis.quadratic;
    let t1 = (b1 + g1) / basis.quadratic;
    let t2 = (b1 + g2) / basis.quadratic;
    let t3 = (b1 + g3) / basis.quadratic;
    let t4 = (b1 + g4) / basis.quadratic;
    (basis.slope * (t0 + 4.0 * t1 + 6.0 * t2 + 4.0 * t3 + t4)).hypot(t4)
}

fn active_intervals(basis: Basis, cuts: &[Real], sign: Real, height: Real) -> Vec<(Real, Real)> {
    let mut intervals: Vec<(Real, Real)> = Vec::new();
    for pair in cuts.windows(2) {
        if basis.axial(0.5 * (pair[0] + pair[1]), sign) <= height {
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

fn fit_curve(
    basis: Basis,
    start: Real,
    end: Real,
    sign: Real,
    fit_tolerance: Real,
    derivative_bound: Real,
) -> Result<NurbsCurve, GeometryError> {
    let max_span = (384.0 * fit_tolerance / derivative_bound).powf(0.25);
    if !max_span.is_finite() || max_span <= 0.0 {
        return Err(GeometryError::TooManyCurveFitControlPoints {
            maximum: 3 * MAX_SEGMENTS + 1,
        });
    }
    let required = ((end - start) / max_span).ceil();
    if !required.is_finite() || required > MAX_SEGMENTS as Real {
        return Err(GeometryError::TooManyCurveFitControlPoints {
            maximum: 3 * MAX_SEGMENTS + 1,
        });
    }
    let closed = (end - start - TURN).abs() <= 32.0 * Real::EPSILON;
    let segments = (required as usize).max(if closed { 4 } else { 1 });
    let mut controls = Vec::with_capacity(3 * segments + 1);
    let mut knots = Vec::with_capacity(3 * segments + 5);
    knots.extend([start; 4]);
    let (first_point, mut previous_tangent) = basis.sample(start, sign)?;
    controls.push(first_point);
    let mut previous_point = first_point;
    let mut previous_angle = start;
    for segment in 1..=segments {
        let angle = if segment == segments {
            end
        } else {
            start + (end - start) * (segment as Real / segments as Real)
        };
        let (mut point, tangent) = basis.sample(angle, sign)?;
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

    fn sphere() -> NurbsSurface {
        NurbsSurface::try_sphere(frame().with_origin(point(0.2, 0.0, 2.0)), 1.5).unwrap()
    }

    fn assert_on_walls(location: Point3, height: Real) {
        assert!((location.z() - 4.0 * location.x().hypot(location.y()) / 3.0).abs() < 5e-9);
        assert!((location.distance_to(point(0.2, 0.0, 2.0)).unwrap() - 1.5).abs() < 5e-9);
        assert!((-5e-9..=height + 5e-9).contains(&location.z()));
    }

    #[test]
    fn separated_sphere_cone_makes_two_closed_loops_in_both_orders() {
        let sphere = sphere();
        let cone = cone(4.0);
        for (left, right) in [(&sphere, &cone), (&cone, &sphere)] {
            let events =
                surface_surface_intersection_events(left, right, Tolerance::DEFAULT).unwrap();
            assert_eq!(events.len(), 2);
            for event in events {
                let SurfaceSurfaceIntersectionEvent::Curve(curve) = event else {
                    panic!("separated sphere/cone branches must be curves")
                };
                assert_eq!(curve.degree(), 3);
                assert!(curve.is_closed().unwrap());
                for index in 0..=128 {
                    assert_on_walls(curve.evaluate(TURN * (index as Real / 128.0)).unwrap(), 4.0);
                }
            }
        }
    }

    #[test]
    fn separated_sphere_cone_clips_each_loop_at_the_base() {
        let events =
            surface_surface_intersection_events(&sphere(), &cone(2.0), Tolerance::DEFAULT).unwrap();
        let [
            SurfaceSurfaceIntersectionEvent::Curve(lower),
            SurfaceSurfaceIntersectionEvent::Curve(upper),
        ] = events.as_slice()
        else {
            panic!("base must keep lower loop and upper arc, got {events:#?}")
        };
        assert!(lower.is_closed().unwrap());
        assert!(!upper.is_closed().unwrap());
        for curve in [lower, upper] {
            let domain = curve.domain();
            for index in 0..=64 {
                let angle =
                    *domain.start() + (*domain.end() - *domain.start()) * (index as Real / 64.0);
                assert_on_walls(curve.evaluate(angle).unwrap(), 2.0);
            }
        }
        for angle in [*upper.domain().start(), *upper.domain().end()] {
            assert!((upper.evaluate(angle).unwrap().z() - 2.0).abs() < 5e-9);
        }

        let events =
            surface_surface_intersection_events(&sphere(), &cone(0.6), Tolerance::DEFAULT).unwrap();
        let [SurfaceSurfaceIntersectionEvent::Curve(lower_arc)] = events.as_slice() else {
            panic!("small cone must leave one lower arc, got {events:#?}")
        };
        assert!(!lower_arc.is_closed().unwrap());
        let domain = lower_arc.domain();
        for index in 0..=64 {
            let angle =
                *domain.start() + (*domain.end() - *domain.start()) * (index as Real / 64.0);
            assert_on_walls(lower_arc.evaluate(angle).unwrap(), 0.6);
        }
        assert!(
            surface_surface_intersection_events(&sphere(), &cone(0.4), Tolerance::DEFAULT)
                .unwrap()
                .is_empty()
        );
    }

    #[test]
    fn separated_sphere_cone_keeps_isolated_rim_contacts() {
        let slope: Real = 0.75;
        let quadratic = slope.mul_add(slope, 1.0);
        let constant: Real = 0.2_f64.hypot(2.0).powi(2) - 1.5_f64.powi(2);
        let high_linear = 2.0 + slope * 0.2;
        let low_linear = 2.0 - slope * 0.2;
        let lower_minimum =
            constant / (high_linear + (high_linear * high_linear - quadratic * constant).sqrt());
        let upper_minimum =
            (low_linear + (low_linear * low_linear - quadratic * constant).sqrt()) / quadratic;
        let events = surface_surface_intersection_events(
            &sphere(),
            &cone(lower_minimum),
            Tolerance::DEFAULT,
        )
        .unwrap();
        let [SurfaceSurfaceIntersectionEvent::Point(contact)] = events.as_slice() else {
            panic!("lower loop must touch the base in one point, got {events:#?}")
        };
        assert_on_walls(*contact, lower_minimum);

        let events = surface_surface_intersection_events(
            &sphere(),
            &cone(upper_minimum),
            Tolerance::DEFAULT,
        )
        .unwrap();
        assert_eq!(events.len(), 2);
        assert!(matches!(
            events[0],
            SurfaceSurfaceIntersectionEvent::Curve(_)
        ));
        let SurfaceSurfaceIntersectionEvent::Point(contact) = events[1] else {
            panic!("upper loop must touch the base in one point")
        };
        assert_on_walls(contact, upper_minimum);
    }

    #[test]
    fn separated_sphere_cone_handles_negative_and_distant_rotated_frames() {
        let negative = NurbsSurface::try_cone(frame(), 3.0, -4.0).unwrap();
        let negative_sphere =
            NurbsSurface::try_sphere(frame().with_origin(point(0.2, 0.0, -2.0)), 1.5).unwrap();
        let events =
            surface_surface_intersection_events(&negative_sphere, &negative, Tolerance::DEFAULT)
                .unwrap();
        assert_eq!(events.len(), 2);
        for event in events {
            let SurfaceSurfaceIntersectionEvent::Curve(curve) = event else {
                panic!("negative cone height must keep both loops")
            };
            for index in 0..=64 {
                let location = curve.evaluate(TURN * (index as Real / 64.0)).unwrap();
                assert!((location.z() + 4.0 * location.x().hypot(location.y()) / 3.0).abs() < 5e-9);
                assert!((location.distance_to(point(0.2, 0.0, -2.0)).unwrap() - 1.5).abs() < 5e-9);
            }
        }

        let rotated = Frame3::try_from_normal(
            point(1.0e8, -1.0e8, 1.0e8),
            Vector3::try_new(1.0, 2.0, 3.0).unwrap(),
            Tolerance::DEFAULT,
        )
        .unwrap();
        let center = rotated.point_at([0.12, 0.16, 2.0]).unwrap();
        let cone = NurbsSurface::try_cone(rotated, 3.0, 4.0).unwrap();
        let sphere = NurbsSurface::try_sphere(rotated.with_origin(center), 1.5).unwrap();
        for (left, right) in [(&sphere, &cone), (&cone, &sphere)] {
            let events =
                surface_surface_intersection_events(left, right, Tolerance::DEFAULT).unwrap();
            assert_eq!(events.len(), 2);
            for event in events {
                let SurfaceSurfaceIntersectionEvent::Curve(curve) = event else {
                    panic!("distant rotated sphere/cone must make curves")
                };
                for index in 0..=64 {
                    let location = curve.evaluate(TURN * (index as Real / 64.0)).unwrap();
                    let local = rotated.coordinates_of(location).unwrap();
                    assert!((local[2] - 4.0 * local[0].hypot(local[1]) / 3.0).abs() < 4e-7);
                    assert!((location.distance_to(center).unwrap() - 1.5).abs() < 4e-7);
                }
            }
        }
    }
}
