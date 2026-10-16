//! Extend projected windows to corresponding natural transverse endpoints.
use crate::{CurveRef, GeometryError, Tolerance, Vector3};
#[cfg(test)]
mod tests;

/// Preserve corresponding natural ends in a projected overlap window when
/// their displacement is perpendicular to the aligned endpoint chords within
/// the supplied absolute tolerance. This supplies boundary correspondences for
/// translated profiles without clipping away their ends merely because a
/// closest point lies farther along the other profile.
///
/// This is a geometric endpoint policy, not Rhino's complete overlap finder.
/// Closed curves and degenerate/ambiguous chord frames retain the input window.
/// Inputs and output are fractions in each source's unedited NURBS chart.
pub fn preserve_transverse_curve_ends(
    first: CurveRef<'_>,
    second: CurveRef<'_>,
    mut intervals: [[f64; 2]; 2],
    tolerance: Tolerance,
) -> Result<[[f64; 2]; 2], GeometryError> {
    for range in intervals {
        if range
            .iter()
            .any(|t| !t.is_finite() || !(0. ..=1.).contains(t))
            || range[0] > range[1]
        {
            return Err(GeometryError::InvalidCurveTrimInterval);
        }
    }
    if first.is_closed()? || second.is_closed()? {
        return Ok(intervals);
    }
    let ends = [
        [first.start_point()?, first.end_point()?],
        [second.start_point()?, second.end_point()?],
    ];
    let reverse = !first.directions_match(second)?;
    let a = ends[0][0].vector_to(ends[0][1])?;
    let b = ends[1][usize::from(reverse)].vector_to(ends[1][usize::from(!reverse)])?;
    if a.length()? == 0. || b.length()? == 0. {
        return Ok(intervals);
    }
    let a = a.normalized_nonzero()?.as_vector().to_array();
    let b = b.normalized_nonzero()?.as_vector().to_array();
    let axis = Vector3::try_from(std::array::from_fn(|i| a[i] + b[i]))?;
    if axis.length()? <= 64. * f64::EPSILON {
        return Ok(intervals);
    }
    let axis = axis.normalized_nonzero()?.as_vector();
    for i in 0..2 {
        let j = if reverse { 1 - i } else { i };
        if axis.dot_point_difference(ends[0][i], ends[1][j]).abs() <= tolerance.absolute() {
            intervals[0][i] = i as f64;
            intervals[1][j] = j as f64;
        }
    }
    Ok(intervals)
}
