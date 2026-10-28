//! Real profiles share their basis before collapsed point sections are added.
use super::*;

/// Optional point constraints at the start/end of the native rail domain.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct SweepEndPoints {
    pub start: Option<Point3>,
    pub end: Option<Point3>,
}
impl SweepEndPoints {
    pub fn is_empty(self) -> bool {
        self.start.is_none() && self.end.is_none()
    }
}

impl Sweep1 {
    /// Sweep real profiles and optional point ends at the rail endpoints.
    /// Poles inherit the adjacent prepared profile's positive weights. At least
    /// one real profile is required; point ends count toward the section limit.
    pub fn try_new_with_point_ends(
        rail: CurveRef<'_>,
        sections: &[SweepSection],
        ends: SweepEndPoints,
        style: SweepFrameStyle,
        blend: SweepBlend,
        tolerance: Tolerance,
    ) -> Result<Self, GeometryError> {
        Self::try_new_with_point_ends_and_section_preparation(
            rail,
            sections,
            ends,
            style,
            blend,
            tolerance,
            &mut |curves, maximum, limit| {
                crate::section_basis::prepare(
                    curves,
                    maximum,
                    limit,
                    crate::section_basis::WeightScale::Common,
                )
            },
        )
    }

    /// Reuse an exact section-preparation backend on the real profiles only.
    /// Constant pole curves are constructed after matching, so native curve
    /// matching never needs to interpret a zero-length profile.
    pub fn try_new_with_point_ends_and_section_preparation<E: From<GeometryError>>(
        rail: CurveRef<'_>,
        sections: &[SweepSection],
        ends: SweepEndPoints,
        style: SweepFrameStyle,
        blend: SweepBlend,
        tolerance: Tolerance,
        prepare: &mut impl FnMut(&[NurbsCurve], usize, GeometryError) -> Result<Vec<NurbsCurve>, E>,
    ) -> Result<Self, E> {
        if ends.is_empty() {
            return Self::try_new_with_section_preparation(
                rail, sections, style, blend, tolerance, prepare,
            );
        }
        let pole_count = usize::from(ends.start.is_some()) + usize::from(ends.end.is_some());
        if sections.is_empty() || sections.len() > MAX_SECTIONS - pole_count {
            return Err(invalid(
                "point-ended sweep requires real profiles within the section budget",
            )
            .into());
        }
        let count = sections.len() + pole_count;
        if sections
            .iter()
            .flat_map(|s| s.curve.control_points())
            .any(|c| c.weight() <= 0.)
        {
            return Err(invalid("sweep sections require positive rational weights").into());
        }
        let curves = sections.iter().map(|s| s.curve.clone()).collect::<Vec<_>>();
        let curves = crate::loft::prepare_section_regions(&curves, &mut |curve: &NurbsCurve| {
            curve.clamped_to_active_domain()
        })?;
        let prepared = prepare(
            &curves,
            MAX_SECTION_CONTROLS,
            invalid("section control budget exceeded"),
        )?;
        if prepared.len() != sections.len()
            || prepared
                .iter()
                .any(|c| c.control_points().len() > MAX_SECTION_CONTROLS)
        {
            return Err(invalid("section preparation exceeded the profile budget").into());
        }
        let pole = |curve: &NurbsCurve, point| {
            NurbsCurve::try_new_rational(
                curve.degree(),
                curve
                    .control_points()
                    .iter()
                    .map(|p| WeightedPoint3::try_new(point, p.weight()))
                    .collect::<Result<Vec<_>, _>>()?,
                curve.knots().to_vec(),
            )
        };
        let mut constraints = Vec::with_capacity(count);
        if let Some(point) = ends.start {
            constraints.push(SweepSection {
                parameter: *rail.domain().start(),
                curve: pole(&prepared[0], point)?,
            });
        }
        constraints.extend(
            sections
                .iter()
                .zip(&prepared)
                .map(|(s, curve)| SweepSection {
                    parameter: s.parameter,
                    curve: curve.clone(),
                }),
        );
        if let Some(point) = ends.end {
            constraints.push(SweepSection {
                parameter: *rail.domain().end(),
                curve: pole(prepared.last().unwrap(), point)?,
            });
        }
        Self::try_new_with_section_preparation(
            rail,
            &constraints,
            style,
            blend,
            tolerance,
            &mut |curves, _, _| Ok(curves.to_vec()),
        )
    }
}
