//! Smooth, tolerance-bounded cylinder intersections with separated square-root branches.
//!
//! In the frame of the smaller cylinder, the larger wall leaves a positive
//! square root at every angle. Its two signs are separate closed branches.
//! Finite-height cuts reduce to quadratic or quartic equations.

use super::{SurfaceSurfaceIntersectionEvent, skew_clip};
use crate::{GeometryError, NurbsCurve, Point3, Real, Tolerance, Vector3};

const MAX_SEGMENTS_PER_BRANCH: usize = 2048;

#[derive(Clone, Copy)]
struct Basis {
    crossing: Point3,
    small_axis: Vector3,
    plane_axis: Vector3,
    normal: Vector3,
    cosine: Real,
    sine: Real,
    small: Real,
    big: Real,
    minimum: Real,
    miss: Real,
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
    (first_axis, first_radius, first_height, first_at_crossing, first_closest): (
        Vector3,
        Real,
        Real,
        Real,
        Point3,
    ),
    (second_axis, second_radius, second_height, second_at_crossing, second_closest): (
        Vector3,
        Real,
        Real,
        Real,
        Point3,
    ),
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
    ) = if first_radius < second_radius {
        (
            first_axis,
            first_radius,
            first_height,
            first_at_crossing,
            first_closest,
            second_axis,
            second_radius,
            second_height,
            second_at_crossing,
        )
    } else {
        (
            second_axis,
            second_radius,
            second_height,
            second_at_crossing,
            second_closest,
            first_axis,
            first_radius,
            first_height,
            first_at_crossing,
        )
    };
    let cosine = small_axis.dot(big_axis)?;
    let cross = small_axis.cross(big_axis)?;
    let sine = cross.length()?;
    if sine <= tolerance.angular() {
        return Err(GeometryError::UnsupportedSurfaceSurfaceIntersection {
            context: "nearly parallel unequal cylinder axes",
        });
    }
    let small_vector = small_axis.to_array();
    let big_vector = big_axis.to_array();
    let plane_axis = Vector3::try_new(
        (big_vector[0] - cosine * small_vector[0]) / sine,
        (big_vector[1] - cosine * small_vector[1]) / sine,
        (big_vector[2] - cosine * small_vector[2]) / sine,
    )?;
    let radial_reach = miss.abs() + small;
    let minimum = (big - radial_reach) * (big + radial_reach);
    if minimum <= 0.0 {
        return Err(GeometryError::UnsupportedSurfaceSurfaceIntersection {
            context: "skew cylinder square-root branches meet",
        });
    }
    let basis = Basis {
        crossing: small_closest,
        small_axis,
        plane_axis,
        normal: cross.scaled(1.0 / sine)?,
        cosine,
        sine,
        small,
        big,
        minimum,
        miss,
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
            context: "oblique unequal-cylinder curve fit is ill-conditioned",
        });
    }
    let mut events = Vec::new();
    for sign in [-1.0, 1.0] {
        let AngularClip { intervals, cuts } = active_intervals(basis, clip, sign, fit_tolerance)?;
        for angle in cuts {
            let (small_axial, big_axial) = basis.axials(sign, angle);
            let small_axial = clip.small_center + small_axial;
            let big_axial = clip.big_center + big_axial;
            let inside = (-fit_tolerance..=clip.small_height + fit_tolerance)
                .contains(&small_axial)
                && (-fit_tolerance..=clip.big_height + fit_tolerance).contains(&big_axial);
            let on_rim = small_axial.abs() <= fit_tolerance
                || (small_axial - clip.small_height).abs() <= fit_tolerance
                || big_axial.abs() <= fit_tolerance
                || (big_axial - clip.big_height).abs() <= fit_tolerance;
            if inside && on_rim && !angle_in_intervals(angle, &intervals) {
                let point = basis.sample(sign, angle)?.0;
                if !events.iter().any(|event| {
                    matches!(event, SurfaceSurfaceIntersectionEvent::Point(existing)
                        if existing.distance_to(point).is_ok_and(|distance| distance <= fit_tolerance))
                }) {
                    events.push(SurfaceSurfaceIntersectionEvent::Point(point));
                }
            }
        }
        for (start, end) in intervals {
            events.push(SurfaceSurfaceIntersectionEvent::Curve(fit_branch(
                basis,
                sign,
                start,
                end,
                fit_tolerance,
                derivative_bound,
            )?));
        }
    }
    Ok(events)
}

impl Basis {
    fn root(self, angle: Real) -> Real {
        let radial = (self.small * angle.sin() - self.miss).abs();
        ((self.big - radial) * (self.big + radial)).sqrt()
    }

    fn axials(self, sign: Real, angle: Real) -> (Real, Real) {
        let y = self.small * angle.cos();
        let w = sign * self.root(angle);
        (
            (self.cosine * y - w) / self.sine,
            (y - self.cosine * w) / self.sine,
        )
    }

    fn sample(self, sign: Real, angle: Real) -> Result<(Point3, Vector3), GeometryError> {
        let (sine, cosine) = angle.sin_cos();
        let y = self.small * cosine;
        let z = self.small * sine;
        let root = self.root(angle);
        let w = sign * root;
        let x = (self.cosine * y - w) / self.sine;
        let y_tangent = -self.small * sine;
        let z_tangent = self.small * cosine;
        let w_tangent = -sign * (z - self.miss) * z_tangent / root;
        let x_tangent = (self.cosine * y_tangent - w_tangent) / self.sine;
        let point = self
            .crossing
            .translated(self.small_axis.scaled(x)?)?
            .translated(self.plane_axis.scaled(y)?)?
            .translated(self.normal.scaled(z)?)?;
        let a = self.small_axis.to_array();
        let e = self.plane_axis.to_array();
        let n = self.normal.to_array();
        let tangent = Vector3::try_new(
            a[0] * x_tangent + e[0] * y_tangent + n[0] * z_tangent,
            a[1] * x_tangent + e[1] * y_tangent + n[1] * z_tangent,
            a[2] * x_tangent + e[2] * y_tangent + n[2] * z_tangent,
        )?;
        Ok((point, tangent))
    }
}

fn fourth_derivative_bound(basis: Basis) -> Real {
    let r2 = basis.small * basis.small;
    let cross_term = 2.0 * basis.small * basis.miss.abs();
    let first = r2 + cross_term;
    let second = 2.0 * r2 + cross_term;
    let third = 4.0 * r2 + cross_term;
    let fourth = 8.0 * r2 + cross_term;
    let root = basis.minimum.sqrt();
    let root3 = basis.minimum * root;
    let root5 = basis.minimum * root3;
    let root7 = basis.minimum * root5;
    let root_bound = fourth / (2.0 * root)
        + (first * third + 0.75 * second * second) / root3
        + 2.25 * first * first * second / root5
        + 0.9375 * first.powi(4) / root7;
    basis.small + (basis.cosine.abs() * basis.small + root_bound) / basis.sine
}

fn active_intervals(
    basis: Basis,
    clip: Clip,
    sign: Real,
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
    let small_coefficients = (basis.cosine * basis.small / basis.sine, -sign / basis.sine);
    let big_coefficients = (basis.small / basis.sine, -basis.cosine * sign / basis.sine);
    for (coefficients, center, height, small_axial) in [
        (
            small_coefficients,
            clip.small_center,
            clip.small_height,
            true,
        ),
        (big_coefficients, clip.big_center, clip.big_height, false),
    ] {
        for boundary in [0.0, height] {
            if basis.miss == 0.0 {
                add_boundary_angles(
                    &mut angles,
                    basis,
                    sign,
                    coefficients,
                    boundary - center,
                    small_axial,
                    fit_tolerance,
                )?;
            } else {
                angles.extend(skew_clip::boundary_angles(
                    (basis.small, basis.big, basis.miss),
                    coefficients,
                    boundary - center,
                    fit_tolerance,
                )?);
            }
        }
    }
    angles.sort_by(Real::total_cmp);
    angles.dedup_by(|left, right| (*left - *right).abs() <= 16.0 * Real::EPSILON);
    let mut intervals: Vec<(Real, Real)> = Vec::new();
    for pair in angles.windows(2) {
        let (small_axial, big_axial) = basis.axials(sign, 0.5 * (pair[0] + pair[1]));
        if (0.0..=clip.small_height).contains(&(clip.small_center + small_axial))
            && (0.0..=clip.big_height).contains(&(clip.big_center + big_axial))
        {
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
    Ok(AngularClip {
        intervals,
        cuts: angles,
    })
}

#[allow(clippy::too_many_arguments)]
fn add_boundary_angles(
    angles: &mut Vec<Real>,
    basis: Basis,
    sign: Real,
    (linear, root): (Real, Real),
    target: Real,
    small_axial: bool,
    fit_tolerance: Real,
) -> Result<(), GeometryError> {
    // (target-A c)^2 = B^2 (min+small^2 c^2), c=cos(theta).
    let a = if small_axial {
        -basis.small * basis.small
    } else {
        basis.small * basis.small
    };
    let b = -2.0 * target * linear;
    let c = target * target - root * root * basis.minimum;
    let discriminant = b * b - 4.0 * a * c;
    if ![a, b, c, discriminant]
        .iter()
        .all(|value| value.is_finite())
    {
        return Err(GeometryError::UnsupportedSurfaceSurfaceIntersection {
            context: "oblique unequal-cylinder clipping is ill-conditioned",
        });
    }
    let discriminant_scale = b * b + (4.0 * a * c).abs();
    let discriminant_roundoff = 64.0 * Real::EPSILON * discriminant_scale;
    if discriminant < -discriminant_roundoff {
        return Ok(());
    }
    let roots = if discriminant.abs() <= discriminant_roundoff {
        vec![-b / (2.0 * a)]
    } else {
        let square_root = discriminant.sqrt();
        let q = -0.5 * (b + if b >= 0.0 { square_root } else { -square_root });
        if q == 0.0 {
            vec![-b / (2.0 * a)]
        } else {
            vec![q / a, c / q]
        }
    };
    for cosine in roots {
        if !(-1.0 - 32.0 * Real::EPSILON..=1.0 + 32.0 * Real::EPSILON).contains(&cosine) {
            continue;
        }
        let mut cosine = cosine.clamp(-1.0, 1.0);
        for endpoint in [-1.0, 1.0] {
            if (cosine - endpoint).abs() <= 64.0 * Real::EPSILON {
                let endpoint_value =
                    linear * endpoint + root * (basis.minimum + basis.small * basis.small).sqrt();
                if (endpoint_value - target).abs() <= fit_tolerance {
                    cosine = endpoint;
                }
            }
        }
        let exact = if small_axial {
            basis.axials(sign, cosine.acos()).0
        } else {
            basis.axials(sign, cosine.acos()).1
        };
        if (exact - target).abs() <= fit_tolerance {
            let angle = cosine.acos();
            angles.extend([angle, std::f64::consts::TAU - angle]);
        }
    }
    Ok(())
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

fn fit_branch(
    basis: Basis,
    sign: Real,
    start: Real,
    end: Real,
    fit_tolerance: Real,
    derivative_bound: Real,
) -> Result<NurbsCurve, GeometryError> {
    // Cubic Hermite remainder: ||error|| <= M h^4 / 384.
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
    let (first_point, mut previous_tangent) = basis.sample(sign, start)?;
    controls.push(first_point);
    let mut previous_point = first_point;
    let mut previous_angle = start;
    for segment in 1..=segments {
        let angle = if segment == segments {
            end
        } else {
            start + (end - start) * (segment as Real / segments as Real)
        };
        let (mut point, tangent) = basis.sample(sign, angle)?;
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
        first_start: Real,
        first_height: Real,
        second_start: Real,
        second_height: Real,
    ) -> ((NurbsSurface, Frame3), (NurbsSurface, Frame3)) {
        cylinders_with_miss(
            angle,
            0.0,
            first_start,
            first_height,
            second_start,
            second_height,
        )
    }

    fn cylinders_with_miss(
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
    fn skew_separated_cylinders_make_two_closed_curves() {
        for angle in [
            std::f64::consts::FRAC_PI_2,
            std::f64::consts::FRAC_PI_3,
            2.0 * std::f64::consts::FRAC_PI_3,
        ] {
            for miss in [-0.5, 0.5] {
                let ((first, first_frame), (second, second_frame)) =
                    cylinders_with_miss(angle, miss, -4.0, 8.0, -4.0, 8.0);
                for (left, right) in [(&first, &second), (&second, &first)] {
                    let events =
                        surface_surface_intersection_events(left, right, Tolerance::DEFAULT)
                            .unwrap();
                    assert_eq!(events.len(), 2);
                    for event in events {
                        let SurfaceSurfaceIntersectionEvent::Curve(curve) = event else {
                            panic!("separated skew walls must meet in two curves")
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
    }

    #[test]
    fn skew_separated_cylinders_clip_at_big_cylinder_rims() {
        let ((first, first_frame), (second, second_frame)) =
            cylinders_with_miss(std::f64::consts::FRAC_PI_3, 0.5, -4.0, 8.0, 0.0, 1.0);
        let events =
            surface_surface_intersection_events(&first, &second, Tolerance::DEFAULT).unwrap();
        assert_eq!(events.len(), 3);
        for event in events {
            let SurfaceSurfaceIntersectionEvent::Curve(curve) = event else {
                panic!("skew cylinder rim cuts must make arcs")
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
                let axial = second_frame.coordinates_of(location).unwrap()[2];
                assert!((-1e-9..=1.0 + 1e-9).contains(&axial));
            }
            for parameter in [*domain.start(), *domain.end()] {
                let axial = second_frame
                    .coordinates_of(curve.evaluate(parameter).unwrap())
                    .unwrap()[2];
                assert!(axial.abs() < 1e-9 || (axial - 1.0).abs() < 1e-9);
            }
        }
    }

    #[test]
    fn skew_separated_cylinders_clip_with_negative_axis_miss() {
        let ((first, first_frame), (second, second_frame)) =
            cylinders_with_miss(std::f64::consts::FRAC_PI_3, -0.5, -4.0, 8.0, 0.0, 1.0);
        let events =
            surface_surface_intersection_events(&second, &first, Tolerance::DEFAULT).unwrap();
        assert_eq!(events.len(), 3);
        for event in events {
            let SurfaceSurfaceIntersectionEvent::Curve(curve) = event else {
                panic!("negative skew offset must clip into arcs")
            };
            assert!(!curve.is_closed().unwrap());
            let domain = curve.domain();
            for parameter in [*domain.start(), *domain.end()] {
                let location = curve.evaluate(parameter).unwrap();
                assert_on_walls(location, first_frame, second_frame);
                let axial = second_frame.coordinates_of(location).unwrap()[2];
                assert!(axial.abs() < 1e-9 || (axial - 1.0).abs() < 1e-9);
            }
        }
    }

    #[test]
    fn skew_separated_cylinders_keep_two_interior_rim_contacts() {
        let ((first, first_frame), (second, second_frame)) =
            cylinders_with_miss(std::f64::consts::FRAC_PI_2, 0.5, -3.0, 1.0, -4.0, 8.0);
        let events =
            surface_surface_intersection_events(&first, &second, Tolerance::DEFAULT).unwrap();
        assert_eq!(events.len(), 2);
        for event in events {
            let SurfaceSurfaceIntersectionEvent::Point(location) = event else {
                panic!("the small cylinder's upper rim must touch in two points")
            };
            assert_on_walls(location, first_frame, second_frame);
            assert!((location.z() + 2.0).abs() < 1e-9);
        }
    }

    #[test]
    fn skew_separated_cylinders_discard_rim_contacts_outside_other_height() {
        let ((first, _), (second, _)) =
            cylinders_with_miss(std::f64::consts::FRAC_PI_2, 0.5, -3.0, 1.0, 0.0, 0.4);
        assert!(
            surface_surface_intersection_events(&first, &second, Tolerance::DEFAULT)
                .unwrap()
                .is_empty()
        );
    }

    #[test]
    fn skew_cylinders_at_internal_branch_tangency_remain_unsupported() {
        let ((first, _), (second, _)) =
            cylinders_with_miss(std::f64::consts::FRAC_PI_3, 1.0, -4.0, 8.0, -4.0, 8.0);
        assert!(matches!(
            surface_surface_intersection_events(&first, &second, Tolerance::DEFAULT),
            Err(GeometryError::UnsupportedSurfaceSurfaceIntersection { .. })
        ));
    }

    #[test]
    fn oblique_unequal_cylinders_make_two_closed_curves() {
        for angle in [
            std::f64::consts::FRAC_PI_3,
            2.0 * std::f64::consts::FRAC_PI_3,
        ] {
            let ((first, first_frame), (second, second_frame)) =
                cylinders(angle, -4.0, 8.0, -4.0, 8.0);
            for (left, right) in [(&first, &second), (&second, &first)] {
                let events =
                    surface_surface_intersection_events(left, right, Tolerance::DEFAULT).unwrap();
                assert_eq!(events.len(), 2);
                for event in events {
                    let SurfaceSurfaceIntersectionEvent::Curve(curve) = event else {
                        panic!("unequal oblique walls must meet in two curves")
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
    fn oblique_unequal_cylinders_clip_one_branch_at_small_rim() {
        let ((first, first_frame), (second, second_frame)) =
            cylinders(std::f64::consts::FRAC_PI_3, 0.0, 4.0, -4.0, 8.0);
        let events =
            surface_surface_intersection_events(&first, &second, Tolerance::DEFAULT).unwrap();
        assert_eq!(events.len(), 1);
        let SurfaceSurfaceIntersectionEvent::Curve(curve) = &events[0] else {
            panic!("positive branch must remain a curve")
        };
        assert!(curve.is_closed().unwrap());
        let domain = curve.domain();
        for index in 0..=32 {
            let parameter =
                *domain.start() + (*domain.end() - *domain.start()) * (index as Real / 32.0);
            let location = curve.evaluate(parameter).unwrap();
            assert_on_walls(location, first_frame, second_frame);
            assert!(first_frame.coordinates_of(location).unwrap()[2] >= -1e-9);
        }
    }

    #[test]
    fn oblique_unequal_cylinders_keep_tangent_point_at_big_rim() {
        let ((first, first_frame), (second, second_frame)) =
            cylinders(std::f64::consts::FRAC_PI_3, -4.0, 8.0, 0.0, 4.0);
        let events =
            surface_surface_intersection_events(&first, &second, Tolerance::DEFAULT).unwrap();
        assert_eq!(events.len(), 2);
        assert!(
            matches!(&events[0], SurfaceSurfaceIntersectionEvent::Curve(curve) if curve.is_closed().unwrap())
        );
        let SurfaceSurfaceIntersectionEvent::Point(location) = events[1] else {
            panic!("the opposite branch must touch the second rim at a point")
        };
        assert_on_walls(location, first_frame, second_frame);
        assert!(second_frame.coordinates_of(location).unwrap()[2].abs() < 1e-9);
    }

    #[test]
    fn oblique_unequal_cylinders_clip_at_two_small_cylinder_rims() {
        let ((first, first_frame), (second, second_frame)) =
            cylinders(std::f64::consts::FRAC_PI_3, 2.1, 0.4, -4.0, 8.0);
        let events =
            surface_surface_intersection_events(&first, &second, Tolerance::DEFAULT).unwrap();
        assert_eq!(events.len(), 2);
        for event in events {
            let SurfaceSurfaceIntersectionEvent::Curve(curve) = event else {
                panic!("small cylinder band must leave two arcs")
            };
            assert!(!curve.is_closed().unwrap());
            let domain = curve.domain();
            for index in 0..=32 {
                let parameter =
                    *domain.start() + (*domain.end() - *domain.start()) * (index as Real / 32.0);
                let location = curve.evaluate(parameter).unwrap();
                assert_on_walls(location, first_frame, second_frame);
                assert!((2.1 - 1e-9..=2.5 + 1e-9).contains(&location.z()));
            }
            for parameter in [*domain.start(), *domain.end()] {
                let z = curve.evaluate(parameter).unwrap().z();
                assert!((z - 2.1).abs() < 1e-9 || (z - 2.5).abs() < 1e-9);
            }
        }
    }

    #[test]
    fn oblique_unequal_cylinders_clip_at_two_large_cylinder_rims() {
        let ((first, first_frame), (second, second_frame)) =
            cylinders(std::f64::consts::FRAC_PI_3, -4.0, 8.0, 0.5, 1.0);
        let events =
            surface_surface_intersection_events(&first, &second, Tolerance::DEFAULT).unwrap();
        assert_eq!(events.len(), 2);
        for event in events {
            let SurfaceSurfaceIntersectionEvent::Curve(curve) = event else {
                panic!("large cylinder band must leave two arcs")
            };
            assert!(!curve.is_closed().unwrap());
            let domain = curve.domain();
            for index in 0..=32 {
                let parameter =
                    *domain.start() + (*domain.end() - *domain.start()) * (index as Real / 32.0);
                let location = curve.evaluate(parameter).unwrap();
                assert_on_walls(location, first_frame, second_frame);
                let axial = second_frame.coordinates_of(location).unwrap()[2];
                assert!((-1e-9..=1.0 + 1e-9).contains(&axial));
            }
            for parameter in [*domain.start(), *domain.end()] {
                let axial = second_frame
                    .coordinates_of(curve.evaluate(parameter).unwrap())
                    .unwrap()[2];
                assert!(axial.abs() < 1e-9 || (axial - 1.0).abs() < 1e-9);
            }
        }
    }

    #[test]
    fn oblique_unequal_cylinders_keep_interior_axial_tangencies_as_points() {
        let (sine, cosine) = std::f64::consts::FRAC_PI_3.sin_cos();
        let first_frame = Frame3::try_from_normal(
            point(0.0, 0.0, (1.2_f64 * 1.2 - 1.0).sqrt() - 1.0),
            Vector3::try_new(0.0, 0.0, 1.0).unwrap(),
            Tolerance::DEFAULT,
        )
        .unwrap();
        let second_frame = Frame3::try_from_normal(
            point(-3.0 * sine, 0.0, -3.0 * cosine),
            Vector3::try_new(sine, 0.0, cosine).unwrap(),
            Tolerance::DEFAULT,
        )
        .unwrap();
        let first = NurbsSurface::try_cylinder(first_frame, 1.0, 0.0, 1.0).unwrap();
        let second = NurbsSurface::try_cylinder(second_frame, 1.2, 0.0, 6.0).unwrap();
        let events =
            surface_surface_intersection_events(&first, &second, Tolerance::DEFAULT).unwrap();
        assert_eq!(events.len(), 2);
        for event in events {
            let SurfaceSurfaceIntersectionEvent::Point(location) = event else {
                panic!("interior axial tangencies must be isolated points")
            };
            for (frame, radius) in [(first_frame, 1.0), (second_frame, 1.2)] {
                let local = frame.coordinates_of(location).unwrap();
                assert!((local[0].hypot(local[1]) - radius).abs() < 1e-9);
            }
            assert!((first_frame.coordinates_of(location).unwrap()[2] - 1.0).abs() < 1e-9);
        }
    }

    #[test]
    fn oblique_unequal_cylinders_work_far_from_origin_in_rotated_frames() {
        let crossing = point(1.0e8, -1.0e8, 1.0e8);
        let base = Frame3::try_from_normal(
            crossing,
            Vector3::try_new(1.0, 2.0, 3.0).unwrap(),
            Tolerance::DEFAULT,
        )
        .unwrap();
        let (sine, cosine) = std::f64::consts::FRAC_PI_3.sin_cos();
        let first_axis = base.z_axis().as_vector();
        let x_axis = base.x_axis().as_vector();
        let second_axis = Vector3::try_new(
            cosine * first_axis.x() + sine * x_axis.x(),
            cosine * first_axis.y() + sine * x_axis.y(),
            cosine * first_axis.z() + sine * x_axis.z(),
        )
        .unwrap();
        let first_frame = Frame3::try_from_normal(
            crossing
                .translated(first_axis.scaled(-4.0).unwrap())
                .unwrap(),
            first_axis,
            Tolerance::DEFAULT,
        )
        .unwrap();
        let second_frame = Frame3::try_from_normal(
            crossing
                .translated(second_axis.scaled(-4.0).unwrap())
                .unwrap(),
            second_axis,
            Tolerance::DEFAULT,
        )
        .unwrap();
        let first = NurbsSurface::try_cylinder(first_frame, 1.0, 0.0, 8.0).unwrap();
        let second = NurbsSurface::try_cylinder(second_frame, 2.0, 0.0, 8.0).unwrap();
        let events =
            surface_surface_intersection_events(&first, &second, Tolerance::DEFAULT).unwrap();
        assert_eq!(events.len(), 2);
        for event in events {
            let SurfaceSurfaceIntersectionEvent::Curve(curve) = event else {
                panic!("rotated oblique walls must meet in curves")
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

    #[test]
    fn skew_separated_cylinders_work_far_from_origin_in_rotated_frames() {
        let crossing = point(1.0e8, -1.0e8, 1.0e8);
        let base = Frame3::try_from_normal(
            crossing,
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
            crossing
                .translated(first_axis.scaled(-4.0).unwrap())
                .unwrap(),
            first_axis,
            Tolerance::DEFAULT,
        )
        .unwrap();
        let second_frame = Frame3::try_from_normal(
            crossing
                .translated(normal.scaled(0.5).unwrap())
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
            assert_eq!(events.len(), 2);
            for event in events {
                let SurfaceSurfaceIntersectionEvent::Curve(curve) = event else {
                    panic!("translated skew walls must meet in curves")
                };
                assert!(curve.is_closed().unwrap());
                let domain = curve.domain();
                for index in 0..=32 {
                    let parameter = *domain.start()
                        + (*domain.end() - *domain.start()) * (index as Real / 32.0);
                    let location = curve.evaluate(parameter).unwrap();
                    for (frame, radius) in [(first_frame, 1.0), (second_frame, 2.0)] {
                        let local = frame.coordinates_of(location).unwrap();
                        assert!((local[0].hypot(local[1]) - radius).abs() < 3e-7);
                    }
                }
            }
        }
    }
}
