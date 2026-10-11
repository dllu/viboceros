//! Cached two-object choices using only copied original kernel operands.
use super::*;
use viboceros_smlib::{BooleanOperation, Solid};
#[cfg(test)]
mod tests;

fn pieces(solid: &Solid, tolerance: Tolerance) -> Result<Vec<Piece>, CommandError> {
    let parts = boolean_solids::native::parts(solid, tolerance)?;
    let retain_geometry_user_text = parts.len() == 1;
    Ok(parts
        .into_iter()
        .map(|brep| Piece {
            brep,
            owner: 0,
            retain_geometry_user_text,
        })
        .collect())
}

pub(super) fn prepare(
    first: &Brep,
    second: &Brep,
    tolerance: Tolerance,
) -> Result<Candidates, CommandError> {
    let inputs = boolean_solids::native::imports(&[first, second], tolerance)?;
    let (a, b) = (&inputs[0], &inputs[1]);
    if !a.boundary_contact(b, tolerance)? {
        return Err(GeometryError::UnsupportedPolyhedralBrepBoolean {
            context: "two boundary-crossing operands required",
        }
        .into());
    }
    let common = a.boolean_with_postprocess(b, BooleanOperation::Intersection, false)?;
    if common.is_empty()? {
        return Err(GeometryError::UnsupportedPolyhedralBrepBoolean {
            context: "positive shared material required",
        }
        .into());
    }
    let ab = a.boolean_with_postprocess(b, BooleanOperation::Difference, false)?;
    let ba = b.boolean_with_postprocess(a, BooleanOperation::Difference, false)?;
    if ab.is_empty()? && ba.is_empty()? {
        return Err(GeometryError::UnsupportedPolyhedralBrepBoolean {
            context: "distinct boundary-crossing operands required",
        }
        .into());
    }
    let union = a.boolean_with_postprocess(b, BooleanOperation::Union, false)?;
    let ab = pieces(&ab, tolerance)?;
    let ba = pieces(&ba, tolerance)?;
    let inverse = ba.iter().cloned().chain(ab.iter().cloned()).collect();
    Ok(Candidates {
        values: [
            pieces(&union, tolerance)?,
            pieces(&common, tolerance)?,
            ab,
            ba,
            inverse,
        ]
        .map(Arc::new),
    })
}
