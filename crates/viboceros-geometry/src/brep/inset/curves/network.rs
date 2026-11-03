//! Creased offsets of a closed NURBS edge, including connector crossings.
use super::*;
mod assembly;
mod bindings;
mod input;
mod plan;
mod regions;
mod topology;

/// A smooth curved seam uses an annulus. A straight seam keeps its cap
/// vertex but contributes no connector; a G2 seam contributes a connector.
pub(super) fn seam_policy(
    source: &NurbsCurve,
    tolerance: Tolerance,
) -> Result<Option<bool>, GeometryError> {
    let mut spans = source.spans();
    let first = spans
        .next()
        .ok_or_else(|| invalid("inset source has no spans"))?;
    let last = spans.last().unwrap_or(first);
    let first = source.try_trimmed(first.0..=first.1)?;
    let last = source.try_trimmed(last.0..=last.1)?;
    let seam = PolyCurve3::try_new(vec![
        CurveSegment3::NurbsCurve(last.clone()),
        CurveSegment3::NurbsCurve(first.clone()),
    ])?;
    let join = *seam.segment_domain(1)?.start();
    if seam
        .to_nurbs()?
        .discontinuity_parameters(crate::CurveContinuity::G2, tolerance.angular())?
        .binary_search_by(|t| t.total_cmp(&join))
        .is_ok()
    {
        return Ok(Some(true));
    }
    if groups::is_line(&first, tolerance)? && groups::is_line(&last, tolerance)? {
        Ok(Some(false))
    } else {
        Ok(None)
    }
}

#[allow(clippy::too_many_arguments)]
pub(super) fn inset(
    brep: &mut Brep,
    face: &BrepFace,
    boundary: &BrepLoop,
    normal: UnitVector3,
    distance: Real,
    tolerance: Tolerance,
    operations: PlanarInsetOperations,
    work: &mut Work,
    seam_connector: bool,
) -> Result<Boundary, GeometryError> {
    if boundary.trims.len() != 1 {
        return Err(invalid(
            "closed inset networks require one original boundary edge",
        ));
    }
    let anchor = brep.vertices[boundary.trims[0].vertices[0]].point;
    let source = joined(brep, &boundary.trims)?;
    work.charge_curve(&source)?;
    let raw = crate::curve_offset::fit_normal_locus(
        &source,
        distance,
        normal,
        tolerance,
        operations.edit_points,
    )?;
    work.charge_curve(&raw)?;
    let mut pieces = crate::curve_offset::trim_normal_locus(
        &source,
        &raw,
        distance,
        normal,
        tolerance,
        operations.curves,
        true,
    )?;
    // Cusp trimming changes cap topology and connector correspondence. Keep
    // that unfinished branch explicit while regular closed-edge networks use
    // the same fit and intersection machinery.
    if pieces.len() != 1 || pieces[0].domain() != (0. ..=1.) {
        return Err(GeometryError::Degenerate {
            context: "NURBS offset cusp",
        });
    }
    let whole = pieces.pop().unwrap();
    let parameters =
        whole.discontinuity_parameters(crate::CurveContinuity::G2, std::f64::consts::PI / 180.)?;
    if parameters.is_empty() {
        let inset = build::cap(brep, &face.surface, vec![whole], tolerance)?;
        return Ok(Boundary {
            borders: vec![build::annulus(face, boundary, &inset)?],
            inset,
        });
    }
    let mut cap = Vec::new();
    let mut remainder = whole;
    for t in parameters {
        let (left, right) = remainder.try_split(t)?;
        cap.push(left);
        remainder = right;
    }
    cap.push(remainder);
    for c in &cap {
        work.charge_curve(c)?;
    }
    work.charge_extra_strips(
        &face.surface,
        cap.len().saturating_sub(boundary.trims.len()),
    )?;
    let connectors = cap
        .iter()
        .skip(usize::from(!seam_connector))
        .map(|c| {
            NurbsCurve::try_new(
                1,
                vec![anchor, c.evaluate(*c.domain().start())?],
                vec![0., 0., 1., 1.],
            )
        })
        .collect::<Result<Vec<_>, GeometryError>>()?;
    for curve in &connectors {
        work.charge_curve(curve)?;
    }
    let plan = plan::plan(brep, face, boundary, &cap, &connectors, tolerance, work)?;
    assembly::build(brep, face, boundary, cap, connectors, plan, tolerance, work)
}
