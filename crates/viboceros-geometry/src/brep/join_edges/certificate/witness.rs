//! Certified intermediate curves avoid dense pairwise rational products.
use super::*;

pub(in crate::brep::join_edges) fn whole_curve_bound_with_witnesses(
    a: &NurbsCurve,
    b: &NurbsCurve,
    reversed: bool,
    limit: Real,
    witnesses: &[&NurbsCurve],
    mut charge: impl FnMut(usize) -> Result<(), GeometryError>,
) -> Result<Option<Real>, GeometryError> {
    if witnesses.is_empty() {
        return whole_curve_bound(a, b, reversed, limit, charge);
    }
    charge(
        a.control_points()
            .len()
            .saturating_add(b.control_points().len()),
    )?;
    if let Some(bound) = curve_bound(a, b, reversed, limit) {
        return Ok(Some(bound));
    }
    // Polynomial pairs already have cheap exact Bernstein products.
    if [a, b].iter().all(|c| {
        c.control_points()
            .iter()
            .all(|p| p.weight() == c.control_points()[0].weight())
    }) {
        return whole_curve_bound(a, b, reversed, limit, charge);
    }
    for end in [false, true] {
        if let (Some(x), Some(y)) = (
            product::endpoint(a, end),
            product::endpoint(b, end ^ reversed),
        ) && point_bound(x, y, limit).is_none()
        {
            return Ok(None);
        }
    }
    for witness in witnesses {
        for direction in [false, true] {
            let Some(x) = whole_curve_bound(a, witness, direction, limit * 0.5, &mut charge)?
            else {
                continue;
            };
            let Some(y) =
                whole_curve_bound(b, witness, direction ^ reversed, limit * 0.5, &mut charge)?
            else {
                continue;
            };
            // Each certificate supplies a complete oriented correspondence.
            // Their composition through the witness bounds the original pair.
            let sum = add_bound(x, y)?;
            if sum <= limit {
                return Ok(Some(sum));
            }
        }
    }
    whole_curve_bound(a, b, reversed, limit, charge)
}
