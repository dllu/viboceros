//! Smooth sphere/cone sections when the sphere strictly contains the cone apex.
//!
//! The cone generator meets the sphere once for every angle. Its positive
//! axial root is smooth because the quadratic discriminant stays positive.
//! Cubic Hermite spans use a global fourth-derivative error bound.

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
    let center_offset = center_x.hypot(center_y);
    let height = signed_height.abs();
    let center_axial = center_z * signed_height.signum();
    let apex_distance = center_offset.hypot(center_axial);
    let slope = cone_radius / height;
    let quadratic = slope.mul_add(slope, 1.0);
    let constant = (apex_distance - sphere_radius) * (apex_distance + sphere_radius);
    let discriminant_floor = -quadratic * constant;
    let basis = Basis {
        frame: cone_frame,
        slope,
        axial_sign: signed_height.signum(),
        direction: [center_x / center_offset, center_y / center_offset],
        center_axial,
        cosine_coefficient: slope * center_offset,
        quadratic,
        constant,
        discriminant_floor,
    };
    let fit_tolerance = tolerance
        .absolute()
        .max(tolerance.relative() * cone_radius.max(height).max(sphere_radius))
        .max(coordinate_roundoff);
    if !fit_tolerance.is_finite() || !discriminant_floor.is_finite() {
        return Err(GeometryError::UnsupportedSurfaceSurfaceIntersection {
            context: "noncoaxial sphere/cone fit is ill-conditioned",
        });
    }
    let minimum = basis.axial(std::f64::consts::PI);
    let maximum = basis.axial(0.0);
    if minimum > height + fit_tolerance {
        return Ok(Vec::new());
    }
    let derivative_bound = fourth_derivative_bound(basis);
    if !derivative_bound.is_finite() {
        return Err(GeometryError::UnsupportedSurfaceSurfaceIntersection {
            context: "noncoaxial sphere/cone derivative bound is not finite",
        });
    }
    let mut cuts = vec![0.0, std::f64::consts::PI, TURN];
    if minimum < height && height < maximum {
        let required_linear = (quadratic * height * height + constant) / (2.0 * height);
        let cosine = (required_linear - center_axial) / basis.cosine_coefficient;
        let angle = cosine.clamp(-1.0, 1.0).acos();
        cuts.push(angle);
        cuts.push(TURN - angle);
    }
    cuts.sort_by(Real::total_cmp);
    cuts.dedup_by(|left, right| (*left - *right).abs() <= 32.0 * Real::EPSILON);
    let intervals = active_intervals(basis, &cuts, height);
    let mut events = Vec::new();
    for angle in cuts {
        if (basis.axial(angle) - height).abs() <= fit_tolerance
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
    fn linear(self, angle: Real) -> Real {
        self.cosine_coefficient
            .mul_add(angle.cos(), self.center_axial)
    }

    fn axial(self, angle: Real) -> Real {
        let linear = self.linear(angle);
        let root = linear.hypot(self.discriminant_floor.sqrt());
        if linear < 0.0 {
            -self.constant / (root - linear)
        } else {
            (linear + root) / self.quadratic
        }
    }

    fn sample(self, angle: Real) -> Result<(Point3, Vector3), GeometryError> {
        let (sine, cosine) = angle.sin_cos();
        let linear = self.cosine_coefficient.mul_add(cosine, self.center_axial);
        let root = linear.hypot(self.discriminant_floor.sqrt());
        let axial = if linear < 0.0 {
            -self.constant / (root - linear)
        } else {
            (linear + root) / self.quadratic
        };
        let axial_derivative = -self.cosine_coefficient * sine * axial / root;
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
        let x = self.frame.x_axis().as_vector().to_array();
        let y = self.frame.y_axis().as_vector().to_array();
        let z = self.frame.z_axis().as_vector().to_array();
        let tangent = Vector3::try_from(std::array::from_fn(|i| {
            derivative[0].mul_add(x[i], derivative[1].mul_add(y[i], derivative[2] * z[i]))
        }))?;
        Ok((self.frame.point_at(local)?, tangent))
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
    let g0 = linear_max.hypot(root);
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

fn active_intervals(basis: Basis, cuts: &[Real], height: Real) -> Vec<(Real, Real)> {
    let mut intervals: Vec<(Real, Real)> = Vec::new();
    for pair in cuts.windows(2) {
        if basis.axial(0.5 * (pair[0] + pair[1])) <= height {
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
    let (first_point, mut previous_tangent) = basis.sample(start)?;
    controls.push(first_point);
    let mut previous_point = first_point;
    let mut previous_angle = start;
    for segment in 1..=segments {
        let angle = if segment == segments {
            end
        } else {
            start + (end - start) * (segment as Real / segments as Real)
        };
        let (mut point, tangent) = basis.sample(angle)?;
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

    fn cone() -> NurbsSurface {
        NurbsSurface::try_cone(frame(), 3.0, 4.0).unwrap()
    }

    fn sphere(center: Point3, radius: Real) -> NurbsSurface {
        NurbsSurface::try_sphere(frame().with_origin(center), radius).unwrap()
    }

    fn assert_on_walls(location: Point3, center: Point3, radius: Real) {
        assert!((location.z() - 4.0 * location.x().hypot(location.y()) / 3.0).abs() < 5e-9);
        assert!((location.distance_to(center).unwrap() - radius).abs() < 5e-9);
        assert!((-5e-9..=4.0 + 5e-9).contains(&location.z()));
    }

    #[test]
    fn noncoaxial_sphere_cone_containing_apex_makes_one_closed_curve() {
        let center = point(0.5, 0.0, 0.5);
        let sphere = sphere(center, 1.5);
        let cone = cone();
        for (first, second) in [(&sphere, &cone), (&cone, &sphere)] {
            let events =
                surface_surface_intersection_events(first, second, Tolerance::DEFAULT).unwrap();
            let [SurfaceSurfaceIntersectionEvent::Curve(curve)] = events.as_slice() else {
                panic!("sphere containing cone apex must cut one loop, got {events:#?}")
            };
            assert_eq!(curve.degree(), 3);
            assert!(curve.is_closed().unwrap());
            for index in 0..=128 {
                let angle = TURN * (index as Real / 128.0);
                assert_on_walls(curve.evaluate(angle).unwrap(), center, 1.5);
            }
        }
    }

    #[test]
    fn noncoaxial_sphere_cone_clips_to_base_arc_and_tangent_point() {
        let center = point(0.5, 0.0, 3.0);
        let section = sphere(center, 3.5);
        let events =
            surface_surface_intersection_events(&section, &cone(), Tolerance::DEFAULT).unwrap();
        let [SurfaceSurfaceIntersectionEvent::Curve(arc)] = events.as_slice() else {
            panic!("cone base must clip sphere section to an arc, got {events:#?}")
        };
        assert!(!arc.is_closed().unwrap());
        let domain = arc.domain();
        for angle in [*domain.start(), *domain.end()] {
            assert!((arc.evaluate(angle).unwrap().z() - 4.0).abs() < 5e-9);
        }
        for index in 0..=64 {
            let angle =
                *domain.start() + (*domain.end() - *domain.start()) * (index as Real / 64.0);
            assert_on_walls(arc.evaluate(angle).unwrap(), center, 3.5);
        }

        let tangent_radius = 13.25_f64.sqrt();
        let events = surface_surface_intersection_events(
            &sphere(center, tangent_radius),
            &cone(),
            Tolerance::DEFAULT,
        )
        .unwrap();
        let [SurfaceSurfaceIntersectionEvent::Point(contact)] = events.as_slice() else {
            panic!("cone base must touch at one point, got {events:#?}")
        };
        assert!(contact.distance_to(point(-3.0, 0.0, 4.0)).unwrap() < 5e-9);

        assert!(
            surface_surface_intersection_events(
                &sphere(point(0.5, 0.0, 5.0), 5.1),
                &cone(),
                Tolerance::DEFAULT,
            )
            .unwrap()
            .is_empty()
        );
    }

    #[test]
    fn noncoaxial_sphere_cone_handles_sphere_center_below_apex() {
        let center = point(0.3, 0.4, -0.5);
        let sphere = sphere(center, 1.5);
        for (first, second) in [(&sphere, &cone()), (&cone(), &sphere)] {
            let events =
                surface_surface_intersection_events(first, second, Tolerance::DEFAULT).unwrap();
            let [SurfaceSurfaceIntersectionEvent::Curve(curve)] = events.as_slice() else {
                panic!("sphere below apex must still produce one closed loop")
            };
            assert!(curve.is_closed().unwrap());
            for index in 0..=64 {
                assert_on_walls(
                    curve.evaluate(TURN * (index as Real / 64.0)).unwrap(),
                    center,
                    1.5,
                );
            }
        }
    }

    #[test]
    fn noncoaxial_sphere_cone_handles_negative_and_distant_rotated_frames() {
        let negative = NurbsSurface::try_cone(frame(), 3.0, -4.0).unwrap();
        let sphere = sphere(point(0.5, 0.0, -0.5), 1.5);
        let events =
            surface_surface_intersection_events(&sphere, &negative, Tolerance::DEFAULT).unwrap();
        let [SurfaceSurfaceIntersectionEvent::Curve(curve)] = events.as_slice() else {
            panic!("negative cone height must produce a closed loop")
        };
        assert!(curve.is_closed().unwrap());
        for index in 0..=64 {
            let location = curve.evaluate(TURN * (index as Real / 64.0)).unwrap();
            assert!((location.z() + 4.0 * location.x().hypot(location.y()) / 3.0).abs() < 5e-9);
            assert!((location.distance_to(point(0.5, 0.0, -0.5)).unwrap() - 1.5).abs() < 5e-9);
        }

        let rotated = Frame3::try_from_normal(
            point(1.0e8, -1.0e8, 1.0e8),
            Vector3::try_new(1.0, 2.0, 3.0).unwrap(),
            Tolerance::DEFAULT,
        )
        .unwrap();
        let center = rotated.point_at([0.5, 0.0, 0.5]).unwrap();
        let cone = NurbsSurface::try_cone(rotated, 3.0, 4.0).unwrap();
        let sphere = NurbsSurface::try_sphere(rotated.with_origin(center), 1.5).unwrap();
        for (first, second) in [(&sphere, &cone), (&cone, &sphere)] {
            let events =
                surface_surface_intersection_events(first, second, Tolerance::DEFAULT).unwrap();
            let [SurfaceSurfaceIntersectionEvent::Curve(curve)] = events.as_slice() else {
                panic!("distant rotated sphere/cone must produce a loop")
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
