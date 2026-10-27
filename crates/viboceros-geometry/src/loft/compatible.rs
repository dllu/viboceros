use super::*;

/// Region correspondence, exact basis matching, and common endpoint weights.
pub(super) fn prepare(curves: &[NurbsCurve]) -> Result<Vec<NurbsCurve>, GeometryError> {
    prepare_with_operations(
        curves,
        &mut |curve| curve.clamped_to_active_domain(),
        &mut |curve, degree| curve.try_change_degree(degree, false),
        &mut |curve, knot, target| curve.try_insert_knot(knot, target),
    )
}

pub(super) fn prepare_with_operations<E: From<GeometryError>>(
    curves: &[NurbsCurve],
    clamp: &mut impl FnMut(&NurbsCurve) -> Result<NurbsCurve, E>,
    elevate: &mut impl FnMut(&NurbsCurve, usize) -> Result<NurbsCurve, E>,
    refine: &mut impl FnMut(&NurbsCurve, Real, usize) -> Result<NurbsCurve, E>,
) -> Result<Vec<NurbsCurve>, E> {
    let parameterized = super::parameterize::prepare(curves, clamp)?;
    match_basis(&parameterized, clamp, elevate, refine)
}

pub(super) fn prepare_refit_with_operations<E: From<GeometryError>>(
    curves: &[NurbsCurve],
    angle: Option<Real>,
    clamp: &mut impl FnMut(&NurbsCurve) -> Result<NurbsCurve, E>,
    elevate: &mut impl FnMut(&NurbsCurve, usize) -> Result<NurbsCurve, E>,
    refine: &mut impl FnMut(&NurbsCurve, Real, usize) -> Result<NurbsCurve, E>,
) -> Result<Vec<NurbsCurve>, E> {
    let parameterized = super::parameterize::prepare_refit(curves, angle, clamp)?;
    match_basis(&parameterized, clamp, elevate, refine)
}

pub(super) fn match_basis<E: From<GeometryError>>(
    curves: &[NurbsCurve],
    clamp: &mut impl FnMut(&NurbsCurve) -> Result<NurbsCurve, E>,
    elevate: &mut impl FnMut(&NurbsCurve, usize) -> Result<NurbsCurve, E>,
    refine: &mut impl FnMut(&NurbsCurve, Real, usize) -> Result<NurbsCurve, E>,
) -> Result<Vec<NurbsCurve>, E> {
    let limit = GeometryError::LoftResourceLimit {
        context: "compatible section controls",
        maximum: MAX_LOFT_SECTION_CONTROLS,
    };
    let sections = crate::section_basis::prepare_with_operations(
        curves,
        MAX_LOFT_SECTION_CONTROLS,
        limit,
        crate::section_basis::WeightScale::PerSection,
        clamp,
        elevate,
        refine,
    )?;
    Ok(crate::section_basis::normalized_end_weights(&sections)?)
}
