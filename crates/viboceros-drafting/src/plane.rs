//! Plane-local precision drafting with screen-space capture distances.
use super::{DraftingError, OrthogonalTrack, TrackAxis, validate_capture_radius};
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
    let frame = plane.with_origin(anchor);
    let [x, y, _] = frame.coordinates_of(cursor)?;
    let mut best = None;
    let mut best_distance = capture_radius;
    // Capture the anchor first; otherwise the closer local axis wins (X on ties).
    for (point, axis) in [
        (anchor, TrackAxis::Both),
        (frame.point_at([x, 0., 0.])?, TrackAxis::Horizontal),
        (frame.point_at([0., y, 0.])?, TrackAxis::Vertical),
    ] {
        if let Some(screen) = project(point) {
            let distance = (screen[0] - pointer[0]).hypot(screen[1] - pointer[1]);
            if distance.is_finite()
                && distance <= capture_radius
                && (best.is_none() || distance < best_distance)
            {
                best = Some(OrthogonalTrack { point, axis });
                best_distance = distance;
                if axis == TrackAxis::Both {
                    return Ok(best);
                }
            }
        }
    }
    Ok(best)
}

#[cfg(test)]
mod tests;
