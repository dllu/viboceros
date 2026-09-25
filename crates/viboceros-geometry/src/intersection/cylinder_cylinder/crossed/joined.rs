//! One smooth loop where the square-root branches of skew cylinder walls join.
//!
//! The small-cylinder angle sweeps from one square-root zero to the other and
//! back as a cosine of the curve parameter. Factoring the radical into sinc
//! terms removes its apparent endpoint singularities.

use super::{SurfaceSurfaceIntersectionEvent, skew_clip};
use crate::{GeometryError, NurbsCurve, Point3, Real, Tolerance, Vector3};

const MAX_SEGMENTS: usize = 8192;
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
    alpha: Real,
    length: Real,
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
            context: "nearly parallel skew cylinder axes",
        });
    }
    let normal = cross.scaled(1.0 / sine)?;
    let separation = miss.abs();
    let miss_sign = miss.signum();
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
    if separation >= big + small {
        if (0.0..=small_height).contains(&small_center) && (0.0..=big_height).contains(&big_center)
        {
            return Ok(vec![SurfaceSurfaceIntersectionEvent::Point(
                small_closest.translated(normal.scaled(miss_sign * small)?)?,
            )]);
        }
        return Ok(Vec::new());
    }
    if separation <= big - small + coordinate_roundoff.max(64.0 * Real::EPSILON * big) {
        return Err(GeometryError::UnsupportedSurfaceSurfaceIntersection {
            context: "skew cylinder branches meet at an internal tangency",
        });
    }
    let small_vector = small_axis.to_array();
    let big_vector = big_axis.to_array();
    let plane_axis = Vector3::try_new(
        (big_vector[0] - cosine * small_vector[0]) / sine,
        (big_vector[1] - cosine * small_vector[1]) / sine,
        (big_vector[2] - cosine * small_vector[2]) / sine,
    )?;
    let alpha = ((separation - big) / small).asin();
    let length = std::f64::consts::PI - 2.0 * alpha;
    let basis = Basis {
        closest: small_closest,
        small_axis,
        plane_axis,
        normal,
        cosine,
        sine,
        small,
        big,
        miss,
        miss_sign,
        alpha,
        length,
    };
    let derivative_bound = fourth_derivative_bound(basis);
    if !derivative_bound.is_finite() {
        return Err(GeometryError::UnsupportedSurfaceSurfaceIntersection {
            context: "joined skew cylinder fit is ill-conditioned",
        });
    }
    let AngularClip { intervals, cuts } = active_intervals(basis, clip, fit_tolerance)?;
    let mut events = Vec::new();
    for angle in cuts {
        let (small_axial, big_axial) = basis.axials(angle);
        let small_axial = small_center + small_axial;
        let big_axial = big_center + big_axial;
        let inside = (-fit_tolerance..=small_height + fit_tolerance).contains(&small_axial)
            && (-fit_tolerance..=big_height + fit_tolerance).contains(&big_axial);
        let on_rim = small_axial.abs() <= fit_tolerance
            || (small_axial - small_height).abs() <= fit_tolerance
            || big_axial.abs() <= fit_tolerance
            || (big_axial - big_height).abs() <= fit_tolerance;
        if inside && on_rim && !angle_in_intervals(angle, &intervals) {
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
    fn phi(self, parameter: Real) -> Real {
        self.alpha + 0.5 * self.length * (1.0 - parameter.cos())
    }

    fn root_factor(self, parameter: Real, phi: Real) -> Real {
        let half = 0.5 * parameter;
        let left = 0.5 * self.length * half.sin().powi(2);
        let right = 0.5 * self.length * half.cos().powi(2);
        let positive = self.big + self.miss.abs() - self.small * phi.sin();
        // R²-(r sin(phi)-H)² = sin²(parameter) * factor².
        (positive * self.small * self.length * self.length / 8.0 * sinc(left) * sinc(right)).sqrt()
    }

    fn coordinates(self, parameter: Real) -> (Real, Real, Real, Real, Real) {
        let phi = self.phi(parameter);
        let y = self.small * phi.cos();
        let z = self.miss_sign * self.small * phi.sin();
        let root_factor = self.root_factor(parameter, phi);
        let w = parameter.sin() * root_factor;
        let x = (self.cosine * y - w) / self.sine;
        (x, y, z, w, root_factor)
    }

    fn axials(self, parameter: Real) -> (Real, Real) {
        let (x, y, _, w, _) = self.coordinates(parameter);
        (x, (y - self.cosine * w) / self.sine)
    }

    fn sample(self, parameter: Real) -> Result<(Point3, Vector3), GeometryError> {
        let phi = self.phi(parameter);
        let derivative_phi = 0.5 * self.length * parameter.sin();
        let (x, y, z, w, root_factor) = self.coordinates(parameter);
        let derivative_y = -self.small * phi.sin() * derivative_phi;
        let derivative_z = self.miss_sign * self.small * phi.cos() * derivative_phi;
        let derivative_w = if parameter.sin().abs() <= 1e-8 {
            parameter.cos() * root_factor
        } else {
            -(self.small * phi.sin() - self.miss.abs()) * self.small * phi.cos() * derivative_phi
                / w
        };
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

fn sinc(value: Real) -> Real {
    if value.abs() < 1e-4 {
        let square = value * value;
        1.0 - square / 6.0 + square * square / 120.0
    } else {
        value.sin() / value
    }
}

fn fourth_derivative_bound(basis: Basis) -> Real {
    // sinc(x) = integral_0^1 cos(tx) dt, so its nth derivative has magnitude
    // at most 1/(n+1). Product and square-root rules bound the fourth
    // derivative of the smooth factor in w = sin(parameter) * factor.
    let half_length = 0.5 * basis.length;
    let quarter_length = 0.25 * basis.length;
    let h = basis.miss.abs();
    let r = basis.small;
    let m = half_length;
    let k = quarter_length;
    let trig_fourth = m + 7.0 * m * m + 6.0 * m.powi(3) + m.powi(4);
    let factor = [
        basis.big + h + r,
        r * m,
        r * (m + m * m),
        r * (m + 3.0 * m * m + m.powi(3)),
        r * trig_fourth,
    ];
    let sinc_bounds = [
        1.0,
        k / 2.0,
        k / 2.0 + k * k / 3.0,
        k / 2.0 + k * k + k.powi(3) / 4.0,
        k / 2.0 + 7.0 * k * k / 3.0 + 1.5 * k.powi(3) + k.powi(4) / 5.0,
    ];
    let scale = r * basis.length * basis.length / 8.0;
    let mut q = product_bounds(product_bounds(factor, sinc_bounds), sinc_bounds);
    for derivative in &mut q {
        *derivative *= scale;
    }
    let lower = scale * (basis.big + h - r) * sinc(half_length).powi(2);
    let square_root = lower.sqrt();
    let lower3 = lower * square_root;
    let lower5 = lower * lower3;
    let lower7 = lower * lower5;
    let g0 = q[0].sqrt();
    let g1 = q[1] / (2.0 * square_root);
    let g2 = q[2] / (2.0 * square_root) + q[1] * q[1] / (4.0 * lower3);
    let g3 =
        q[3] / (2.0 * square_root) + 0.75 * q[1] * q[2] / lower3 + 0.375 * q[1].powi(3) / lower5;
    let g4 = q[4] / (2.0 * square_root)
        + (q[1] * q[3] + 0.75 * q[2] * q[2]) / lower3
        + 2.25 * q[1] * q[1] * q[2] / lower5
        + 0.9375 * q[1].powi(4) / lower7;
    let w4 = g0 + 4.0 * g1 + 6.0 * g2 + 4.0 * g3 + g4;
    let radial4 = r * trig_fourth;
    2.0 * radial4 + (basis.cosine.abs() * radial4 + w4) / basis.sine
}

fn product_bounds(left: [Real; 5], right: [Real; 5]) -> [Real; 5] {
    const CHOOSE: [[Real; 5]; 5] = [
        [1.0, 0.0, 0.0, 0.0, 0.0],
        [1.0, 1.0, 0.0, 0.0, 0.0],
        [1.0, 2.0, 1.0, 0.0, 0.0],
        [1.0, 3.0, 3.0, 1.0, 0.0],
        [1.0, 4.0, 6.0, 4.0, 1.0],
    ];
    std::array::from_fn(|order| {
        (0..=order)
            .map(|index| CHOOSE[order][index] * left[index] * right[order - index])
            .sum()
    })
}

fn active_intervals(
    basis: Basis,
    clip: Clip,
    fit_tolerance: Real,
) -> Result<AngularClip, GeometryError> {
    let turn = std::f64::consts::TAU;
    let mut angles = vec![
        0.0,
        std::f64::consts::FRAC_PI_2,
        std::f64::consts::PI,
        3.0 * std::f64::consts::FRAC_PI_2,
        turn,
    ];
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
                    let phi = basis.miss_sign * angle;
                    let beta = basis.alpha + basis.length;
                    for winding in [-1.0, 0.0, 1.0] {
                        let phi = phi + winding * turn;
                        if phi < basis.alpha - 64.0 * Real::EPSILON
                            || phi > beta + 64.0 * Real::EPSILON
                        {
                            continue;
                        }
                        let phi = phi.clamp(basis.alpha, beta);
                        let cosine = ((basis.alpha + 0.5 * basis.length - phi)
                            / (0.5 * basis.length))
                            .clamp(-1.0, 1.0);
                        let forward = cosine.acos();
                        angles.push(if sign > 0.0 { forward } else { turn - forward });
                    }
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
        && intervals.last().is_some_and(|last| last.1 == turn)
    {
        let first = intervals.remove(0);
        let last = intervals.pop().expect("at least two intervals");
        intervals.insert(0, (last.0 - turn, first.1));
    }
    Ok(AngularClip {
        intervals,
        cuts: angles,
    })
}

fn angle_in_intervals(angle: Real, intervals: &[(Real, Real)]) -> bool {
    let turn = std::f64::consts::TAU;
    [-turn, 0.0, turn].into_iter().any(|shift| {
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
    let required = ((end - start) / max_span).ceil();
    if !required.is_finite() || required > MAX_SEGMENTS as Real {
        return Err(GeometryError::TooManyCurveFitControlPoints {
            maximum: 3 * MAX_SEGMENTS + 1,
        });
    }
    let closed = (end - start - std::f64::consts::TAU).abs() <= 32.0 * Real::EPSILON;
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
    fn joined_skew_cylinders_make_one_closed_curve() {
        for angle in [
            std::f64::consts::FRAC_PI_2,
            std::f64::consts::FRAC_PI_3,
            2.0 * std::f64::consts::FRAC_PI_3,
        ] {
            for miss in [-1.5, 1.5] {
                let ((first, first_frame), (second, second_frame)) =
                    cylinders(angle, miss, -4.0, 8.0, -4.0, 8.0);
                for (left, right) in [(&first, &second), (&second, &first)] {
                    let events =
                        surface_surface_intersection_events(left, right, Tolerance::DEFAULT)
                            .unwrap();
                    assert_eq!(events.len(), 1);
                    let SurfaceSurfaceIntersectionEvent::Curve(curve) = &events[0] else {
                        panic!("joined skew walls must make a closed curve")
                    };
                    assert_eq!(curve.degree(), 3);
                    assert!(curve.is_closed().unwrap());
                    let domain = curve.domain();
                    for index in 0..=64 {
                        let parameter = *domain.start()
                            + (*domain.end() - *domain.start()) * (index as Real / 64.0);
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
    fn joined_skew_cylinders_clip_at_small_cylinder_rim() {
        let ((first, first_frame), (second, second_frame)) =
            cylinders(std::f64::consts::FRAC_PI_2, 1.5, 0.0, 2.0, -4.0, 8.0);
        let events =
            surface_surface_intersection_events(&first, &second, Tolerance::DEFAULT).unwrap();
        assert_eq!(events.len(), 1);
        let SurfaceSurfaceIntersectionEvent::Curve(curve) = &events[0] else {
            panic!("one half of the joined loop must remain")
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
            assert!((-1e-9..=2.0 + 1e-9).contains(&location.z()));
        }
    }

    #[test]
    fn joined_skew_cylinders_clip_oblique_loop_at_both_heights() {
        let ((first, first_frame), (second, second_frame)) =
            cylinders(std::f64::consts::FRAC_PI_3, 1.5, -1.0, 2.0, -1.0, 2.0);
        let events =
            surface_surface_intersection_events(&first, &second, Tolerance::DEFAULT).unwrap();
        assert!(!events.is_empty());
        for event in events {
            let SurfaceSurfaceIntersectionEvent::Curve(curve) = event else {
                panic!("finite oblique walls must meet in arcs")
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
                for frame in [first_frame, second_frame] {
                    let axial = frame.coordinates_of(location).unwrap()[2];
                    assert!((-1e-9..=2.0 + 1e-9).contains(&axial));
                }
            }
            for parameter in [*domain.start(), *domain.end()] {
                let location = curve.evaluate(parameter).unwrap();
                let first_axial = first_frame.coordinates_of(location).unwrap()[2];
                let second_axial = second_frame.coordinates_of(location).unwrap()[2];
                assert!(
                    first_axial.abs() < 1e-9
                        || (first_axial - 2.0).abs() < 1e-9
                        || second_axial.abs() < 1e-9
                        || (second_axial - 2.0).abs() < 1e-9
                );
            }
        }
    }

    #[test]
    fn joined_skew_cylinders_keep_isolated_small_rim_tangency() {
        let tangent = (4.0_f64 - 0.5 * 0.5).sqrt();
        let ((first, first_frame), (second, second_frame)) = cylinders(
            std::f64::consts::FRAC_PI_2,
            1.5,
            -tangent - 1.0,
            1.0,
            -4.0,
            8.0,
        );
        let events =
            surface_surface_intersection_events(&first, &second, Tolerance::DEFAULT).unwrap();
        assert_eq!(events.len(), 1);
        let SurfaceSurfaceIntersectionEvent::Point(location) = events[0] else {
            panic!("the small cylinder's upper rim must touch in one point")
        };
        assert_on_walls(location, first_frame, second_frame);
        assert!((location.z() + tangent).abs() < 1e-9);
    }

    #[test]
    fn externally_tangent_skew_cylinders_keep_isolated_point() {
        let ((first, first_frame), (second, second_frame)) =
            cylinders(std::f64::consts::FRAC_PI_2, 3.0, -1.0, 2.0, -1.0, 2.0);
        let events =
            surface_surface_intersection_events(&first, &second, Tolerance::DEFAULT).unwrap();
        assert_eq!(events.len(), 1);
        let SurfaceSurfaceIntersectionEvent::Point(location) = events[0] else {
            panic!("external skew tangency must be one point")
        };
        assert_on_walls(location, first_frame, second_frame);
        assert!(location.distance_to(point(0.0, 1.0, 0.0)).unwrap() < 1e-9);

        let ((short, _), (other, _)) =
            cylinders(std::f64::consts::FRAC_PI_2, 3.0, 1.0, 1.0, -1.0, 2.0);
        assert!(
            surface_surface_intersection_events(&short, &other, Tolerance::DEFAULT)
                .unwrap()
                .is_empty()
        );
    }

    #[test]
    fn joined_skew_cylinders_work_far_from_origin_in_rotated_frames() {
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
                .translated(normal.scaled(1.5).unwrap())
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
                panic!("translated joined skew walls must meet in a curve")
            };
            assert!(curve.is_closed().unwrap());
            let domain = curve.domain();
            for index in 0..=32 {
                let parameter =
                    *domain.start() + (*domain.end() - *domain.start()) * (index as Real / 32.0);
                let location = curve.evaluate(parameter).unwrap();
                for (frame, radius) in [(first_frame, 1.0), (second_frame, 2.0)] {
                    let local = frame.coordinates_of(location).unwrap();
                    assert!((local[0].hypot(local[1]) - radius).abs() < 3e-7);
                }
            }
        }
    }
}
