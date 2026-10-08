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

/// One connected target region with a constant cutter membership signature.
#[derive(Clone, Debug)]
pub struct BrepSplitComponent {
    pub brep: Brep,
    /// Original target is operand zero; cutters are operands one through N.
    pub face_sources: Vec<[usize; 2]>,
    /// One entry per original cutter; true means this region is inside it.
    pub cutter_membership: Vec<bool>,
    /// At each ordered cutter step, the number of connected children on this
    /// side of this region's parent. Counts are derived from exact cell states.
    pub branch_component_counts: Vec<usize>,
}

/// Partition a closed polyhedral target by all original cutter boundaries.
/// Unlike subtraction or intersection with a union, overlapping cutters keep
/// their internal interfaces. One exact arrangement is shared by every region;
/// no rounded intermediate B-reps become new operands. Disconnected components
/// of a signature are separate outputs and cavity shells remain attached.
/// Empty cutters certify and rebuild the target. Native contact and deletion
/// policies remain outside this mathematical operation. The ordinary
/// polyhedral input certificates and a shared work/output budget apply.
pub fn split_polyhedral_brep(
    target: &Brep,
    cutters: &[&Brep],
    tolerance: Tolerance,
) -> Result<Vec<BrepSplitComponent>, GeometryError> {
    let refs = std::iter::once(target)
        .chain(cutters.iter().copied())
        .collect::<Vec<_>>();
    let mut budget = Budget(EXACT_WORK_LIMIT);
    let built = arrangement::build(&refs, tolerance, &mut budget)?;
    let mut parents = built
        .samples
        .iter()
        .map(|s| s.inside[0].then_some(0usize))
        .collect::<Vec<_>>();
    let mut paths = vec![Vec::<usize>::new()];
    let mut result = Vec::new();
    for depth in 1..=cutters.len().max(1) {
        let depth = depth.min(cutters.len());
        let mut signatures = BTreeSet::new();
        for sample in &built.samples {
            budget.spend(sample.inside.len())?;
            if sample.inside[0] {
                signatures.insert(sample.inside[1..=depth].to_vec());
            }
        }
        let mut next_parents = vec![None; built.samples.len()];
        let mut next = Vec::new();
        let mut counts = BTreeMap::<(usize, bool), usize>::new();
        let mut lineage = Vec::new();
        let mut total_faces = 0usize;
        for signature in signatures {
            let assembled = arrangement::assemble(
                &built,
                Expression::Region(&signature),
                tolerance,
                &mut budget,
            )?;
            let offset = next.len();
            let mut original_parents = vec![None; assembled.bodies.len()];
            for (i, body) in assembled.sample_bodies.iter().enumerate() {
                budget.spend(1)?;
                if let Some(body) = body {
                    let parent = parents[i].ok_or(GeometryError::UnrepresentableBrepBoolean)?;
                    if original_parents[*body]
                        .replace(parent)
                        .is_some_and(|old| old != parent)
                    {
                        return Err(GeometryError::UnrepresentableBrepBoolean);
                    }
                    next_parents[i] = Some(offset + body);
                }
            }
            for (i, body) in assembled.bodies.into_iter().enumerate() {
                total_faces = total_faces
                    .checked_add(body.brep.faces.len())
                    .ok_or(GeometryError::BrepBooleanWorkLimit)?;
                if total_faces > MAX_OUTPUT_FACES {
                    return Err(GeometryError::BrepBooleanWorkLimit);
                }
                let parent =
                    original_parents[i].ok_or(GeometryError::UnrepresentableBrepBoolean)?;
                let branch = (parent, signature.last().copied().unwrap_or(false));
                *counts.entry(branch).or_default() += 1;
                lineage.push(branch);
                next.push(BrepSplitComponent {
                    brep: body.brep,
                    face_sources: body
                        .faces
                        .into_iter()
                        .map(|i| assembled.face_sources[i])
                        .collect(),
                    cutter_membership: signature.clone(),
                    branch_component_counts: Vec::new(),
                });
            }
        }
        let mut next_paths = Vec::with_capacity(lineage.len());
        for (piece, branch) in next.iter_mut().zip(lineage) {
            let mut path = paths[branch.0].clone();
            if depth != 0 {
                path.push(counts[&branch]);
            }
            piece.branch_component_counts = path.clone();
            next_paths.push(path);
        }
        parents = next_parents;
        paths = next_paths;
        result = next;
    }
    Ok(result)
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
