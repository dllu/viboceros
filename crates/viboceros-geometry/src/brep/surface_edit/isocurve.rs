//! Exact images of affine axis-aligned trim segments retain rational weights.
use super::*;
#[cfg(test)]
mod tests;

pub(super) fn image(
    trim: &BrepTrim,
    surface: &NurbsSurface,
) -> Result<Option<NurbsCurve>, GeometryError> {
    let controls = trim.curve.control_points();
    if trim.curve.degree() != 1 || controls.iter().any(|c| c.weight() != controls[0].weight()) {
        return Ok(None);
    }
    if controls.len() == 2 {
        return segment(
            surface,
            controls[0].point(),
            controls[1].point(),
            trim.curve.domain(),
        );
    }
    let mut segments = Vec::new();
    let mut parameters = Vec::new();
    let mut count = 0;
    for (i, pair) in controls.windows(2).enumerate() {
        let a = trim.curve.knots()[i + 1];
        let b = trim.curve.knots()[i + 2];
        if a == b {
            continue;
        }
        let Some(curve) = segment(surface, pair[0].point(), pair[1].point(), a..=b)? else {
            return Ok(None);
        };
        count += curve.control_points().len();
        if count > MAX_TRIM_IMAGE_CONTROLS {
            return Err(GeometryError::SurfaceCurveCertificateWorkLimit);
        }
        if parameters.is_empty() {
            parameters.push(a);
        }
        parameters.push(b);
        segments.push(curve);
    }
    Ok(Some(
        crate::PolyCurve3::try_with_segment_domains(segments, parameters)?.to_nurbs()?,
    ))
}

fn segment(
    surface: &NurbsSurface,
    a: crate::Point2,
    b: crate::Point2,
    domain: std::ops::RangeInclusive<Real>,
) -> Result<Option<NurbsCurve>, GeometryError> {
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
    Ok(Some(curve.try_reparameterized(domain)?))
}
