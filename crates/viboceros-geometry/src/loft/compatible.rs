use super::*;

/// Exact degree/knot matching, followed by Rhino's common-basis end-weight policy.
pub(super) fn prepare(curves: &[NurbsCurve]) -> Result<Vec<NurbsCurve>, GeometryError> {
    let limit = GeometryError::LoftResourceLimit {
        context: "compatible section controls",
        maximum: MAX_LOFT_SECTION_CONTROLS,
    };
    let mut sections = crate::section_basis::prepare(curves, MAX_LOFT_SECTION_CONTROLS, limit)?;
    for section in &mut sections {
        *section = section.try_normalized_end_weights()?;
    }
    // Rhino normalizes weights after structural matching and retains the first
    // normalized section's knots for the whole control net. For unequal endpoint
    // weight ratios this is not necessarily shape-preserving on later sections.
    let knots = sections[0].knots().to_vec();
    for section in &mut sections[1..] {
        *section = NurbsCurve::try_new_rational(
            section.degree(),
            section.control_points().to_vec(),
            knots.clone(),
        )?;
    }
    Ok(sections)
}
