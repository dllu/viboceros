//! Geometry shared by Polygon and the Pyramid base getters.
use std::f64::consts::{PI, TAU};
use viboceros_geometry::{Frame3, GeometryError, Point3, Tolerance, Vector3};

pub(crate) fn radial(
    frame: Frame3,
    size: f64,
    sides: usize,
    circumscribed: bool,
    tolerance: Tolerance,
) -> Result<(Frame3, f64), GeometryError> {
    if circumscribed {
        let angle = PI / sides as f64;
        Ok((rotated_frame(frame, angle, tolerance)?, size / angle.cos()))
    } else {
        Ok((frame, size))
    }
}

pub(crate) fn edge(
    edge: Frame3,
    end: Point3,
    length: f64,
    sides: usize,
    flipped: bool,
    tolerance: Tolerance,
) -> Result<(Frame3, f64), GeometryError> {
    let half = length * 0.5;
    let apothem = half / (PI / sides as f64).tan();
    let center = edge.point_at([half, if flipped { -apothem } else { apothem }, 0.])?;
    let normal = if flipped {
        edge.z_axis().opposite()
    } else {
        edge.z_axis()
    };
    let frame =
        Frame3::try_from_unit_x_and_normal(center, center.direction_to(end)?, normal, tolerance)?;
    Ok((frame, half.hypot(apothem)))
}

pub(crate) fn automatic_inner_radius(radius: f64, sides: usize) -> f64 {
    let step = PI / sides as f64;
    let ratio = if sides < 5 {
        0.25
    } else {
        (2. * step).cos() / step.cos()
    };
    radius * ratio
}

pub(crate) fn sampled(
    frame: Frame3,
    radius: f64,
    inner_radius: Option<f64>,
    sides: usize,
    limit: usize,
) -> Result<Vec<Point3>, GeometryError> {
    let samples = sides.min(limit.max(3));
    let step = TAU / sides as f64;
    let mut points = Vec::with_capacity(samples * (1 + usize::from(inner_radius.is_some())));
    let at = |radius: f64, angle: f64| {
        let (sin, cos) = angle.sin_cos();
        frame.point_at([radius * cos, radius * sin, 0.0])
    };
    for sample in 0..samples {
        let angle = (sample * sides / samples) as f64 * step;
        points.push(at(radius, angle)?);
        if let Some(inner) = inner_radius {
            points.push(at(inner, angle + 0.5 * step)?);
        }
    }
    Ok(points)
}

pub(crate) fn rotated_frame(
    frame: Frame3,
    angle: f64,
    tolerance: Tolerance,
) -> Result<Frame3, GeometryError> {
    if angle == 0. {
        return Ok(frame);
    }
    let (sin, cos) = angle.sin_cos();
    let x = frame.x_axis().as_vector().to_array();
    let y = frame.y_axis().as_vector().to_array();
    let direction = Vector3::try_from(std::array::from_fn(|i| cos.mul_add(x[i], sin * y[i])))?
        .normalized_nonzero()?;
    Frame3::try_from_unit_x_and_normal(frame.origin(), direction, frame.z_axis(), tolerance)
}
