//! Intersections of finite cylinders with nonparallel crossing or skew axes.

mod joined;
mod separated;
mod skew_clip;
mod unequal;

use super::SurfaceSurfaceIntersectionEvent;
use crate::{Frame3, GeometryError, NurbsCurve, Point3, Real, Tolerance, Vector3, WeightedPoint3};

pub(super) fn intersect(
    (first_frame, first_radius, first_height): (Frame3, Real, Real),
    (second_frame, second_radius, second_height): (Frame3, Real, Real),
    tolerance: Tolerance,
) -> Result<Vec<SurfaceSurfaceIntersectionEvent>, GeometryError> {
    let first_axis = first_frame.z_axis().as_vector();
    let second_axis = second_frame.z_axis().as_vector();
    let cross = first_axis.cross(second_axis)?;
    let cross_length = cross.length()?;
    let axis_dot = first_axis.dot(second_axis)?;
    let axis_sum = combine_axes(first_axis, second_axis, 1.0)?;
    let axis_difference = combine_axes(first_axis, second_axis, -1.0)?;
    let sum_length = axis_sum.length()?;
    let difference_length = axis_difference.length()?;
    let cosine_half = 0.5 * sum_length;
    let sine_half = 0.5 * difference_length;
    let radius = 0.5 * (first_radius + second_radius);
    let scale = first_radius
        .max(second_radius)
        .max(first_height)
        .max(second_height);
    let spatial_tolerance = tolerance.absolute().max(tolerance.relative() * scale);
    let roundoff = 8.0
        * Real::EPSILON
        * first_frame
            .origin()
            .to_array()
            .into_iter()
            .chain(second_frame.origin().to_array())
            .map(Real::abs)
            .fold(scale, Real::max);
    let equal_radii = (first_radius - second_radius).abs() <= roundoff;
    if cosine_half <= tolerance.angular() || sine_half <= tolerance.angular() {
        return Err(GeometryError::UnsupportedSurfaceSurfaceIntersection {
            context: "nearly parallel crossed cylinder axes",
        });
    }
    let normal = cross.scaled(1.0 / cross_length)?;
    let displacement = first_frame.origin().vector_to(second_frame.origin())?;
    let axis_miss = displacement.dot(normal)?;
    if axis_miss.abs() > first_radius + second_radius + spatial_tolerance.max(roundoff) {
        return Ok(Vec::new());
    }
    if finite_cylinder_bounds_are_disjoint(
        (first_frame, first_radius, first_height),
        (second_frame, second_radius, second_height),
        spatial_tolerance.max(roundoff),
    )? {
        return Ok(Vec::new());
    }
    let first_projection = displacement.dot(first_axis)?;
    let second_projection = displacement.dot(second_axis)?;
    let denominator = 1.0 - axis_dot * axis_dot;
    let first_at_crossing = (first_projection - axis_dot * second_projection) / denominator;
    let second_at_crossing = (axis_dot * first_projection - second_projection) / denominator;
    let crossing = first_frame
        .origin()
        .translated(first_axis.scaled(first_at_crossing)?)?;
    if axis_miss.abs() > spatial_tolerance.max(roundoff) {
        let second_closest = second_frame
            .origin()
            .translated(second_axis.scaled(second_at_crossing)?)?;
        let first = (
            first_axis,
            first_radius,
            first_height,
            first_at_crossing,
            crossing,
        );
        let second = (
            second_axis,
            second_radius,
            second_height,
            second_at_crossing,
            second_closest,
        );
        let radial_roundoff = roundoff
            .max(64.0 * Real::EPSILON * first_radius.max(second_radius).max(axis_miss.abs()));
        if first_radius.max(second_radius)
            > first_radius.min(second_radius) + axis_miss.abs() + radial_roundoff
        {
            return separated::intersect(first, second, axis_miss, tolerance, roundoff);
        }
        return joined::intersect(first, second, axis_miss, tolerance, roundoff);
    }
    if !equal_radii {
        if axis_dot.abs() * scale > spatial_tolerance.max(roundoff) {
            return separated::intersect(
                (
                    first_axis,
                    first_radius,
                    first_height,
                    first_at_crossing,
                    crossing,
                ),
                (
                    second_axis,
                    second_radius,
                    second_height,
                    second_at_crossing,
                    crossing,
                ),
                0.0,
                tolerance,
                roundoff,
            );
        }
        return unequal::intersect(
            (first_axis, first_radius, first_height, first_at_crossing),
            (
                second_axis,
                second_radius,
                second_height,
                second_at_crossing,
            ),
            normal,
            crossing,
            tolerance,
            roundoff,
        );
    }

    let mut events = Vec::new();
    // Equal-radius wall equations differ by a product of the two bisector
    // coordinates, so each zero factor gives one exact ellipse.
    for (bisector, semi_axis, first_slope, second_slope) in [
        (
            axis_sum,
            radius / sine_half,
            radius * cosine_half / sine_half,
            radius * cosine_half / sine_half,
        ),
        (
            axis_difference,
            radius / cosine_half,
            radius * sine_half / cosine_half,
            -radius * sine_half / cosine_half,
        ),
    ] {
        let bisector_length = bisector.length()?;
        let cosine_axis = bisector.scaled(semi_axis / bisector_length)?;
        let first_limits = cosine_limits(first_at_crossing, first_slope, first_height);
        let second_limits = cosine_limits(second_at_crossing, second_slope, second_height);
        let lower = first_limits.0.max(second_limits.0).max(-1.0);
        let upper = first_limits.1.min(second_limits.1).min(1.0);
        if lower > upper + spatial_tolerance / semi_axis {
            continue;
        }
        let sine_axis = normal.scaled(radius)?;
        if upper - lower <= spatial_tolerance / semi_axis {
            let cosine = (lower + upper) * 0.5;
            if !(-1.0..=1.0).contains(&cosine) {
                continue;
            }
            let angle = cosine.acos();
            for parameter in [angle, std::f64::consts::TAU - angle] {
                let point = ellipse_point(crossing, cosine_axis, sine_axis, parameter, 1.0)?;
                if !events.iter().any(|event| {
                    matches!(event, SurfaceSurfaceIntersectionEvent::Point(existing)
                        if existing.distance_to(point).is_ok_and(|distance| distance <= spatial_tolerance))
                }) {
                    events.push(SurfaceSurfaceIntersectionEvent::Point(point));
                }
            }
            continue;
        }
        let lower = lower.max(-1.0);
        let upper = upper.min(1.0);
        let alpha = upper.acos();
        let beta = lower.acos();
        let turn = std::f64::consts::TAU;
        let intervals = if alpha == 0.0 && beta == std::f64::consts::PI {
            vec![(0.0, turn)]
        } else if alpha == 0.0 {
            vec![(-beta, beta)]
        } else if beta == std::f64::consts::PI {
            vec![(alpha, turn - alpha)]
        } else {
            vec![(alpha, beta), (turn - beta, turn - alpha)]
        };
        for (start, end) in intervals {
            events.push(SurfaceSurfaceIntersectionEvent::Curve(ellipse_arc(
                crossing,
                cosine_axis,
                sine_axis,
                start,
                end,
            )?));
        }
    }
    Ok(events)
}

fn finite_cylinder_bounds_are_disjoint(
    (first_frame, first_radius, first_height): (Frame3, Real, Real),
    (second_frame, second_radius, second_height): (Frame3, Real, Real),
    tolerance: Real,
) -> Result<bool, GeometryError> {
    let bounds =
        |frame: Frame3, radius: Real, height: Real| -> Result<[(Real, Real); 3], GeometryError> {
            let start = frame.origin().to_array();
            let end = frame
                .origin()
                .translated(frame.z_axis().as_vector().scaled(height)?)?
                .to_array();
            let axis = frame.z_axis().as_vector().to_array();
            Ok(std::array::from_fn(|index| {
                let radial = radius * (1.0 - axis[index] * axis[index]).max(0.0).sqrt();
                (
                    start[index].min(end[index]) - radial,
                    start[index].max(end[index]) + radial,
                )
            }))
        };
    let first = bounds(first_frame, first_radius, first_height)?;
    let second = bounds(second_frame, second_radius, second_height)?;
    Ok((0..3).any(|index| {
        first[index].1 + tolerance < second[index].0 || second[index].1 + tolerance < first[index].0
    }))
}

fn combine_axes(first: Vector3, second: Vector3, sign: Real) -> Result<Vector3, GeometryError> {
    let a = first.to_array();
    let b = second.to_array();
    Vector3::try_new(a[0] + sign * b[0], a[1] + sign * b[1], a[2] + sign * b[2])
}

fn cosine_limits(center: Real, slope: Real, height: Real) -> (Real, Real) {
    if slope > 0.0 {
        (-center / slope, (height - center) / slope)
    } else {
        ((height - center) / slope, -center / slope)
    }
}

fn ellipse_point(
    center: Point3,
    cosine_axis: Vector3,
    sine_axis: Vector3,
    angle: Real,
    scale: Real,
) -> Result<Point3, GeometryError> {
    let (sine, cosine) = angle.sin_cos();
    center
        .translated(cosine_axis.scaled(scale * cosine)?)?
        .translated(sine_axis.scaled(scale * sine)?)
}

fn ellipse_arc(
    center: Point3,
    cosine_axis: Vector3,
    sine_axis: Vector3,
    start: Real,
    end: Real,
) -> Result<NurbsCurve, GeometryError> {
    let quarter = std::f64::consts::FRAC_PI_2;
    let mut cuts = vec![start, end];
    for index in -4..=8 {
        let cut = index as Real * quarter;
        if start < cut && cut < end {
            cuts.push(cut);
        }
    }
    cuts.sort_by(Real::total_cmp);
    let mut controls = vec![WeightedPoint3::try_new(
        ellipse_point(center, cosine_axis, sine_axis, start, 1.0)?,
        1.0,
    )?];
    let mut knots = vec![start; 3];
    for pair in cuts.windows(2) {
        let middle = 0.5 * (pair[0] + pair[1]);
        let weight = (0.5 * (pair[1] - pair[0])).cos();
        controls.push(WeightedPoint3::try_new(
            ellipse_point(center, cosine_axis, sine_axis, middle, 1.0 / weight)?,
            weight,
        )?);
        controls.push(WeightedPoint3::try_new(
            ellipse_point(center, cosine_axis, sine_axis, pair[1], 1.0)?,
            1.0,
        )?);
        let multiplicity = if pair[1] == end { 3 } else { 2 };
        knots.extend(std::iter::repeat_n(pair[1], multiplicity));
    }
    if (end - start - std::f64::consts::TAU).abs() <= 16.0 * Real::EPSILON {
        *controls.last_mut().expect("arc has at least one span") = controls[0];
    }
    NurbsCurve::try_new_rational(2, controls, knots)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{NurbsSurface, surface_surface_intersection_events};

    fn point(x: Real, y: Real, z: Real) -> Point3 {
        Point3::try_new(x, y, z).unwrap()
    }

    fn cylinder(origin: Point3, axis: Vector3, height: Real) -> (NurbsSurface, Frame3) {
        let frame = Frame3::try_from_normal(origin, axis, Tolerance::DEFAULT).unwrap();
        (
            NurbsSurface::try_cylinder(frame, 1.0, 0.0, height).unwrap(),
            frame,
        )
    }

    fn crossed_cylinders(
        first_start: Real,
        first_height: Real,
    ) -> ((NurbsSurface, Frame3), (NurbsSurface, Frame3)) {
        let first = cylinder(
            point(0.0, 0.0, first_start),
            Vector3::try_new(0.0, 0.0, 1.0).unwrap(),
            first_height,
        );
        let second = cylinder(
            point(-2.0, 0.0, 0.0),
            Vector3::try_new(1.0, 0.0, 0.0).unwrap(),
            4.0,
        );
        (first, second)
    }

    fn assert_on_both_walls(location: Point3, first: Frame3, second: Frame3) {
        for frame in [first, second] {
            let local = frame.coordinates_of(location).unwrap();
            assert!((local[0].hypot(local[1]) - 1.0).abs() < 2e-12);
        }
    }

    #[test]
    fn crossed_equal_cylinders_make_two_exact_closed_ellipses() {
        let ((first, first_frame), (second, second_frame)) = crossed_cylinders(-2.0, 4.0);
        for (left, right) in [(&first, &second), (&second, &first)] {
            let events =
                surface_surface_intersection_events(left, right, Tolerance::DEFAULT).unwrap();
            assert_eq!(events.len(), 2);
            for event in events {
                let SurfaceSurfaceIntersectionEvent::Curve(curve) = event else {
                    panic!("crossed walls must meet in ellipses")
                };
                assert_eq!(curve.degree(), 2);
                assert!(curve.is_rational());
                assert!(curve.is_closed().unwrap());
                let domain = curve.domain();
                for index in 0..=64 {
                    let parameter = *domain.start()
                        + (*domain.end() - *domain.start()) * (index as Real / 64.0);
                    assert_on_both_walls(
                        curve.evaluate(parameter).unwrap(),
                        first_frame,
                        second_frame,
                    );
                }
            }
        }
    }

    #[test]
    fn crossed_cylinders_clip_both_ellipses_at_first_rim() {
        let ((first, first_frame), (second, second_frame)) = crossed_cylinders(0.0, 2.0);
        let events =
            surface_surface_intersection_events(&first, &second, Tolerance::DEFAULT).unwrap();
        assert_eq!(events.len(), 2);
        for event in events {
            let SurfaceSurfaceIntersectionEvent::Curve(curve) = event else {
                panic!("finite walls must meet in ellipse arcs")
            };
            assert_eq!(curve.degree(), 2);
            assert!(!curve.is_closed().unwrap());
            let domain = curve.domain();
            for index in 0..=64 {
                let parameter =
                    *domain.start() + (*domain.end() - *domain.start()) * (index as Real / 64.0);
                let location = curve.evaluate(parameter).unwrap();
                assert_on_both_walls(location, first_frame, second_frame);
                assert!((-1e-12..=2.0 + 1e-12).contains(&location.z()));
            }
            for parameter in [*domain.start(), *domain.end()] {
                assert!(curve.evaluate(parameter).unwrap().z().abs() < 1e-12);
            }
        }
    }

    #[test]
    fn crossed_cylinders_keep_isolated_rim_points() {
        let ((first, _), (second, _)) = crossed_cylinders(1.0, 1.0);
        let events =
            surface_surface_intersection_events(&first, &second, Tolerance::DEFAULT).unwrap();
        assert_eq!(events.len(), 2);
        let mut xs = Vec::new();
        for event in events {
            let SurfaceSurfaceIntersectionEvent::Point(location) = event else {
                panic!("rim tangency must be an isolated point")
            };
            assert!((location.z() - 1.0).abs() < 1e-12);
            assert!(location.y().abs() < 1e-12);
            xs.push(location.x());
        }
        xs.sort_by(Real::total_cmp);
        assert!((xs[0] + 1.0).abs() < 1e-12);
        assert!((xs[1] - 1.0).abs() < 1e-12);
    }

    #[test]
    fn crossed_cylinders_clip_at_second_rim() {
        let ((first, first_frame), _) = crossed_cylinders(-2.0, 4.0);
        let (second, second_frame) = cylinder(
            point(0.0, 0.0, 0.0),
            Vector3::try_new(1.0, 0.0, 0.0).unwrap(),
            2.0,
        );
        let events =
            surface_surface_intersection_events(&first, &second, Tolerance::DEFAULT).unwrap();
        assert_eq!(events.len(), 2);
        for event in events {
            let SurfaceSurfaceIntersectionEvent::Curve(curve) = event else {
                panic!("second rim must clip both conics")
            };
            assert!(!curve.is_closed().unwrap());
            let domain = curve.domain();
            for index in 0..=32 {
                let parameter =
                    *domain.start() + (*domain.end() - *domain.start()) * (index as Real / 32.0);
                let location = curve.evaluate(parameter).unwrap();
                assert_on_both_walls(location, first_frame, second_frame);
                assert!((-1e-12..=2.0 + 1e-12).contains(&location.x()));
            }
            for parameter in [*domain.start(), *domain.end()] {
                assert!(curve.evaluate(parameter).unwrap().x().abs() < 1e-12);
            }
        }
    }

    #[test]
    fn crossed_cylinders_work_in_rotated_frames_far_from_origin() {
        let crossing = point(1.0e8, -1.0e8, 1.0e8);
        let base = Frame3::try_from_normal(
            crossing,
            Vector3::try_new(1.0, 2.0, 3.0).unwrap(),
            Tolerance::DEFAULT,
        )
        .unwrap();
        let first_origin = crossing
            .translated(base.z_axis().as_vector().scaled(-2.0).unwrap())
            .unwrap();
        let second_origin = crossing
            .translated(base.x_axis().as_vector().scaled(-2.0).unwrap())
            .unwrap();
        let (first, first_frame) = cylinder(first_origin, base.z_axis().as_vector(), 4.0);
        let (second, second_frame) = cylinder(second_origin, base.x_axis().as_vector(), 4.0);
        let events =
            surface_surface_intersection_events(&first, &second, Tolerance::DEFAULT).unwrap();
        assert_eq!(events.len(), 2);
        for event in events {
            let SurfaceSurfaceIntersectionEvent::Curve(curve) = event else {
                panic!("rotated crossed walls must meet in ellipses")
            };
            assert!(curve.is_closed().unwrap());
            let domain = curve.domain();
            for index in 0..=32 {
                let parameter =
                    *domain.start() + (*domain.end() - *domain.start()) * (index as Real / 32.0);
                let location = curve.evaluate(parameter).unwrap();
                for frame in [first_frame, second_frame] {
                    let local = frame.coordinates_of(location).unwrap();
                    assert!((local[0].hypot(local[1]) - 1.0).abs() < 3e-7);
                }
            }
        }
    }

    fn oblique_pair(
        angle: Real,
        first_start: Real,
        first_height: Real,
        second_start: Real,
        second_height: Real,
    ) -> ((NurbsSurface, Frame3), (NurbsSurface, Frame3)) {
        let (sine, cosine) = angle.sin_cos();
        let first = cylinder(
            point(0.0, 0.0, first_start),
            Vector3::try_new(0.0, 0.0, 1.0).unwrap(),
            first_height,
        );
        let second = cylinder(
            point(second_start * sine, 0.0, second_start * cosine),
            Vector3::try_new(sine, 0.0, cosine).unwrap(),
            second_height,
        );
        (first, second)
    }

    #[test]
    fn oblique_crossed_cylinders_have_two_exact_ellipses() {
        for angle in [
            std::f64::consts::FRAC_PI_3,
            2.0 * std::f64::consts::FRAC_PI_3,
        ] {
            let ((first, first_frame), (second, second_frame)) =
                oblique_pair(angle, -3.0, 6.0, -3.0, 6.0);
            for (left, right) in [(&first, &second), (&second, &first)] {
                let events =
                    surface_surface_intersection_events(left, right, Tolerance::DEFAULT).unwrap();
                assert_eq!(events.len(), 2);
                let mut major_radii = Vec::new();
                for event in events {
                    let SurfaceSurfaceIntersectionEvent::Curve(curve) = event else {
                        panic!("oblique crossed walls must meet in ellipses")
                    };
                    assert_eq!(curve.degree(), 2);
                    assert!(curve.is_rational());
                    assert!(curve.is_closed().unwrap());
                    major_radii.push(
                        curve
                            .evaluate(*curve.domain().start())
                            .unwrap()
                            .distance_to(point(0.0, 0.0, 0.0))
                            .unwrap(),
                    );
                    let domain = curve.domain();
                    for index in 0..=64 {
                        let parameter = *domain.start()
                            + (*domain.end() - *domain.start()) * (index as Real / 64.0);
                        assert_on_both_walls(
                            curve.evaluate(parameter).unwrap(),
                            first_frame,
                            second_frame,
                        );
                    }
                }
                major_radii.sort_by(Real::total_cmp);
                assert!((major_radii[0] - 2.0 / 3.0_f64.sqrt()).abs() < 1e-12);
                assert!((major_radii[1] - 2.0).abs() < 1e-12);
            }
        }
    }

    #[test]
    fn oblique_crossed_cylinders_clip_at_both_finite_heights() {
        let angle = std::f64::consts::FRAC_PI_3;
        for (first_start, first_height, second_start, second_height) in
            [(0.0, 3.0, -3.0, 6.0), (-3.0, 6.0, 0.0, 3.0)]
        {
            let ((first, first_frame), (second, second_frame)) = oblique_pair(
                angle,
                first_start,
                first_height,
                second_start,
                second_height,
            );
            let events =
                surface_surface_intersection_events(&first, &second, Tolerance::DEFAULT).unwrap();
            assert_eq!(events.len(), 2);
            for event in events {
                let SurfaceSurfaceIntersectionEvent::Curve(curve) = event else {
                    panic!("finite oblique walls must meet in arcs")
                };
                assert!(!curve.is_closed().unwrap());
                let domain = curve.domain();
                for index in 0..=32 {
                    let parameter = *domain.start()
                        + (*domain.end() - *domain.start()) * (index as Real / 32.0);
                    let location = curve.evaluate(parameter).unwrap();
                    assert_on_both_walls(location, first_frame, second_frame);
                    let first_z = first_frame.coordinates_of(location).unwrap()[2];
                    let second_z = second_frame.coordinates_of(location).unwrap()[2];
                    assert!((-1e-12..=first_height + 1e-12).contains(&first_z));
                    assert!((-1e-12..=second_height + 1e-12).contains(&second_z));
                }
            }
        }
    }

    #[test]
    fn oblique_crossed_cylinders_split_each_ellipse_into_two_arcs() {
        let ((first, first_frame), (second, second_frame)) =
            oblique_pair(std::f64::consts::FRAC_PI_3, -0.5, 1.0, -0.5, 1.0);
        let events =
            surface_surface_intersection_events(&first, &second, Tolerance::DEFAULT).unwrap();
        assert_eq!(events.len(), 4);
        for event in events {
            let SurfaceSurfaceIntersectionEvent::Curve(curve) = event else {
                panic!("both finite walls must leave four conic arcs")
            };
            assert!(!curve.is_closed().unwrap());
            let domain = curve.domain();
            for index in 0..=32 {
                let parameter =
                    *domain.start() + (*domain.end() - *domain.start()) * (index as Real / 32.0);
                let location = curve.evaluate(parameter).unwrap();
                assert_on_both_walls(location, first_frame, second_frame);
                for frame in [first_frame, second_frame] {
                    let axial = frame.coordinates_of(location).unwrap()[2];
                    assert!((-1e-12..=1.0 + 1e-12).contains(&axial));
                }
            }
        }
    }
}
