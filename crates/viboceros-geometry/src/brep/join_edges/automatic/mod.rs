//! Bounded full-boundary discovery and straight partial-overlap planning.
use super::*;
mod components;
mod overlap;
pub(super) mod rebuild;
mod search;
mod selection;
mod subdivision;
#[cfg(test)]
mod tests;

const MAX_SOURCES: usize = 10_000;
const MAX_NAKED: usize = 200_000;
const MAX_CANDIDATES: usize = 1_000_000;
/// One connected output, with sorted indices of the sources contributing faces.
/// A disconnected source can contribute to more than one output.
#[derive(Clone, Debug, PartialEq)]
pub struct BrepJoinComponent {
    pub brep: Brep,
    pub source_indices: Vec<usize>,
    pub joined_edge_count: usize,
}

/// Assembly outputs and certified cross-source boundary contacts before
/// ambiguity resolution. Contact alone does not establish a topological join.
#[derive(Clone, Debug, PartialEq)]
pub struct BrepJoinReport {
    pub components: Vec<BrepJoinComponent>,
    /// Sorted unique pairs of original source indices, each in ascending order.
    pub candidate_source_pairs: Vec<[usize; 2]>,
}

/// Discovers matching naked boundaries between inputs and produces connected,
/// oriented B-reps. Existing unmatched edges within a single input are not sewn.
///
/// Complete curved boundaries use the whole-curve certificate documented by
/// [`Brep::try_join_edge_pairs`]. Exactly straight, monotone clamped curves also
/// support independent degrees/parameter speeds and partial overlaps; all
/// necessary cuts update every incident UV trim before assembly. Other curved
/// partial overlaps and incompatible curved representations remain unmatched.
///
/// Surfaces and UV geometry are never refitted. Nearby boundary vertices are
/// aligned to incident-edge-weighted means, even when no mate is accepted.
/// Incident clamped edges use certified chord-based control adjustment;
/// changed nonclamped edges retain the original assembly policy. Movement of
/// a vertex cluster or adjusted edge beyond `join_distance` fails atomically.
/// Exact surface isocurves can certify tighter component uncertainty;
/// other boundaries propagate their validated uncertainty conservatively.
/// Mutual unique candidates join first. Remaining ambiguous
/// boundaries join only when they become mutually unique within a component
/// established by other edges. Every certified candidate participates, even
/// when another candidate has a smaller distance. Each piece is paired once.
/// Original vertices precede subdivision vertices. Complete spatial boundaries
/// precede cut ones; two cut boundaries and deferred closures retain the later
/// source's edge. Nonorientable
/// candidate sets fail atomically. Newly closed positive-volume shells are
/// oriented outward; zero-volume double sheets retain the first face's sense.
/// This does not classify nested/cavity solids or compute a Boolean union.
/// Successfully joined components coalesce redundant smooth valence-two edges
/// at the angular tolerance; untouched components and explicit edge-pair
/// assembly retain their original subdivisions. See [`Brep::try_merge_all_edges`].
///
/// Empty input returns no components. Limits: 10,000 sources, 200,000 naked
/// edges, one million broad-phase pairs, and 16 million charged work units.
/// Inputs are immutable; callers own selection and object replacement policy.
pub fn join_breps(
    sources: &[&Brep],
    join_distance: Real,
    tolerance: Tolerance,
) -> Result<Vec<BrepJoinComponent>, GeometryError> {
    Ok(join_breps_with_report(sources, join_distance, tolerance)?.components)
}

/// Like [`join_breps`], with original-source candidate provenance for callers
/// implementing incremental selection. Input-internal mated edges are not
/// candidates; only certified contacts between naked boundary pieces are listed.
pub fn join_breps_with_report(
    sources: &[&Brep],
    join_distance: Real,
    tolerance: Tolerance,
) -> Result<BrepJoinReport, GeometryError> {
    require_nonnegative_finite(join_distance, "B-rep join distance")?;
    if sources.len() > MAX_SOURCES {
        return Err(invalid("too many B-rep join sources"));
    }
    if sources.is_empty() {
        return Ok(BrepJoinReport {
            components: Vec::new(),
            candidate_source_pairs: Vec::new(),
        });
    }
    let mut budget = Budget(MAX_WORK);
    let mut face_sources = Vec::new();
    for (i, source) in sources.iter().enumerate() {
        budget.charge(source.faces.len().saturating_add(source.edges.len()))?;
        if !source.is_manifold() {
            return Err(invalid("edge joining requires manifold input"));
        }
        face_sources.extend(std::iter::repeat_n(i, source.faces.len()));
    }
    let mut combined =
        Brep::try_combine(sources.iter().map(|s| (*s).clone()).collect(), tolerance)?;
    let candidates = search::find(&combined, join_distance, &mut budget)?;
    let source_of_edge = edge_sources(&combined, &face_sources);
    let mut cuts = BTreeMap::<usize, Vec<Real>>::new();
    for &(a, b) in &candidates {
        if source_of_edge[a] == source_of_edge[b] {
            continue;
        }
        let a_curve = &combined.edges[a].curve;
        let b_curve = &combined.edges[b].curve;
        if full_match(a_curve, b_curve, join_distance, &mut budget)?.is_some() {
            continue;
        }
        if let Some(intervals) = overlap::intervals(a_curve, b_curve, join_distance, &mut budget)? {
            for (edge, interval) in [a, b].into_iter().zip(intervals) {
                let domain = combined.edges[edge].curve.domain();
                for t in interval {
                    if t > *domain.start() && t < *domain.end() {
                        cuts.entry(edge).or_default().push(t);
                    }
                }
            }
        }
    }
    let mut split_edges = vec![false; combined.edges.len()];
    let candidates = if cuts.is_empty() {
        candidates
    } else {
        let cuts = cuts
            .into_iter()
            .map(|(edge, mut p)| {
                p.sort_by(Real::total_cmp);
                p.dedup();
                (edge, p)
            })
            .collect::<Vec<_>>();
        (combined, split_edges) = subdivision::apply(sources, &cuts, tolerance)?;
        search::find(&combined, join_distance, &mut budget)?
    };
    let mut matches = Vec::new();
    let mut vertex_contacts = Vec::new();
    let source_of_edge = edge_sources(&combined, &face_sources);
    for (a, b) in candidates {
        if source_of_edge[a] == source_of_edge[b] {
            continue;
        }
        budget.charge(4)?;
        for &av in &combined.edges[a].vertices {
            for &bv in &combined.edges[b].vertices {
                if certificate::point_bound(
                    combined.vertices[av].point,
                    combined.vertices[bv].point,
                    join_distance,
                )
                .is_some()
                {
                    vertex_contacts.push((av, bv));
                }
            }
        }
        let ac = &combined.edges[a].curve;
        let bc = &combined.edges[b].curve;
        if let Some((reversed, bound)) = full_match(ac, bc, join_distance, &mut budget)? {
            matches.push((bound, a, b, reversed));
        }
    }
    let pairs = selection::pairs(&combined, &matches, &split_edges);
    let mut candidate_source_pairs = matches
        .iter()
        .map(|&(_, a, b, _)| {
            let mut pair = [source_of_edge[a], source_of_edge[b]];
            pair.sort_unstable();
            pair
        })
        .collect::<Vec<_>>();
    candidate_source_pairs.sort_unstable();
    candidate_source_pairs.dedup();
    let edge_uncertainty = combined.edges.iter().map(|e| e.tolerance).collect();
    if let Some(rebuilt) = rebuild::apply(
        &combined,
        &vertex_contacts,
        join_distance,
        tolerance,
        &mut budget,
    )? {
        combined = rebuilt;
    }
    let mut joined =
        combined.join_edge_pairs_with_budget(&pairs, join_distance, tolerance, &mut budget)?;
    rebuild::tighten_joined_edges(
        &mut joined,
        &pairs,
        edge_uncertainty,
        tolerance,
        &mut budget,
    )?;
    let mut components = components::collect(joined, &combined, &pairs, &face_sources, tolerance)?;
    for component in &mut components {
        if component.joined_edge_count > 0 {
            component.brep = super::merge::merge(
                &component.brep,
                tolerance.angular().min(std::f64::consts::PI),
                tolerance,
                &mut budget,
            )?;
        }
    }
    Ok(BrepJoinReport {
        components,
        candidate_source_pairs,
    })
}

fn edge_sources(brep: &Brep, face_sources: &[usize]) -> Vec<usize> {
    let mut result = vec![0; brep.edges.len()];
    for usage in brep.trim_uses() {
        if let Some(edge) = usage.trim.edge {
            result[edge] = face_sources[usage.face];
        }
    }
    result
}

fn full_match(
    a: &NurbsCurve,
    b: &NurbsCurve,
    distance: Real,
    budget: &mut Budget,
) -> Result<Option<(bool, Real)>, GeometryError> {
    let mut best = None;
    for reversed in [false, true] {
        if let Some(bound) =
            certificate::whole_curve_bound(a, b, reversed, distance, |n| budget.charge(n))?
            && best.is_none_or(|(_, previous)| bound < previous)
        {
            best = Some((reversed, bound));
        }
    }
    Ok(best)
}
