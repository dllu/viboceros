//! Multiple original operands, component contributors and pair containment.
use super::*;
use arrangement::{Arrangement, Assembled, Expression};

/// Union of up to 128 certified polyhedral operands, with original face,
/// material and boundary contributors for each component. An input containing
/// disjoint shells can contribute to several components. Construction uses one
/// arrangement of original faces, without rounded intermediate Booleans.
pub fn union_polyhedral_breps(
    breps: &[&Brep],
    tolerance: Tolerance,
) -> Result<Vec<BrepUnionComponent>, GeometryError> {
    if breps.is_empty() {
        return Ok(Vec::new());
    }
    let mut budget = Budget(EXACT_WORK_LIMIT);
    let built = arrangement::build(breps, tolerance, &mut budget)?;
    let assembled = arrangement::assemble(&built, Expression::Union, tolerance, &mut budget)?;
    let mut sources = vec![BTreeSet::new(); assembled.bodies.len()];
    let mut boundary = vec![BTreeSet::new(); assembled.bodies.len()];
    for (sample, body) in built.samples.iter().zip(&assembled.sample_bodies) {
        if let Some(body) = body {
            budget.spend(sample.inside.len())?;
            sources[*body].extend(
                sample
                    .inside
                    .iter()
                    .enumerate()
                    .filter_map(|(i, v)| v.then_some(i)),
            );
        }
    }
    for cell in &built.cells {
        budget.spend(1)?;
        let values = cell
            .sides
            .map(|i| Expression::Union.includes(&built.samples[i].inside));
        if values[0] != values[1] {
            let sample = cell.sides[usize::from(!values[0])];
            let body =
                assembled.sample_bodies[sample].ok_or(GeometryError::UnrepresentableBrepBoolean)?;
            boundary[body].insert(cell.source[0]);
        }
    }
    let mut result = Vec::new();
    for (i, body) in assembled.bodies.into_iter().enumerate() {
        if sources[i].is_empty() || boundary[i].is_empty() {
            return Err(GeometryError::UnrepresentableBrepBoolean);
        }
        result.push(BrepUnionComponent {
            brep: body.brep,
            source_indices: sources[i].iter().copied().collect(),
            boundary_source_indices: boundary[i].iter().copied().collect(),
            face_sources: body
                .faces
                .into_iter()
                .map(|f| assembled.face_sources[f])
                .collect(),
        });
    }
    result.sort_by_key(|body| body.source_indices[0]);
    Ok(result)
}

/// Common solid intersection of original polyhedral operands. Unlike convex
/// common intersection this can return several material components and cavities.
pub fn intersect_polyhedral_breps(
    breps: &[&Brep],
    tolerance: Tolerance,
) -> Result<Vec<BrepPolyhedralBooleanComponent>, GeometryError> {
    if breps.is_empty() {
        return Ok(Vec::new());
    }
    let mut budget = Budget(EXACT_WORK_LIMIT);
    let built = arrangement::build(breps, tolerance, &mut budget)?;
    let assembled = arrangement::assemble(&built, Expression::Common, tolerance, &mut budget)?;
    Ok(assembled
        .bodies
        .into_iter()
        .map(|body| BrepPolyhedralBooleanComponent {
            brep: body.brep,
            face_sources: body
                .faces
                .into_iter()
                .map(|i| assembled.face_sources[i])
                .collect(),
        })
        .collect())
}

/// Intersection of two unions of original polyhedral operands. Pair reports
/// include every positive-volume contributing cross-set pair. Maximal pairs
/// remove contained pair regions within each output component, using exact
/// arrangement membership states; equal regions retain the earliest pair.
pub fn intersect_polyhedral_brep_sets(
    first: &[&Brep],
    second: &[&Brep],
    tolerance: Tolerance,
) -> Result<Vec<BrepSetIntersection>, GeometryError> {
    if first.len() + second.len() > 128 {
        return Err(GeometryError::BrepBooleanWorkLimit);
    }
    if first.is_empty() || second.is_empty() {
        return Ok(Vec::new());
    }
    let mut budget = Budget(EXACT_WORK_LIMIT);
    let refs = first.iter().chain(second).copied().collect::<Vec<_>>();
    let built = arrangement::build(&refs, tolerance, &mut budget)?;
    let assembled = arrangement::assemble(
        &built,
        Expression::Sets(first.len()),
        tolerance,
        &mut budget,
    )?;
    let states = component_states(&built, &assembled, &mut budget)?;
    let mut result = Vec::new();
    for (index, body) in assembled.bodies.into_iter().enumerate() {
        let mut regions = Vec::new();
        for a in 0..first.len() {
            for b in 0..second.len() {
                budget.spend(states[index].len())?;
                let coverage = states[index]
                    .iter()
                    .enumerate()
                    .filter_map(|(i, mask)| (mask[a] && mask[first.len() + b]).then_some(i))
                    .collect::<BTreeSet<_>>();
                if !coverage.is_empty() {
                    regions.push(([a, b], coverage));
                }
            }
        }
        let mut maximal = Vec::new();
        for (i, (pair, coverage)) in regions.iter().enumerate() {
            let mut keep = true;
            for (j, (_, other)) in regions.iter().enumerate() {
                if i == j {
                    continue;
                }
                budget.spend(coverage.len() + other.len())?;
                if coverage.is_subset(other) && (coverage != other || j < i) {
                    keep = false;
                    break;
                }
            }
            if keep {
                maximal.push(*pair);
            }
        }
        if maximal.is_empty() {
            return Err(GeometryError::UnrepresentableBrepBoolean);
        }
        result.push(BrepSetIntersection {
            brep: body.brep,
            pairs: regions.into_iter().map(|(p, _)| p).collect(),
            maximal_pairs: maximal,
            face_sources: body
                .faces
                .into_iter()
                .map(|i| assembled.face_sources[i])
                .collect(),
        });
    }
    result.sort_by_key(|body| body.pairs[0]);
    Ok(result)
}

/// Target minus the union of up to 127 original polyhedral cutters. Components
/// retain cavities and original face ownership; an empty cutter list certifies
/// and rebuilds the target. Interactive deletion/contact policies are separate.
pub fn subtract_polyhedral_breps(
    target: &Brep,
    cutters: &[&Brep],
    tolerance: Tolerance,
) -> Result<Vec<BrepDifferenceComponent>, GeometryError> {
    let refs = std::iter::once(target)
        .chain(cutters.iter().copied())
        .collect::<Vec<_>>();
    let mut budget = Budget(EXACT_WORK_LIMIT);
    let built = arrangement::build(&refs, tolerance, &mut budget)?;
    let assembled = arrangement::assemble(&built, Expression::Difference, tolerance, &mut budget)?;
    Ok(assembled
        .bodies
        .into_iter()
        .map(|body| BrepDifferenceComponent {
            brep: body.brep,
            face_sources: body
                .faces
                .into_iter()
                .map(|i| assembled.face_sources[i])
                .collect(),
        })
        .collect())
}

/// These are exact membership states of adjacent volume cells, not approximate
/// samples. Any nonempty bounded Boolean expression has a positive-area patch
/// on an original boundary. Our arrangement subdivides every such patch and
/// retains both strictly adjacent volume states. Thus absence of inner && !outer
/// states proves pair-region containment, including holes and disconnected parts.
fn component_states(
    built: &Arrangement<'_>,
    assembled: &Assembled,
    budget: &mut Budget,
) -> Result<Vec<BTreeSet<Vec<bool>>>, GeometryError> {
    let mut result = vec![BTreeSet::new(); assembled.bodies.len()];
    for (sample, body) in built.samples.iter().zip(&assembled.sample_bodies) {
        if let Some(body) = body {
            budget.spend(sample.inside.len())?;
            result[*body].insert(sample.inside.clone());
        }
    }
    Ok(result)
}
