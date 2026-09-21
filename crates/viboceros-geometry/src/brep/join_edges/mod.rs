//! Transactional assembly of explicitly paired, complete naked boundaries.
use super::*;

mod certificate;
#[cfg(test)]
mod tests;
mod topology;

const MAX_JOIN_PAIRS: usize = 100_000;
const MAX_JOIN_CONTROLS: usize = 4_000_000;

fn invalid(context: &'static str) -> GeometryError {
    GeometryError::InvalidBrepTopology { context }
}

impl Brep {
    /// Joins explicit pairs of complete naked edges without refitting geometry.
    ///
    /// Each tuple is `(retained_edge, removed_edge, opposite_curve_directions)`.
    /// An edge may occur in only one pair. Combine separate B-reps first with
    /// [`Self::try_combine`]; split partial overlaps first with
    /// [`Self::try_split_edges_at_parameters`]. This is an assembly primitive,
    /// not automatic edge discovery or the interactive Join command.
    ///
    /// Acceptance is deliberately conservative: paired curves must have equal
    /// degree/control count, exactly affine-equivalent full knot vectors and
    /// exactly proportional weights. Every corresponding control-point distance
    /// must be at most `join_distance`, in absolute model units. This supplies a
    /// whole-curve convex-hull bound, including rational and reversed curves.
    /// Other representations of the same locus are rejected, not sampled into
    /// an approximate match. Source component tolerances do not widen this test.
    ///
    /// The retained edge curve, all surfaces, UV trims and their domains are
    /// unchanged. Surviving edges/vertices retain source table order. Merged
    /// vertices use the lowest source index; a transitive vertex cluster is
    /// rejected if any member moves farther than `join_distance`. Tolerances
    /// conservatively include original uncertainty plus the displacement bound.
    /// Face senses are reconciled, keeping the first face of each connected
    /// component fixed (no outward-solid or cavity classification). Nonmanifold
    /// inputs and inconsistent orientation cycles are rejected.
    ///
    /// At most 100,000 pairs and 4,000,000 paired controls are accepted. Inputs
    /// are never mutated, including on error; the assembled result is validated
    /// at the separately supplied modeling `tolerance`.
    pub fn try_join_edge_pairs(
        &self,
        pairs: &[(usize, usize, bool)],
        join_distance: Real,
        tolerance: Tolerance,
    ) -> Result<Self, GeometryError> {
        require_nonnegative_finite(join_distance, "B-rep join distance")?;
        if pairs.len() > MAX_JOIN_PAIRS {
            return Err(invalid("too many B-rep edge join pairs"));
        }
        let mut claimed = vec![false; self.edges.len()];
        let counts = self.edge_use_counts();
        if counts.iter().any(|&n| n > 2) {
            return Err(invalid("edge joining requires manifold input"));
        }
        if pairs.is_empty() {
            return Self::try_new(
                self.vertices.clone(),
                self.edges.clone(),
                self.faces.clone(),
                tolerance,
            );
        }
        let mut controls = 0_usize;
        // Validate every index, multiplicity and work limit before curve work.
        for &(a, b, _) in pairs {
            for e in [a, b] {
                if e >= self.edges.len() || claimed[e] || counts[e] != 1 {
                    return Err(invalid("join pairs must name distinct unused naked edges"));
                }
                claimed[e] = true;
                controls = controls.saturating_add(self.edges[e].curve.control_points().len());
                if controls > MAX_JOIN_CONTROLS {
                    return Err(invalid("B-rep edge join control budget exceeded"));
                }
            }
        }
        let bounds = pairs
            .iter()
            .map(|&(a, b, reversed)| {
                certificate::curve_bound(
                    &self.edges[a].curve,
                    &self.edges[b].curve,
                    reversed,
                    join_distance,
                )
                .ok_or_else(|| invalid("edge curves lack a whole-curve join certificate"))
            })
            .collect::<Result<Vec<_>, _>>()?;
        topology::assemble(self, pairs, &bounds, join_distance, tolerance)
    }
}
