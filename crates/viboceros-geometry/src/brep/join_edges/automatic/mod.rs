//! Bounded full-boundary discovery and straight partial-overlap planning.
use super::*;
mod components;
mod overlap;
mod search;
#[cfg(test)]
mod tests;

const MAX_SOURCES: usize = 10_000;
const MAX_NAKED: usize = 200_000;
const MAX_CANDIDATES: usize = 1_000_000;
const MAX_WORK: usize = 16_000_000;

struct Budget(usize);
impl Budget {
    fn charge(&mut self, amount: usize) -> Result<(), GeometryError> {
        self.0 = self
            .0
            .checked_sub(amount)
            .ok_or_else(|| invalid("B-rep join work budget exceeded"))?;
        Ok(())
    }
}

/// One connected output, with sorted indices of the sources contributing faces.
/// A disconnected source can contribute to more than one output.
#[derive(Clone, Debug, PartialEq)]
pub struct BrepJoinComponent {
    pub brep: Brep,
    pub source_indices: Vec<usize>,
    pub joined_edge_count: usize,
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
/// Surfaces and UV geometry are never refitted. Spatial edges/vertices retain
/// the assembly primitive's geometry-preserving policy, not Rhino's gap-edge
/// rebuilding policy. Matches are ordered by certified distance then source
/// edge indices; each boundary piece can be paired only once. Nonorientable
/// candidate sets fail atomically. Newly closed positive-volume shells are
/// oriented outward; zero-volume double sheets retain the first face's sense.
/// This does not classify nested/cavity solids or compute a Boolean union.
///
/// Empty input returns no components. Limits: 10,000 sources, 200,000 naked
/// edges, one million broad-phase pairs, and 16 million charged work units.
/// Inputs are immutable; callers own selection and object replacement policy.
pub fn join_breps(
    sources: &[&Brep],
    join_distance: Real,
    tolerance: Tolerance,
) -> Result<Vec<BrepJoinComponent>, GeometryError> {
    require_nonnegative_finite(join_distance, "B-rep join distance")?;
    if sources.len() > MAX_SOURCES {
        return Err(invalid("too many B-rep join sources"));
    }
    if sources.is_empty() {
        return Ok(Vec::new());
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
        budget.charge(
            a_curve
                .control_points()
                .len()
                .saturating_add(b_curve.control_points().len()),
        )?;
        if full_match(a_curve, b_curve, join_distance).is_some() {
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
        // Keep each source's new table entries next to that source. Besides
        // deterministic provenance, this ensures a shared partial boundary
        // retains the earlier source's curve and native parameter domain.
        let mut edge_offset = 0;
        let mut cut_offset = 0;
        let mut pieces = Vec::with_capacity(sources.len());
        for source in sources {
            let next_offset = edge_offset + source.edges.len();
            let count = cuts[cut_offset..].partition_point(|(edge, _)| *edge < next_offset);
            let local = cuts[cut_offset..cut_offset + count]
                .iter()
                .map(|(edge, parameters)| (edge - edge_offset, parameters.clone()))
                .collect::<Vec<_>>();
            pieces.push(if local.is_empty() {
                (*source).clone()
            } else {
                source.try_split_edges_at_parameters(&local, tolerance)?
            });
            edge_offset = next_offset;
            cut_offset += count;
        }
        combined = Brep::try_combine(pieces, tolerance)?;
        search::find(&combined, join_distance, &mut budget)?
    };
    let mut matches = Vec::new();
    let source_of_edge = edge_sources(&combined, &face_sources);
    for (a, b) in candidates {
        if source_of_edge[a] == source_of_edge[b] {
            continue;
        }
        let ac = &combined.edges[a].curve;
        let bc = &combined.edges[b].curve;
        budget.charge(
            ac.control_points()
                .len()
                .saturating_add(bc.control_points().len()),
        )?;
        if let Some((reversed, bound)) = full_match(ac, bc, join_distance) {
            matches.push((bound, a, b, reversed));
        }
    }
    matches.sort_by(|a, b| {
        a.0.total_cmp(&b.0)
            .then_with(|| (a.1, a.2, a.3).cmp(&(b.1, b.2, b.3)))
    });
    let mut used = vec![false; combined.edges.len()];
    let mut pairs = Vec::new();
    for (_, a, b, reversed) in matches {
        if !used[a] && !used[b] {
            used[a] = true;
            used[b] = true;
            pairs.push((a, b, reversed));
        }
    }
    let joined = combined.try_join_edge_pairs(&pairs, join_distance, tolerance)?;
    components::collect(joined, &combined, &pairs, &face_sources, tolerance)
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

fn full_match(a: &NurbsCurve, b: &NurbsCurve, distance: Real) -> Option<(bool, Real)> {
    [false, true]
        .into_iter()
        .filter_map(|r| certificate::curve_bound(a, b, r, distance).map(|d| (r, d)))
        .min_by(|a, b| a.1.total_cmp(&b.1).then_with(|| a.0.cmp(&b.0)))
}
