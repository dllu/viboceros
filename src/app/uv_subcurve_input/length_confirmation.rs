//! Numeric getter orientation and open-end clamping, separate from length math.
use viboceros_geometry::{CurveRef, GeometryError, Real, Tolerance};

pub(super) fn interval(
    curve: CurveRef<'_>,
    anchor: Real,
    confirmation: Real,
    length: Real,
    tolerance: Tolerance,
) -> Result<Option<[Real; 2]>, GeometryError> {
    let domain = curve.domain();
    let closed = curve.is_closed()?;
    if closed && length == curve.length(tolerance)? {
        return Ok(None);
    }
    let forward = if closed {
        confirmation != anchor
            && curve
                .to_owned()
                .try_subcurve(anchor, confirmation)?
                .as_ref()
                .length(tolerance)?
                <= curve.length(tolerance)? * 0.5
    } else {
        confirmation > anchor
    };
    let signed = if forward { length } else { -length };
    let result = curve
        .to_owned()
        .try_subcurve_at_arc_length_with_endpoint(anchor, signed, tolerance)?;
    let endpoint = if let Some((_, endpoint)) = result {
        endpoint
    } else if forward {
        *domain.end()
    } else {
        *domain.start()
    };
    Ok(Some(if forward {
        [anchor, endpoint]
    } else {
        [endpoint, anchor]
    }))
}
