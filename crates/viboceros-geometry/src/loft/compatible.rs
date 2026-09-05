use super::*;

/// Exact degree/knot matching, followed by Rhino's common-basis end-weight policy.
pub(super) fn prepare(curves: &[NurbsCurve]) -> Result<Vec<NurbsCurve>, GeometryError> {
    let limit = GeometryError::LoftResourceLimit {
        context: "compatible section controls",
        maximum: MAX_LOFT_SECTION_CONTROLS,
    };
    let sections = crate::section_basis::prepare(
        curves,
        MAX_LOFT_SECTION_CONTROLS,
        limit,
        crate::section_basis::WeightScale::PerSection,
    )?;
    crate::section_basis::normalized_end_weights(&sections)
}
