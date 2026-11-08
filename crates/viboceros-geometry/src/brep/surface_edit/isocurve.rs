//! Exact images of affine axis-aligned trim segments retain rational weights.
use super::*;

pub(super) fn image(
    trim: &BrepTrim,
    surface: &NurbsSurface,
) -> Result<Option<NurbsCurve>, GeometryError> {
    let controls = trim.curve.control_points();
    if trim.curve.degree() != 1
        || controls.len() != 2
        || controls[0].weight() != controls[1].weight()
    {
        return Ok(None);
    }
    let a = controls[0].point();
    let b = controls[1].point();
    let (curve, start, end) = if a.y() == b.y() && a.x() != b.x() {
        (surface.isocurve_u(a.y())?, a.x(), b.x())
    } else if a.x() == b.x() && a.y() != b.y() {
        (surface.isocurve_v(a.x())?, a.y(), b.y())
    } else {
        return Ok(None);
    };
    let curve = curve.try_trimmed(start.min(end)..=start.max(end))?;
    let curve = if start > end {
        curve.reversed()?
    } else {
        curve
    };
    Ok(Some(curve.try_reparameterized(trim.curve.domain())?))
}
