//! Adaptive cubic offsets of analytic ellipses in their principal-axis frame.

use std::f64::consts::{FRAC_PI_2, TAU};

use crate::{Curve3, Ellipse3, GeometryError, NurbsCurve, Point3, Real, Tolerance, require_finite};

const MAX_ELLIPSE_OFFSET_SPANS: usize = 8_192;
const CHECKS_PER_SPAN: usize = 15;

#[derive(Clone, Copy)]
struct Sample {
    angle: Real,
    point: [Real; 2],
    tangent: [Real; 2],
}

pub(super) fn offset_ellipse(
    ellipse: Ellipse3,
    distance: Real,
    tolerance: Tolerance,
) -> Result<Curve3, GeometryError> {
    let (a, b) = (ellipse.radius_x(), ellipse.radius_y());
    let (minor, major) = (a.min(b), a.max(b));
    let minimum_curvature_radius = minor * (minor / major);
    if distance > 0.0 && distance >= minimum_curvature_radius {
        return Err(GeometryError::Degenerate {
            context: "inward ellipse offset cusp",
        });
    }
    if a == b {
        let radius = a - distance;
        return Ok(Curve3::Ellipse(
            Ellipse3::try_new(
                ellipse.center(),
                radius,
                radius,
                ellipse.x_axis(),
                ellipse.y_axis(),
                tolerance,
            )?
            .try_reparameterized(ellipse.domain())?,
        ));
    }
    let first = ellipse_sample(ellipse, distance, 0.0)?;
    let mut boundaries = Vec::with_capacity(5);
    boundaries.push(first);
    for quadrant in 1..4 {
        boundaries.push(ellipse_sample(
            ellipse,
            distance,
            quadrant as Real * FRAC_PI_2,
        )?);
    }
    boundaries.push(Sample {
        angle: TAU,
        ..first
    });
    let mut pending = (0..4)
        .rev()
        .map(|index| (boundaries[index], boundaries[index + 1]))
        .collect::<Vec<_>>();
    let mut accepted = Vec::new();
    let target = tolerance.absolute() * 0.25;
    while let Some((start, end)) = pending.pop() {
        let controls = cubic_controls(start, end)?;
        let mut largest = 0.0_f64;
        for check in 1..=CHECKS_PER_SPAN {
            let fraction = check as Real / (CHECKS_PER_SPAN + 1) as Real;
            let angle = start.angle.mul_add(1.0 - fraction, end.angle * fraction);
            let expected = ellipse_sample(ellipse, distance, angle)?.point;
            let actual = cubic_point(controls, fraction);
            largest = largest.max((expected[0] - actual[0]).hypot(expected[1] - actual[1]));
        }
        if largest <= target {
            accepted.push((start, end, controls));
            continue;
        }
        if accepted.len() + pending.len() >= MAX_ELLIPSE_OFFSET_SPANS {
            return Err(GeometryError::EllipseOffsetFitLimit);
        }
        let middle_angle = start.angle.midpoint(end.angle);
        if middle_angle <= start.angle || middle_angle >= end.angle {
            return Err(GeometryError::EllipseOffsetFitLimit);
        }
        let middle = ellipse_sample(ellipse, distance, middle_angle)?;
        pending.push((middle, end));
        pending.push((start, middle));
    }
    let mut points = Vec::with_capacity(3 * accepted.len() + 1);
    let mut knots = vec![0.0; 4];
    points.push(world_point(ellipse, first.point)?);
    for (index, (start, end, controls)) in accepted.iter().enumerate() {
        points.push(world_point(ellipse, controls[1])?);
        points.push(world_point(ellipse, controls[2])?);
        points.push(if index + 1 == accepted.len() {
            points[0]
        } else {
            world_point(ellipse, controls[3])?
        });
        knots.extend(std::iter::repeat_n(
            end.angle,
            if index + 1 == accepted.len() { 4 } else { 3 },
        ));
        debug_assert!(start.angle < end.angle);
    }
    let curve = NurbsCurve::try_new(3, points, knots)?;
    // Check the actual world-space spline, including construction and knot
    // rounding, independently of the local-frame acceptance test.
    for (start, end, _) in &accepted {
        for check in 1..=CHECKS_PER_SPAN {
            let fraction = check as Real / (CHECKS_PER_SPAN + 1) as Real;
            let angle = start.angle.mul_add(1.0 - fraction, end.angle * fraction);
            let expected = world_point(ellipse, ellipse_sample(ellipse, distance, angle)?.point)?;
            if curve.evaluate(angle)?.distance_to(expected)? > tolerance.absolute() {
                return Err(GeometryError::EllipseOffsetFitLimit);
            }
        }
    }
    Ok(Curve3::NurbsCurve(
        curve.try_reparameterized(ellipse.domain())?,
    ))
}

pub(super) fn ellipse_offset_side(
    ellipse: Ellipse3,
    side: Point3,
    tolerance: Tolerance,
) -> Result<Real, GeometryError> {
    let [x, y] = ellipse_coordinates(ellipse, side)?;
    let normalized_radius = (x / ellipse.radius_x()).hypot(y / ellipse.radius_y());
    if (normalized_radius - 1.0).abs() <= 0.5 {
        let nearest = ellipse.evaluate(ellipse.closest_parameter(side)?)?;
        let [nx, ny] = ellipse_coordinates(ellipse, nearest)?;
        if (x - nx).hypot(y - ny) <= tolerance.absolute() {
            return Err(GeometryError::AmbiguousCurveOffsetSide);
        }
    }
    Ok(if normalized_radius < 1.0 { 1.0 } else { -1.0 })
}

pub(super) fn ellipse_region_contains(
    ellipse: Ellipse3,
    point: Point3,
    tolerance: Tolerance,
) -> Result<bool, GeometryError> {
    if ellipse
        .center()
        .vector_to(point)?
        .dot(ellipse.normal()?.as_vector())?
        .abs()
        > tolerance.absolute()
    {
        return Ok(false);
    }
    Ok(ellipse_offset_side(ellipse, point, tolerance)? > 0.0)
}

pub(super) fn ellipse_through_distance(
    ellipse: Ellipse3,
    point: Point3,
    tolerance: Tolerance,
) -> Result<Real, GeometryError> {
    let sign = ellipse_offset_side(ellipse, point, tolerance)?;
    let nearest = ellipse.evaluate(ellipse.closest_parameter(point)?)?;
    let [x, y] = ellipse_coordinates(ellipse, point)?;
    let [nx, ny] = ellipse_coordinates(ellipse, nearest)?;
    let distance = (x - nx).hypot(y - ny);
    require_finite([distance], "ellipse offset through-point distance")?;
    Ok(sign * distance)
}

fn ellipse_coordinates(ellipse: Ellipse3, point: Point3) -> Result<[Real; 2], GeometryError> {
    let x = ellipse
        .x_axis()
        .as_vector()
        .dot_point_difference(point, ellipse.center());
    let y = ellipse
        .y_axis()
        .as_vector()
        .dot_point_difference(point, ellipse.center());
    require_finite([x, y], "ellipse offset coordinates")?;
    Ok([x, y])
}

fn ellipse_sample(ellipse: Ellipse3, distance: Real, angle: Real) -> Result<Sample, GeometryError> {
    let (sine, cosine) = angle.sin_cos();
    let (a, b) = (ellipse.radius_x(), ellipse.radius_y());
    let speed = (a * sine).hypot(b * cosine);
    let curvature = (a / speed) * (b / speed) / speed;
    let factor = 1.0 - distance * curvature;
    let point = [
        cosine * (a - distance * b / speed),
        sine * (b - distance * a / speed),
    ];
    let tangent = [-a * sine * factor, b * cosine * factor];
    require_finite(
        [
            speed, curvature, factor, point[0], point[1], tangent[0], tangent[1],
        ],
        "ellipse offset sample",
    )?;
    Ok(Sample {
        angle,
        point,
        tangent,
    })
}

fn cubic_controls(start: Sample, end: Sample) -> Result<[[Real; 2]; 4], GeometryError> {
    let handle = (end.angle - start.angle) / 3.0;
    let controls = [
        start.point,
        [
            start.point[0] + handle * start.tangent[0],
            start.point[1] + handle * start.tangent[1],
        ],
        [
            end.point[0] - handle * end.tangent[0],
            end.point[1] - handle * end.tangent[1],
        ],
        end.point,
    ];
    require_finite(
        controls.into_iter().flatten(),
        "ellipse offset cubic controls",
    )?;
    Ok(controls)
}

fn cubic_point(mut controls: [[Real; 2]; 4], fraction: Real) -> [Real; 2] {
    for depth in (1..4).rev() {
        for index in 0..depth {
            for axis in 0..2 {
                controls[index][axis] = controls[index][axis]
                    .mul_add(1.0 - fraction, controls[index + 1][axis] * fraction);
            }
        }
    }
    controls[0]
}

fn world_point(ellipse: Ellipse3, local: [Real; 2]) -> Result<Point3, GeometryError> {
    let center = ellipse.center().to_array();
    let x = ellipse.x_axis().as_vector().to_array();
    let y = ellipse.y_axis().as_vector().to_array();
    Point3::try_from(std::array::from_fn(|axis| {
        x[axis].mul_add(local[0], y[axis].mul_add(local[1], center[axis]))
    }))
}
