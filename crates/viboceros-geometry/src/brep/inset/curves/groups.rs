//! Curved G2 joins share a source group; straight subdivisions retain strips.
use super::*;

pub(super) struct Group {
    pub curve: NurbsCurve,
    pub trims: Vec<BrepTrim>,
}

fn polycurve(curves: impl IntoIterator<Item = NurbsCurve>) -> Result<PolyCurve3, GeometryError> {
    PolyCurve3::try_new(
        curves
            .into_iter()
            .map(CurveSegment3::NurbsCurve)
            .collect::<Vec<_>>(),
    )
}

pub(super) fn collect(
    brep: &Brep,
    trims: &[BrepTrim],
    loose: bool,
    crease: bool,
    tolerance: Tolerance,
) -> Result<Vec<Group>, GeometryError> {
    let curves = trims
        .iter()
        .map(|t| {
            if loose {
                oriented(brep, t)
            } else {
                joined(brep, std::slice::from_ref(t))
            }
        })
        .collect::<Result<Vec<_>, _>>()?;
    if curves.is_empty() {
        return Err(invalid("inset requires closed trim loops"));
    }
    if curves.len() == 1 {
        return Ok(curves
            .into_iter()
            .zip(trims)
            .map(|(curve, t)| Group {
                curve,
                trims: vec![t.clone()],
            })
            .collect());
    }
    let whole = polycurve(curves.iter().cloned())?;
    let lines = curves
        .iter()
        .map(|c| is_line(c, tolerance))
        .collect::<Result<Vec<_>, _>>()?;
    let joined = whole.to_nurbs()?;
    let breaks =
        joined.discontinuity_parameters(crate::CurveContinuity::G2, tolerance.angular())?;
    let mut corners = Vec::new();
    for i in 1..curves.len() {
        let join = *whole.segment_domain(i)?.start();
        if lines[i - 1] && lines[i] || breaks.binary_search_by(|t| t.total_cmp(&join)).is_ok() {
            corners.push(i);
        }
    }
    // Public discontinuity traversal excludes a closed seam. Put the original
    // seam inside a two-segment chart to classify it by the same predicate.
    let seam = polycurve([curves.last().unwrap().clone(), curves[0].clone()])?;
    let join = *seam.segment_domain(1)?.start();
    if lines[0] && lines[lines.len() - 1]
        || seam
            .to_nurbs()?
            .discontinuity_parameters(crate::CurveContinuity::G2, tolerance.angular())?
            .binary_search_by(|t| t.total_cmp(&join))
            .is_ok()
    {
        corners.insert(0, 0);
    }
    if corners.is_empty() {
        return Ok(vec![Group {
            curve: joined,
            trims: trims.to_vec(),
        }]);
    }
    if !crease {
        return Ok(curves
            .into_iter()
            .zip(trims)
            .map(|(curve, t)| Group {
                curve,
                trims: vec![t.clone()],
            })
            .collect());
    }
    let mut groups = Vec::with_capacity(corners.len());
    for (i, &start) in corners.iter().enumerate() {
        let end = corners
            .get(i + 1)
            .copied()
            .unwrap_or(corners[0] + curves.len());
        let indices = (start..end).map(|j| j % curves.len()).collect::<Vec<_>>();
        groups.push(Group {
            curve: polycurve(indices.iter().map(|&j| curves[j].clone()))?.to_nurbs()?,
            trims: indices.into_iter().map(|j| trims[j].clone()).collect(),
        });
    }
    Ok(groups)
}

pub(super) fn is_line(curve: &NurbsCurve, tolerance: Tolerance) -> Result<bool, GeometryError> {
    let tight = Tolerance::try_new(
        tolerance.absolute() * 0.01,
        tolerance.relative(),
        tolerance.angular(),
    )?;
    Ok(matches!(
        curve.analytic_form(tight)?,
        Some(CurveSegment3::Line(_))
    ))
}
