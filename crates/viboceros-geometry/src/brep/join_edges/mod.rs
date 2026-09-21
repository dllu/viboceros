//! Transactional assembly of explicitly paired, complete naked boundaries.
use super::*;

mod automatic;
pub use automatic::{BrepJoinComponent, BrepJoinReport, join_breps, join_breps_with_report};
mod certificate;
#[cfg(test)]
mod tests;
mod topology;

const MAX_JOIN_PAIRS: usize = 100_000;
const MAX_JOIN_CONTROLS: usize = 4_000_000;
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
    /// Acceptance uses whole-curve bounds in absolute model units. A shared
    /// rational basis permits a fast corresponding-control convex-hull bound.
    /// Otherwise, positive-basis curves of degree at most 16 are aligned over
    /// exact normalized knot spans. Rational Bernstein products and at most 16
    /// subdivision levels certify their difference under affine or positive
    /// projective parameter correspondences, including different degrees, knots,
    /// weights and reversal. Endpoint data only proposes a projective map;
    /// every accepted map must pass the complete span certificate.
    /// Clamped straight edges with exactly collinear, monotone controls also
    /// support different degrees, knots and rational parameter speeds. Their
    /// endpoint distances bound the complete oriented segment loci.
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
    /// At most 100,000 pairs, 4,000,000 paired controls and 16 million charged
    /// work units are accepted. Inputs
    /// are never mutated, including on error; the assembled result is validated
    /// at the separately supplied modeling `tolerance`.
    pub fn try_join_edge_pairs(
        &self,
        pairs: &[(usize, usize, bool)],
        join_distance: Real,
        tolerance: Tolerance,
    ) -> Result<Self, GeometryError> {
        self.join_edge_pairs_with_budget(pairs, join_distance, tolerance, &mut Budget(MAX_WORK))
    }

    fn join_edge_pairs_with_budget(
        &self,
        pairs: &[(usize, usize, bool)],
        join_distance: Real,
        tolerance: Tolerance,
        budget: &mut Budget,
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
                certificate::whole_curve_bound(
                    &self.edges[a].curve,
                    &self.edges[b].curve,
                    reversed,
                    join_distance,
                    |n| budget.charge(n),
                )?
                .ok_or_else(|| invalid("edge curves lack a whole-curve join certificate"))
            })
            .collect::<Result<Vec<_>, _>>()?;
        topology::assemble(self, pairs, &bounds, join_distance, tolerance)
    }
}
