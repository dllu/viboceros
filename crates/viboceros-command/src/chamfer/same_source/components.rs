//! Flat subdivisions inside the native interval are one picked piece.
use super::*;
pub(super) fn groups(
    pieces: &[Curve3],
    closed: bool,
    tolerance: Tolerance,
) -> Result<Vec<usize>, CommandError> {
    let smooth = |a: &Curve3, b: &Curve3| -> Result<bool, CommandError> {
        let u = a
            .as_ref()
            .evaluate_with_tangent(*a.as_ref().domain().end())?
            .tangent()
            .as_vector();
        let v = b
            .as_ref()
            .evaluate_with_tangent(*b.as_ref().domain().start())?
            .tangent()
            .as_vector();
        Ok(u.dot(v)? >= tolerance.angular().cos())
    };
    let mut labels = vec![0];
    for pair in pieces.windows(2) {
        labels.push(labels.last().unwrap() + usize::from(!smooth(&pair[0], &pair[1])?));
    }
    if closed {
        let seam_smooth = smooth(pieces.last().unwrap(), &pieces[0])?;
        let last = *labels.last().unwrap();
        let corners = last + usize::from(!seam_smooth);
        // A closed NURBS with one kink is privately split to expose its two
        // ends. Preserve that split instead of merging them back together.
        if corners == 1 {
            return Ok((0..pieces.len()).collect());
        }
        // Native Chamfer keeps the original seam as a picking boundary,
        // including a geometrically smooth seam.
    }
    Ok(labels)
}
