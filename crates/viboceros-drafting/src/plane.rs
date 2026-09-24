//! Plane-local precision drafting with screen-space capture distances.
use super::{
    DraftingError, OrthogonalTrack, TrackAxis, validate_capture_radius, validate_cursor_coordinates,
};
use viboceros_geometry::{Frame3, GeometryError, Point3, Vector3};

/// Intersect a camera line/ray with an arbitrary drafting plane. Edge-on planes
/// and rays directed behind a perspective camera have no usable cursor point.
pub fn intersect_view_line(
    origin: Point3,
    direction: Vector3,
    plane: Frame3,
    forward_only: bool,
) -> Result<Option<Point3>, GeometryError> {
    let direction = direction.normalized_nonzero()?.as_vector();
    let denominator = direction.dot(plane.z_axis().as_vector())?;
    if denominator.abs() <= 32.0 * f64::EPSILON {
        return Ok(None);
    }
    let distance = -plane.coordinates_of(origin)?[2] / denominator;
    if !distance.is_finite() || (forward_only && distance < 0.0) {
        return Ok(None);
    }
    let o = origin.to_array();
    let d = direction.to_array();
    Ok(Some(Point3::try_from(std::array::from_fn(|i| {
        distance.mul_add(d[i], o[i])
    }))?))
}

/// Round only local X/Y; the point's construction-plane elevation is retained.
pub fn snap_to_grid(point: Point3, plane: Frame3, spacing: f64) -> Result<Point3, DraftingError> {
    validate_capture_radius(spacing)?;
    let [x, y, z] = plane.coordinates_of(point)?;
    let snap = |coordinate: f64| {
        // Avoid an overflowing grid index and a rounded quotient/product.
        // Compare both distances rather than spacing/2 (which can underflow).
        let remainder = coordinate % spacing;
        let distance = remainder.abs();
        let complement = spacing - distance;
        let adjustment = if distance >= complement {
            complement.copysign(coordinate)
        } else {
            -remainder
        };
        let value = coordinate + adjustment;
        if value == 0.0 { 0.0 } else { value }
    };
    Ok(plane.point_at([snap(x), snap(y), z])?)
}

/// Constrain a cursor to the nearest angular ray through the last picked point.
/// Directions are measured from the construction plane's local X axis.
pub fn ortho_point(
    cursor: Point3,
    anchor: Point3,
    plane: Frame3,
    angle_degrees: f64,
) -> Option<Point3> {
    if !(angle_degrees.is_finite() && 0.0 < angle_degrees && angle_degrees <= 180.0) {
        return None;
    }
    let frame = plane.with_origin(anchor);
    let [x, y, _] = frame.coordinates_of(cursor).ok()?;
    if x == 0.0 && y == 0.0 {
        return Some(anchor);
    }
    let step = angle_degrees.to_radians();
    // Below floating-point angular resolution, every representable direction
    // is already within one increment of the cursor direction.
    if step <= f64::EPSILON {
        return frame.point_at([x, y, 0.0]).ok();
    }
    let direction = (y.atan2(x) / step).round() * step;
    let unit = [direction.cos(), direction.sin()].map(|value| {
        if value.abs() <= 4.0 * f64::EPSILON {
            0.0
        } else {
            value
        }
    });
    // Form each projected component directly: the intermediate dot product
    // can overflow even when both resulting coordinates are representable.
    let projected_x = x.mul_add(unit[0] * unit[0], y * (unit[1] * unit[0]));
    let projected_y = y.mul_add(unit[1] * unit[1], x * (unit[0] * unit[1]));
    frame.point_at([projected_x, projected_y, 0.0]).ok()
}

/// Resolve the screen-space CPlane Z tracking line through the previous pick.
/// The projection callback returns screen X/Y and homogeneous camera depth;
/// parallel views use depth 1. The returned distance is in screen pixels.
pub fn ortho_z_projected(
    anchor: Point3,
    plane: Frame3,
    pointer: [f64; 2],
    project: impl Fn(Point3) -> Option<[f64; 3]>,
) -> Option<(Point3, f64)> {
    if !pointer.into_iter().all(f64::is_finite) {
        return None;
    }
    let start = project(anchor)?;
    if !start.into_iter().all(f64::is_finite) || start[2] <= 0.0 {
        return None;
    }
    let frame = plane.with_origin(anchor);
    for sign in [1.0, -1.0] {
        let Ok(sample) = frame.point_at([0.0, 0.0, sign]) else {
            continue;
        };
        let Some(end) = project(sample) else {
            continue;
        };
        if !end.into_iter().all(f64::is_finite) || end[2] <= 0.0 {
            continue;
        }
        let delta = [end[0] - start[0], end[1] - start[1]];
        let length_squared = delta[0].mul_add(delta[0], delta[1] * delta[1]);
        if !length_squared.is_finite() || length_squared <= 1e-20 {
            continue;
        }
        let screen_parameter = ((pointer[0] - start[0])
            .mul_add(delta[0], (pointer[1] - start[1]) * delta[1]))
            / length_squared;
        let axis = usize::from(delta[1].abs() > delta[0].abs());
        let nearest = start[axis] + screen_parameter * delta[axis];
        let depth_scale = start[2].max(end[2]);
        let d0 = start[2] / depth_scale;
        let d1 = end[2] / depth_scale;
        let numerator = (start[axis] - nearest) * d0;
        let denominator = (nearest - end[axis]).mul_add(d1, -(nearest - start[axis]) * d0);
        let parameter = numerator / denominator;
        if !parameter.is_finite() {
            continue;
        }
        let Ok(candidate) = frame.point_at([0.0, 0.0, sign * parameter]) else {
            continue;
        };
        if let Some(screen) = project(candidate) {
            let distance = (screen[0] - pointer[0]).hypot(screen[1] - pointer[1]);
            if distance.is_finite() {
                return Some((candidate, distance));
            }
        }
    }
    None
}

/// Track the plane's X/Y axes through an anchor. Candidate acceptance is measured
/// in the supplied viewport projection, not in world-XY or model-space units.
pub fn orthogonal_track_projected(
    cursor: Point3,
    anchor: Point3,
    plane: Frame3,
    pointer: [f64; 2],
    capture_radius: f64,
    project: impl Fn(Point3) -> Option<[f64; 2]>,
) -> Result<Option<OrthogonalTrack>, DraftingError> {
    validate_capture_radius(capture_radius)?;
    validate_cursor_coordinates(pointer)?;
    // Anchor capture is independent of the cursor's plane coordinates. Do
    // not construct possibly unrepresentable axis candidates before accepting it.
    if let Some(screen) = project(anchor) {
        let distance = (screen[0] - pointer[0]).hypot(screen[1] - pointer[1]);
        if distance.is_finite() && distance <= capture_radius {
            return Ok(Some(OrthogonalTrack {
                point: anchor,
                axis: TrackAxis::Both,
            }));
        }
    }
    let frame = plane.with_origin(anchor);
    let [x, y, _] = frame.coordinates_of(cursor)?;
    let mut best = None;
    let mut best_distance = capture_radius;
    // Otherwise the closer local axis wins (X on ties). One candidate can
    // overflow world coordinates even when the other remains representable.
    for (coordinates, axis) in [
        ([x, 0., 0.], TrackAxis::Horizontal),
        ([0., y, 0.], TrackAxis::Vertical),
    ] {
        let Ok(point) = frame.point_at(coordinates) else {
            continue;
        };
        if let Some(screen) = project(point) {
            let distance = (screen[0] - pointer[0]).hypot(screen[1] - pointer[1]);
            if distance.is_finite()
                && distance <= capture_radius
                && (best.is_none() || distance < best_distance)
            {
                best = Some(OrthogonalTrack { point, axis });
                best_distance = distance;
            }
        }
    }
    Ok(best)
}

#[cfg(test)]
mod tests;
