//! Refit real section copies before frame placement and pole construction.
use super::*;

impl Sweep1 {
    /// Refit copies of the real profiles while retaining their rail stations.
    /// A single smooth profile uses two successive fitting passes. Each pass is
    /// certified against its input; their errors can accumulate beyond the
    /// nominal tolerance against the original profile, as in Rhino's path.
    pub fn refit_sections(
        sections: &[SweepSection],
        tolerance: Real,
    ) -> Result<Vec<SweepSection>, GeometryError> {
        Self::refit_sections_with_operations(
            sections,
            tolerance,
            &mut |c| c.clamped_to_active_domain(),
            &mut |c, d| c.try_change_degree(d, false),
            &mut |c, k, m| c.try_insert_knot(k, m),
        )
    }
    pub fn refit_sections_with_operations<E: From<GeometryError>>(
        sections: &[SweepSection],
        tolerance: Real,
        clamp: &mut impl FnMut(&NurbsCurve) -> Result<NurbsCurve, E>,
        elevate: &mut impl FnMut(&NurbsCurve, usize) -> Result<NurbsCurve, E>,
        refine: &mut impl FnMut(&NurbsCurve, Real, usize) -> Result<NurbsCurve, E>,
    ) -> Result<Vec<SweepSection>, E> {
        if sections.is_empty() || sections.len() > MAX_SECTIONS {
            return Err(invalid("expected 1 to 256 sections").into());
        }
        if sections.iter().any(|s| !s.parameter.is_finite())
            || sections
                .windows(2)
                .any(|s| s[0].parameter >= s[1].parameter)
        {
            return Err(
                invalid("section rail parameters must be finite and strictly increasing").into(),
            );
        }
        let curves = sections.iter().map(|s| s.curve.clone()).collect::<Vec<_>>();
        crate::loft::prepare_sweep_with_operations(&curves, tolerance, clamp, elevate, refine).map(
            |curves| {
                sections
                    .iter()
                    .zip(curves)
                    .map(|(s, curve)| SweepSection {
                        parameter: s.parameter,
                        curve,
                    })
                    .collect()
            },
        )
    }
}

#[cfg(test)]
mod tests;
