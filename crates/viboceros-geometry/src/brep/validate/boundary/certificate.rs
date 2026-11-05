//! Whole-image fallback when composing UV parameters loses geometric resolution.
use super::*;
use crate::brep::trim_image::isocurve::BoundaryImage;

pub(super) fn within(
    face: &BrepFace,
    trim: &BrepTrim,
    edge: &BrepEdge,
    allowed: Real,
) -> Result<bool, GeometryError> {
    // A failed sampled search is not a proof of a gap. Reversed trims and very
    // short surface spans can have no sufficiently close floating-point UV
    // sample, even when the exact rational images coincide. Certify the entire
    // oriented image; retain the ordinary rejection if it cannot be proved.
    let mut remaining = 16_000_000usize;
    let mut charge = |n| {
        remaining = remaining
            .checked_sub(n)
            .ok_or(GeometryError::InvalidBrepTopology {
                context: "boundary certificate work budget exceeded",
            })?;
        Ok(())
    };
    let Some(image) = BoundaryImage::new(&face.surface, trim, &mut charge)? else {
        return Ok(false);
    };
    Ok(image
        .bound(&edge.curve, trim.reversed_3d, true, &mut charge)?
        .is_some_and(|bound| bound <= allowed))
}
