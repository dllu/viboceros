//! Rebuild real profiles before common-basis matching and point-pole creation.
use super::*;

impl Sweep1 {
    pub fn rebuild_sections(
        sections: &[SweepSection],
        point_count: usize,
        tolerance: Tolerance,
    ) -> Result<Vec<SweepSection>, GeometryError> {
        Self::rebuild_sections_with(
            sections,
            point_count,
            tolerance,
            &mut |curve, count, degree, tol| {
                crate::try_rebuild_open_curve(CurveRef::NurbsCurve(curve), count, degree, tol)
            },
        )
    }

    /// Preserve source stations and delegate clamped rebuilding to the backend.
    pub fn rebuild_sections_with<E: From<GeometryError>>(
        sections: &[SweepSection],
        point_count: usize,
        tolerance: Tolerance,
        rebuild: &mut impl FnMut(&NurbsCurve, usize, usize, Tolerance) -> Result<NurbsCurve, E>,
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
        crate::try_rebuild_nurbs_sections_with(
            sections.iter().map(|s| &s.curve),
            point_count,
            MAX_SECTION_CONTROLS,
            invalid("rebuilt section control budget exceeded"),
            tolerance,
            rebuild,
        )
        .map(|curves| {
            sections
                .iter()
                .zip(curves)
                .map(|(source, curve)| SweepSection {
                    parameter: source.parameter,
                    curve,
                })
                .collect()
        })
    }
}
