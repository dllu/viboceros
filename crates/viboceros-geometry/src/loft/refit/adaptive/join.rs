//! Retiming and Hermite reconstruction within curvature-continuous regions.
use super::*;
mod rebuild;

pub(super) struct Refinement {
    pub curve: NurbsCurve,
    pub source: Option<NurbsCurve>,
}

pub(super) fn reconstruct_at_cuts(
    source: &NurbsCurve,
    cuts: &[Real],
) -> Result<NurbsCurve, GeometryError> {
    rebuild::interpolate(source, cuts)
}

pub(super) fn construct_smooth(raw: &NurbsCurve) -> Result<NurbsCurve, GeometryError> {
    let cuts = raw
        .knots()
        .chunk_by(|a, b| a == b)
        .map(|g| g[0])
        .collect::<Vec<_>>();
    c1_region_impl(raw, &cuts, 0, cuts.len() - 1, false)
}

/// The retimed region has its own parameter clock. Its C1 rebuild chooses
/// intervals afresh instead of inheriting the endpoint-frame fitting cuts.
/// Only the original-source caller can certify the final assembled proposal.
#[cfg(test)]
pub(super) fn construct_refined(
    raw: &NurbsCurve,
    angle: Real,
    tolerance: Real,
) -> Result<Option<NurbsCurve>, GeometryError> {
    Ok(construct_refined_with_policy(raw, angle, tolerance, None)?.map(|r| r.curve))
}

pub(super) fn construct_refined_with_policy(
    raw: &NurbsCurve,
    angle: Real,
    tolerance: Real,
    original: Option<&NurbsCurve>,
) -> Result<Option<Refinement>, GeometryError> {
    if !super::super::super::parameterize::fitted_region_breaks(raw, angle)?.is_empty() {
        return construct(raw, angle).map(|curve| {
            Some(Refinement {
                curve,
                source: None,
            })
        });
    }
    let cuts = raw
        .knots()
        .chunk_by(|a, b| a == b)
        .map(|g| g[0])
        .collect::<Vec<_>>();
    let retimed = c1_region_impl(raw, &cuts, 0, cuts.len() - 1, false)?;
    let curve = if let Some(original) = original {
        rebuild::propose_image(&retimed, original, tolerance)
    } else {
        rebuild::propose(&retimed, tolerance)
    }?;
    Ok(curve.map(|curve| Refinement {
        curve,
        source: original.map(|_| retimed),
    }))
}

pub(super) fn construct(raw: &NurbsCurve, angle: Real) -> Result<NurbsCurve, GeometryError> {
    let cuts = raw
        .knots()
        .chunk_by(|a, b| a == b)
        .map(|g| g[0])
        .collect::<Vec<_>>();
    let mut regions = vec![0];
    for t in super::super::super::parameterize::fitted_region_breaks(raw, angle)? {
        let i = cuts
            .binary_search_by(|k| k.total_cmp(&t))
            .map_err(|_| unsupported())?;
        regions.push(i);
    }
    regions.push(cuts.len() - 1);
    let mut controls = Vec::new();
    let mut knots = vec![0.; 4];
    for pair in regions.windows(2) {
        let (a, b) = (pair[0], pair[1]);
        let (start, end) = (cuts[a], cuts[b]);
        let curve = c1_region_impl(raw, &cuts, a, b, regions.len() == 2)?;
        controls.extend(
            curve
                .control_points()
                .iter()
                .copied()
                .skip(usize::from(!controls.is_empty())),
        );
        knots.extend(
            curve.knots()[4..curve.knots().len() - 4]
                .iter()
                .map(|t| start + t * (end - start)),
        );
        knots.extend([end; 3]);
    }
    knots.push(1.);
    NurbsCurve::try_new_rational(3, controls, knots)
}

fn unsupported() -> GeometryError {
    GeometryError::UnsupportedCurveApproximation {
        context: "Loft Refit currently requires regular adaptive cubic handles",
    }
}

#[cfg(test)]
fn c1_region(
    raw: &NurbsCurve,
    cuts: &[Real],
    first: usize,
    last: usize,
) -> Result<NurbsCurve, GeometryError> {
    c1_region_impl(raw, cuts, first, last, true)
}

fn c1_region_impl(
    raw: &NurbsCurve,
    cuts: &[Real],
    first: usize,
    last: usize,
    rebuild: bool,
) -> Result<NurbsCurve, GeometryError> {
    let points = &raw.control_points()[3 * first..=3 * last];
    if last - first == 1 {
        return NurbsCurve::try_new_rational(
            3,
            points.to_vec(),
            [vec![0.; 4], vec![1.; 4]].concat(),
        );
    }
    let range = cuts[last] - cuts[first];
    let target = cuts[first..=last]
        .iter()
        .map(|t| (t - cuts[first]) / range)
        .collect::<Vec<_>>();
    let mut widths = vec![target[1]];
    for i in 1..last - first {
        let incoming = points[3 * i - 1]
            .point()
            .distance_to(points[3 * i].point())?;
        let outgoing = points[3 * i]
            .point()
            .distance_to(points[3 * i + 1].point())?;
        let next = widths[i - 1] * (outgoing / incoming);
        if !next.is_finite() || next <= 0. {
            return Err(unsupported());
        }
        widths.push(next);
    }
    let total: Real = widths.iter().sum();
    if !total.is_finite() || total <= 0. {
        return Err(unsupported());
    }
    let mut source_knots = vec![0.; 4];
    let mut prefix = 0.;
    for w in &widths[..widths.len() - 1] {
        prefix += w;
        source_knots.extend([prefix / total; 3]);
    }
    source_knots.extend([1.; 4]);
    if !rebuild {
        let controls = points
            .iter()
            .enumerate()
            .filter_map(|(i, p)| (i == 0 || i + 1 == points.len() || i % 3 != 0).then_some(*p))
            .collect();
        let mut knots = vec![0.; 4];
        let mut prefix = 0.;
        for width in &widths[..widths.len() - 1] {
            prefix += width;
            knots.extend([prefix / total; 2]);
        }
        knots.extend([1.; 4]);
        return NurbsCurve::try_new_rational(3, controls, knots);
    }
    let source = NurbsCurve::try_new_rational(3, points.to_vec(), source_knots)?;
    let mut jets = target
        .iter()
        .map(|t| source.evaluate_with_derivative(*t))
        .collect::<Result<Vec<_>, _>>()?;
    jets[0].0 = points[0].point();
    jets.last_mut().unwrap().0 = points.last().unwrap().point();
    let mut controls = vec![jets[0].0];
    for (i, pair) in target.windows(2).enumerate() {
        let h = (pair[1] - pair[0]) / 3.;
        controls.push(jets[i].0.translated(jets[i].1.scaled(h)?)?);
        controls.push(jets[i + 1].0.translated(jets[i + 1].1.scaled(-h)?)?);
    }
    controls.push(jets.last().unwrap().0);
    let mut knots = vec![0.; 4];
    for t in &target[1..target.len() - 1] {
        knots.extend([*t; 2]);
    }
    knots.extend([1.; 4]);
    NurbsCurve::try_new(3, controls, knots)
}

#[cfg(test)]
mod tests;
