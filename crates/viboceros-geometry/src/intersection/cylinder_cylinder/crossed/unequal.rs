//! Tolerance-bounded quartic sections of unequal-radius perpendicular cylinders.
//!
//! In the axes' common frame, `y²+z²=r₁²` and `x²+z²=r₂²`.
//! Parameterizing the smaller wall by an angle leaves two smooth square-root
//! branches along the larger wall's axis. Cubic Hermite interpolation is
//! bounded by the exact branch's fourth derivative.

use super::SurfaceSurfaceIntersectionEvent;
use crate::{GeometryError, NurbsCurve, Point3, Real, Tolerance, Vector3};

const MAX_SEGMENTS_PER_BRANCH: usize = 2048;

pub(super) fn intersect(
    (first_axis, first_radius, first_height, first_at_crossing): (Vector3, Real, Real, Real),
    (second_axis, second_radius, second_height, second_at_crossing): (Vector3, Real, Real, Real),
    normal: Vector3,
    crossing: Point3,
    tolerance: Tolerance,
    coordinate_roundoff: Real,
) -> Result<Vec<SurfaceSurfaceIntersectionEvent>, GeometryError> {
    let first_harmonic = first_radius > second_radius;
    let small = first_radius.min(second_radius);
    let big = first_radius.max(second_radius);
    let minimum_radicand = (big - small) * (big + small);
    let coefficient = 0.5 * small * small;
    let fit_tolerance = tolerance
        .absolute()
        .max(tolerance.relative() * big.max(first_height).max(second_height))
        .max(coordinate_roundoff);
    let derivative_bound = fourth_derivative_bound(small, coefficient, minimum_radicand);
    if !derivative_bound.is_finite() {
        return Err(GeometryError::UnsupportedSurfaceSurfaceIntersection {
            context: "unequal crossed-cylinder fit is ill-conditioned",
        });
    }
    let mut events = Vec::new();
    for sign in [-1.0, 1.0] {
        let intervals = active_intervals(
            first_harmonic,
            sign,
            small,
            big,
            first_at_crossing,
            first_height,
            second_at_crossing,
            second_height,
        );
        for angle in [
            0.0,
            std::f64::consts::FRAC_PI_2,
            std::f64::consts::PI,
            3.0 * std::f64::consts::FRAC_PI_2,
        ] {
            let (x, y, _) = coordinates(first_harmonic, sign, small, big, angle);
            let first_axial = first_at_crossing + x;
            let second_axial = second_at_crossing + y;
            let touches_rim = first_axial.abs() <= fit_tolerance
                || (first_axial - first_height).abs() <= fit_tolerance
                || second_axial.abs() <= fit_tolerance
                || (second_axial - second_height).abs() <= fit_tolerance;
            let inside_both = (-fit_tolerance..=first_height + fit_tolerance)
                .contains(&first_axial)
                && (-fit_tolerance..=second_height + fit_tolerance).contains(&second_axial);
            if touches_rim && inside_both && !angle_in_intervals(angle, &intervals) {
                events.push(SurfaceSurfaceIntersectionEvent::Point(
                    sample(
                        crossing,
                        first_axis,
                        second_axis,
                        normal,
                        first_harmonic,
                        sign,
                        small,
                        big,
                        angle,
                    )?
                    .0,
                ));
            }
        }
        for (start, end) in intervals {
            events.push(SurfaceSurfaceIntersectionEvent::Curve(fit_branch(
                crossing,
                first_axis,
                second_axis,
                normal,
                first_harmonic,
                sign,
                small,
                big,
                start,
                end,
                fit_tolerance,
                derivative_bound,
            )?));
        }
    }
    Ok(events)
}

fn fourth_derivative_bound(small: Real, coefficient: Real, minimum: Real) -> Real {
    // q(θ) = big²-small² sin²θ = A+B cos(2θ). The first four absolute
    // derivative bounds of q are 2B, 4B, 8B, and 16B.
    let b = coefficient;
    let root = minimum.sqrt();
    let root3 = minimum * root;
    let root5 = minimum * root3;
    let root7 = minimum * root5;
    let axial =
        8.0 * b / root + 28.0 * b * b / root3 + 36.0 * b * b * b / root5 + 15.0 * b.powi(4) / root7;
    // The sum also covers the tiny nonorthogonality admitted by the
    // intersection dispatch tolerance.
    small + axial
}

#[allow(clippy::too_many_arguments)]
fn active_intervals(
    first_harmonic: bool,
    sign: Real,
    small: Real,
    big: Real,
    first_at_crossing: Real,
    first_height: Real,
    second_at_crossing: Real,
    second_height: Real,
) -> Vec<(Real, Real)> {
    let turn = std::f64::consts::TAU;
    let mut angles = vec![0.0, turn];
    for (harmonic, center, height) in [
        (first_harmonic, first_at_crossing, first_height),
        (!first_harmonic, second_at_crossing, second_height),
    ] {
        for boundary in [0.0, height] {
            add_boundary_cuts(&mut angles, harmonic, sign, small, big, boundary - center);
        }
    }
    angles.sort_by(Real::total_cmp);
    angles.dedup_by(|left, right| (*left - *right).abs() <= 16.0 * Real::EPSILON);
    let mut intervals: Vec<(Real, Real)> = Vec::new();
    for pair in angles.windows(2) {
        let (x, y, _) = coordinates(first_harmonic, sign, small, big, 0.5 * (pair[0] + pair[1]));
        if (0.0..=first_height).contains(&(first_at_crossing + x))
            && (0.0..=second_height).contains(&(second_at_crossing + y))
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
    intervals
}

fn add_boundary_cuts(
    angles: &mut Vec<Real>,
    harmonic: bool,
    sign: Real,
    small: Real,
    big: Real,
    target: Real,
) {
    let turn = std::f64::consts::TAU;
    if harmonic {
        let cosine = target / small;
        if (-1.0..=1.0).contains(&cosine) {
            let angle = cosine.acos();
            angles.extend([angle, turn - angle]);
        }
    } else if sign * target > 0.0 {
        let sine_squared = ((big - target.abs()) * (big + target.abs())) / (small * small);
        if (-16.0 * Real::EPSILON..=1.0 + 16.0 * Real::EPSILON).contains(&sine_squared) {
            let angle = sine_squared.clamp(0.0, 1.0).sqrt().asin();
            angles.extend([
                angle,
                std::f64::consts::PI - angle,
                std::f64::consts::PI + angle,
                turn - angle,
            ]);
        }
    }
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

fn coordinates(
    first_harmonic: bool,
    sign: Real,
    small: Real,
    big: Real,
    angle: Real,
) -> (Real, Real, Real) {
    let (sine, cosine) = angle.sin_cos();
    let root = ((big - small * sine.abs()) * (big + small * sine.abs())).sqrt();
    if first_harmonic {
        (small * cosine, sign * root, small * sine)
    } else {
        (sign * root, small * cosine, small * sine)
    }
}

#[allow(clippy::too_many_arguments)]
fn sample(
    crossing: Point3,
    first_axis: Vector3,
    second_axis: Vector3,
    normal: Vector3,
    first_harmonic: bool,
    sign: Real,
    small: Real,
    big: Real,
    angle: Real,
) -> Result<(Point3, Vector3), GeometryError> {
    let (sine, cosine) = angle.sin_cos();
    let (x, y, z) = coordinates(first_harmonic, sign, small, big, angle);
    let root = ((big - small * sine.abs()) * (big + small * sine.abs())).sqrt();
    let root_derivative = -sign * small * small * sine * cosine / root;
    let (dx, dy) = if first_harmonic {
        (-small * sine, root_derivative)
    } else {
        (root_derivative, -small * sine)
    };
    let dz = small * cosine;
    let point = crossing
        .translated(first_axis.scaled(x)?)?
        .translated(second_axis.scaled(y)?)?
        .translated(normal.scaled(z)?)?;
    let a = first_axis.to_array();
    let b = second_axis.to_array();
    let n = normal.to_array();
    let tangent = Vector3::try_new(
        a[0] * dx + b[0] * dy + n[0] * dz,
        a[1] * dx + b[1] * dy + n[1] * dz,
        a[2] * dx + b[2] * dy + n[2] * dz,
    )?;
    Ok((point, tangent))
}

#[allow(clippy::too_many_arguments)]
fn fit_branch(
    crossing: Point3,
    first_axis: Vector3,
    second_axis: Vector3,
    normal: Vector3,
    first_harmonic: bool,
    sign: Real,
    small: Real,
    big: Real,
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
    let (first_point, mut previous_tangent) = sample(
        crossing,
        first_axis,
        second_axis,
        normal,
        first_harmonic,
        sign,
        small,
        big,
        start,
    )?;
    controls.push(first_point);
    let mut previous_point = first_point;
    let mut previous_angle = start;
    for segment in 1..=segments {
        let angle = start + (end - start) * (segment as Real / segments as Real);
        let (mut point, tangent) = sample(
            crossing,
            first_axis,
            second_axis,
            normal,
            first_harmonic,
            sign,
            small,
            big,
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Frame3, NurbsSurface, surface_surface_intersection_events};

    fn point(x: Real, y: Real, z: Real) -> Point3 {
        Point3::try_new(x, y, z).unwrap()
    }

    fn cylinders(
        first_radius: Real,
        second_radius: Real,
        first_start: Real,
        first_height: Real,
        second_start: Real,
        second_height: Real,
    ) -> ((NurbsSurface, Frame3), (NurbsSurface, Frame3)) {
        let first_frame = Frame3::try_from_normal(
            point(0.0, 0.0, first_start),
            Vector3::try_new(0.0, 0.0, 1.0).unwrap(),
            Tolerance::DEFAULT,
        )
        .unwrap();
        let second_frame = Frame3::try_from_normal(
            point(second_start, 0.0, 0.0),
            Vector3::try_new(1.0, 0.0, 0.0).unwrap(),
            Tolerance::DEFAULT,
        )
        .unwrap();
        (
            (
                NurbsSurface::try_cylinder(first_frame, first_radius, 0.0, first_height).unwrap(),
                first_frame,
            ),
            (
                NurbsSurface::try_cylinder(second_frame, second_radius, 0.0, second_height)
                    .unwrap(),
                second_frame,
            ),
        )
    }

    fn assert_on_walls(
        location: Point3,
        first_frame: Frame3,
        first_radius: Real,
        second_frame: Frame3,
        second_radius: Real,
    ) {
        for (frame, radius) in [(first_frame, first_radius), (second_frame, second_radius)] {
            let local = frame.coordinates_of(location).unwrap();
            assert!((local[0].hypot(local[1]) - radius).abs() < 2e-9);
        }
    }

    #[test]
    fn unequal_perpendicular_cylinders_make_two_smooth_closed_loops() {
        for (first_radius, second_radius) in [(1.0, 2.0), (2.0, 1.0)] {
            let ((first, first_frame), (second, second_frame)) =
                cylinders(first_radius, second_radius, -3.0, 6.0, -3.0, 6.0);
            for (left, right) in [(&first, &second), (&second, &first)] {
                let events =
                    surface_surface_intersection_events(left, right, Tolerance::DEFAULT).unwrap();
                assert_eq!(events.len(), 2);
                for event in events {
                    let SurfaceSurfaceIntersectionEvent::Curve(curve) = event else {
                        panic!("unequal walls must meet in smooth loops")
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
                            first_radius,
                            second_frame,
                            second_radius,
                        );
                    }
                }
            }
        }
    }

    #[test]
    fn unequal_perpendicular_cylinders_clip_one_branch_at_first_rim() {
        let ((first, first_frame), (second, second_frame)) =
            cylinders(1.0, 2.0, 0.0, 3.0, -3.0, 6.0);
        let events =
            surface_surface_intersection_events(&first, &second, Tolerance::DEFAULT).unwrap();
        assert_eq!(events.len(), 1);
        let SurfaceSurfaceIntersectionEvent::Curve(curve) = &events[0] else {
            panic!("one positive axial branch must remain")
        };
        assert!(curve.is_closed().unwrap());
        for index in 0..=32 {
            let domain = curve.domain();
            let parameter =
                *domain.start() + (*domain.end() - *domain.start()) * (index as Real / 32.0);
            let location = curve.evaluate(parameter).unwrap();
            assert_on_walls(location, first_frame, 1.0, second_frame, 2.0);
            assert!(first_frame.coordinates_of(location).unwrap()[2] >= -1e-9);
        }
    }

    #[test]
    fn unequal_perpendicular_cylinders_clip_both_branches_at_second_rim() {
        let ((first, first_frame), (second, second_frame)) =
            cylinders(1.0, 2.0, -3.0, 6.0, 0.0, 3.0);
        let events =
            surface_surface_intersection_events(&first, &second, Tolerance::DEFAULT).unwrap();
        assert_eq!(events.len(), 2);
        for event in events {
            let SurfaceSurfaceIntersectionEvent::Curve(curve) = event else {
                panic!("second rim must leave two open arcs")
            };
            assert!(!curve.is_closed().unwrap());
            let domain = curve.domain();
            for index in 0..=32 {
                let parameter =
                    *domain.start() + (*domain.end() - *domain.start()) * (index as Real / 32.0);
                let location = curve.evaluate(parameter).unwrap();
                assert_on_walls(location, first_frame, 1.0, second_frame, 2.0);
                assert!(second_frame.coordinates_of(location).unwrap()[2] >= -1e-9);
            }
            for parameter in [*domain.start(), *domain.end()] {
                let location = curve.evaluate(parameter).unwrap();
                assert!(second_frame.coordinates_of(location).unwrap()[2].abs() < 1e-9);
            }
        }
    }

    #[test]
    fn unequal_perpendicular_cylinders_keep_isolated_rim_points() {
        let ((first, _), (second, _)) = cylinders(1.0, 2.0, 2.0, 1.0, -3.0, 6.0);
        let events =
            surface_surface_intersection_events(&first, &second, Tolerance::DEFAULT).unwrap();
        assert_eq!(events.len(), 2);
        let mut second_axis_coordinates = Vec::new();
        for event in events {
            let SurfaceSurfaceIntersectionEvent::Point(location) = event else {
                panic!("rim contacts must be isolated points")
            };
            assert!((location.z() - 2.0).abs() < 1e-9);
            second_axis_coordinates.push(location.x());
        }
        second_axis_coordinates.sort_by(Real::total_cmp);
        assert!((second_axis_coordinates[0] + 1.0).abs() < 1e-9);
        assert!((second_axis_coordinates[1] - 1.0).abs() < 1e-9);
    }

    #[test]
    fn unequal_perpendicular_cylinders_split_at_square_root_height_band() {
        let ((first, first_frame), (second, second_frame)) =
            cylinders(1.0, 2.0, 1.8, 0.1, -3.0, 6.0);
        let events =
            surface_surface_intersection_events(&first, &second, Tolerance::DEFAULT).unwrap();
        assert_eq!(events.len(), 4);
        for event in events {
            let SurfaceSurfaceIntersectionEvent::Curve(curve) = event else {
                panic!("height band must cut the square-root branch into four arcs")
            };
            assert!(!curve.is_closed().unwrap());
            let domain = curve.domain();
            for index in 0..=32 {
                let parameter =
                    *domain.start() + (*domain.end() - *domain.start()) * (index as Real / 32.0);
                let location = curve.evaluate(parameter).unwrap();
                assert_on_walls(location, first_frame, 1.0, second_frame, 2.0);
                assert!((1.8 - 1e-9..=1.9 + 1e-9).contains(&location.z()));
            }
            for parameter in [*domain.start(), *domain.end()] {
                let z = curve.evaluate(parameter).unwrap().z();
                assert!((z - 1.8).abs() < 1e-9 || (z - 1.9).abs() < 1e-9);
            }
        }
    }

    #[test]
    fn unequal_perpendicular_cylinders_reject_rim_contacts_outside_other_height() {
        let ((first, _), (second, _)) = cylinders(1.0, 2.0, 2.0, 1.0, 2.0, 1.0);
        assert!(
            surface_surface_intersection_events(&first, &second, Tolerance::DEFAULT)
                .unwrap()
                .is_empty()
        );
    }

    #[test]
    fn unequal_perpendicular_cylinders_work_far_from_origin_in_rotated_frames() {
        let crossing = point(1.0e8, -1.0e8, 1.0e8);
        let base = Frame3::try_from_normal(
            crossing,
            Vector3::try_new(1.0, 2.0, 3.0).unwrap(),
            Tolerance::DEFAULT,
        )
        .unwrap();
        let first_frame = Frame3::try_from_normal(
            crossing
                .translated(base.z_axis().as_vector().scaled(-3.0).unwrap())
                .unwrap(),
            base.z_axis().as_vector(),
            Tolerance::DEFAULT,
        )
        .unwrap();
        let second_frame = Frame3::try_from_normal(
            crossing
                .translated(base.x_axis().as_vector().scaled(-3.0).unwrap())
                .unwrap(),
            base.x_axis().as_vector(),
            Tolerance::DEFAULT,
        )
        .unwrap();
        let first = NurbsSurface::try_cylinder(first_frame, 1.0, 0.0, 6.0).unwrap();
        let second = NurbsSurface::try_cylinder(second_frame, 2.0, 0.0, 6.0).unwrap();
        let events =
            surface_surface_intersection_events(&first, &second, Tolerance::DEFAULT).unwrap();
        assert_eq!(events.len(), 2);
        for event in events {
            let SurfaceSurfaceIntersectionEvent::Curve(curve) = event else {
                panic!("rotated unequal walls must meet in two loops")
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
