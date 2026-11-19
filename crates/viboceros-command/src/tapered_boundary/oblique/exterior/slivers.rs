//! Stitch shallow rim gaps to the finite cutter while keeping the swept walls.
use super::*;
use std::collections::BTreeMap;
use viboceros_geometry::{BrepEdge, WeightedPoint3};

// The boundary join uses a model-space gap distinct from the document's
// intersection tolerance. Each changed edge stores its own certified bound.
const MAX_JOIN_GAP: f64 = 1e-4;

pub(super) struct OutsideSpan {
    pub(super) edge: usize,
    pub(super) domain: [f64; 2],
}

struct Replacement {
    domain: [f64; 2],
    curve: NurbsCurve,
    tolerance: f64,
}

pub(super) fn stitch(
    section: &Brep,
    gaps: &[OutsideSpan],
    outline: &[&NurbsCurve],
    tolerance: Tolerance,
) -> Result<Option<Brep>, GeometryError> {
    let mut replacements = BTreeMap::<usize, Vec<Replacement>>::new();
    for gap in gaps {
        let edge = &section.edges()[gap.edge];
        let old = edge.curve().try_trimmed(gap.domain[0]..=gap.domain[1])?;
        let mut candidate = None;
        for boundary in outline {
            let Some(curve) = between_contacts(&old, boundary, tolerance)? else {
                continue;
            };
            // Allow one intersection epsilon at the join cutoff. The new
            // edge also carries the prior discrepancy from the wall trim.
            let epsilon = tolerance.absolute().min(MAX_JOIN_GAP / 100.);
            let Some(bound) = deviation(&old, &curve, MAX_JOIN_GAP + epsilon)? else {
                continue;
            };
            if candidate.is_some() {
                return Ok(None);
            }
            candidate = Some(Replacement {
                domain: gap.domain,
                curve,
                tolerance: bound + edge.tolerance().max(tolerance.absolute()),
            });
        }
        let Some(candidate) = candidate else {
            return Ok(None);
        };
        replacements.entry(gap.edge).or_default().push(candidate);
    }

    let splits = replacements
        .iter()
        .filter_map(|(&edge, replacements)| {
            let domain = section.edges()[edge].curve().domain();
            let mut cuts = replacements
                .iter()
                .flat_map(|r| r.domain)
                .filter(|t| *t > *domain.start() && *t < *domain.end())
                .collect::<Vec<_>>();
            cuts.sort_unstable_by(f64::total_cmp);
            cuts.dedup();
            (!cuts.is_empty()).then_some((edge, cuts))
        })
        .collect::<Vec<_>>();
    let (section, split_edges) =
        section.try_split_edges_at_parameters_with_map(&splits, tolerance)?;
    let split_edges = split_edges.into_iter().collect::<BTreeMap<_, _>>();
    let mut edges = section.edges().to_vec();
    for (source, replacements) in replacements {
        let candidates = split_edges
            .get(&source)
            .map_or(std::slice::from_ref(&source), Vec::as_slice);
        for replacement in replacements {
            let Ok(position) = candidates.binary_search_by(|&index| {
                let domain = section.edges()[index].curve().domain();
                domain.start().total_cmp(&replacement.domain[0])
            }) else {
                return Ok(None);
            };
            let index = candidates[position];
            if *section.edges()[index].curve().domain().end() != replacement.domain[1] {
                return Ok(None);
            }
            edges[index] = BrepEdge::try_new(
                edges[index].vertices(),
                replacement.curve,
                replacement.tolerance,
            )?;
        }
    }
    let joined = Brep::try_new(
        section.vertices().to_vec(),
        edges,
        section.faces().to_vec(),
        tolerance,
    )?;
    Ok(joined.is_solid().then_some(joined))
}

fn deviation(
    rim: &NurbsCurve,
    replacement: &NurbsCurve,
    limit: f64,
) -> Result<Option<f64>, GeometryError> {
    let domain = rim.domain();
    let degree = rim.degree();
    let count = rim.control_points().len();
    if replacement.degree() != 1
        || replacement.control_points().len() != 2
        || rim.knots()[..=degree].iter().any(|k| k != domain.start())
        || rim.knots()[count..].iter().any(|k| k != domain.end())
    {
        return rim.projected_curve_deviation_bound(replacement, limit);
    }

    // Use the rim's own basis to parameterize the closest points on a
    // straight replacement. The projected controls stay on the segment
    // and their endpoint controls are its endpoints, so the image is onto.
    let start = replacement.control_points()[0].point();
    let end = replacement.control_points()[1].point();
    let chord = start.vector_to(end)?;
    let direction = chord.normalized_nonzero()?.as_vector();
    let length = chord.length()?;
    let mut controls = Vec::with_capacity(count);
    for (i, control) in rim.control_points().iter().enumerate() {
        let point = if i == 0 {
            start
        } else if i == count - 1 {
            end
        } else {
            let length_along = start.vector_to(control.point())?.dot(direction)?;
            start.translated(direction.scaled(length_along.clamp(0., length))?)?
        };
        controls.push(WeightedPoint3::try_new(point, control.weight())?);
    }
    let projected = NurbsCurve::try_new_rational(degree, controls, rim.knots().to_vec())?;
    rim.parameter_curve_deviation_bound(&projected, limit)
}

fn between_contacts(
    rim: &NurbsCurve,
    boundary: &NurbsCurve,
    tolerance: Tolerance,
) -> Result<Option<NurbsCurve>, GeometryError> {
    let domain = rim.domain();
    let ends = [rim.evaluate(*domain.start())?, rim.evaluate(*domain.end())?];
    let a = boundary.closest_parameter(ends[0], tolerance)?;
    let b = boundary.closest_parameter(ends[1], tolerance)?;
    if a == b
        || !boundary.evaluate(a)?.is_near(ends[0], tolerance)
        || !boundary.evaluate(b)?.is_near(ends[1], tolerance)
    {
        return Ok(None);
    }
    let part = boundary.try_trimmed(a.min(b)..=a.max(b))?;
    Ok(Some(if a > b { part.reversed()? } else { part }))
}
