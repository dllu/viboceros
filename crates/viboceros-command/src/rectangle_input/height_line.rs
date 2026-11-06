//! Point constraints use the actual finite endpoint representation of the line.
use super::*;
#[path = "../../../../third_party/opennurbs_rust/line_projection.rs"]
mod line_projection;

/// Vertical Plane's height getter projects onto the unbounded line from the
/// second edge point to that point plus the rectangle's unit Y direction.
/// At remote coordinates the rounded endpoint matters. Keep this command input
/// policy separate from the geometry kernel's ideal directional projections.
/// Endpoint choice and evaluation use the attributed OpenNURBS Rust adaptation.
pub(super) fn height(second: Point3, point: Point3, frame: Frame3) -> Result<Real, GeometryError> {
    let end = second.translated(frame.y_axis().as_vector())?;
    let (projected, t) = line_projection::closest_point(second, end, point)?;
    Ok(second.distance_to(projected)?.copysign(t))
}
