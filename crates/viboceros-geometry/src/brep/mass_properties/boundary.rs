//! Shared, bounded boundary intervals for planar and nonplanar integration.
use super::super::{collect_bernstein_roots, scalar_bezier_spans};
use super::BrepFace;
use crate::{GeometryError, NurbsCurve2, NurbsSurface, Real};
use std::borrow::Cow;

const MAX_BOUNDARY_INTERVALS: usize = 65_536;
pub(super) const MAX_SURFACE_EVALUATIONS: usize = 2_000_000;

pub(super) struct BoundaryCurve<'a> {
    pub curve: Cow<'a, NurbsCurve2>,
    pub intervals: Vec<[Real; 2]>,
}

pub(super) fn prepare<'a>(
    face: &'a BrepFace,
    surface: &NurbsSurface,
) -> Result<Vec<BoundaryCurve<'a>>, GeometryError> {
    let mut spans = 0usize;
    // Preflight every curve before copying frames or performing root searches.
    for trim in face.loops.iter().flat_map(|l| &l.trims) {
        spans = spans
            .checked_add(trim.curve.spans().count())
            .filter(|&n| n <= MAX_BOUNDARY_INTERVALS)
            .ok_or(GeometryError::NumericalIntegrationDidNotConverge)?;
    }
    if spans == 0 {
        return Err(GeometryError::NumericalIntegrationDidNotConverge);
    }
    let mut count = 0usize;
    face.loops
        .iter()
        .flat_map(|l| &l.trims)
        .map(|trim| {
            let curve = trim.curve.for_integration()?;
            let intervals = intervals(&curve, surface)?;
            count = count
                .checked_add(intervals.len())
                .filter(|&n| n <= MAX_BOUNDARY_INTERVALS)
                .ok_or(GeometryError::NumericalIntegrationDidNotConverge)?;
            Ok(BoundaryCurve { curve, intervals })
        })
        .collect()
}

fn intervals(curve: &NurbsCurve2, surface: &NurbsSurface) -> Result<Vec<[Real; 2]>, GeometryError> {
    let domain = curve.domain();
    let mut breaks = vec![*domain.start(), *domain.end()];
    breaks.extend(curve.spans().map(|(_, end)| end));
    for (axis, knots) in [
        (0, surface.spans_u().map(|(_, end)| end).collect::<Vec<_>>()),
        (1, surface.spans_v().map(|(_, end)| end).collect::<Vec<_>>()),
    ] {
        // Planarity does not imply an affine UV map. The boundary image can
        // cross a surface knot even when the UV trim itself is a single line.
        // Isolate crossings in the same local frame used for quadrature.
        // Natural-domain endpoints cannot be crossed by a valid trim.
        for &knot in knots.iter().take(knots.len().saturating_sub(1)) {
            for span in scalar_bezier_spans(curve, axis, knot)? {
                if !span.coefficients.iter().all(|value| *value == 0.0) {
                    collect_bernstein_roots(
                        &span.coefficients,
                        span.parameter,
                        0,
                        true,
                        true,
                        &mut breaks,
                    );
                    if breaks.len() > MAX_BOUNDARY_INTERVALS {
                        return Err(GeometryError::NumericalIntegrationDidNotConverge);
                    }
                }
            }
        }
    }
    breaks.sort_by(Real::total_cmp);
    breaks.dedup();
    Ok(breaks
        .windows(2)
        .filter_map(|p| (p[0] < p[1]).then_some([p[0], p[1]]))
        .collect())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn interval_budget_rejects_excessive_trim_frames_before_root_search() {
        let source =
            super::super::tests::round_trim(super::super::tests::paraboloid(), &[0.5], false);
        let mut face = source.faces()[0].clone();
        let trim = face.loops[0].trims[0].clone();
        let count = MAX_BOUNDARY_INTERVALS / trim.curve.spans().count() + 1;
        // Preflight malformed over-budget data; this is not a valid face.
        face.loops[0].trims = vec![trim; count];
        assert!(matches!(
            prepare(&face, &face.surface),
            Err(GeometryError::NumericalIntegrationDidNotConverge)
        ));
    }
}
