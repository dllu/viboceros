//! Opt-in proposal complexity policy; never a geometric accuracy certificate.
use super::*;

/// Experimental refinement budget motivated by equal-degree Rhino captures.
/// A one-patch proposal is always eligible. Larger proposals must stay within
/// the source's simple-basis counts and reduce at least one axis. Repeated
/// interior knots do not increase these ceilings.
///
/// This tests candidate complexity only: it predicts neither fitting success
/// nor native null results. Existing bounded fitters do not apply this policy.
/// Degree changes, unclamped and discontinuous bases remain unqualified.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SurfaceFitRefinementBudget {
    initial: [usize; 2],
    maximum: [usize; 2],
}

impl SurfaceFitRefinementBudget {
    /// `None` means the input/target basis relationship is not supported by
    /// this policy. Geometry and the source basis remain unchanged.
    pub fn try_from_surface(
        source: &NurbsSurface,
        degree: [usize; 2],
    ) -> Result<Option<Self>, GeometryError> {
        validate_source(source)?;
        if degree.iter().any(|p| !(1..=11).contains(p)) {
            return Err(unsupported(
                "refinement budgets require target degrees 1..=11",
            ));
        }
        if degree != [source.degree_u(), source.degree_v()] {
            return Ok(None);
        }
        let counts = [
            source.control_point_count_u(),
            source.control_point_count_v(),
        ];
        let knots = [source.knots_u(), source.knots_v()];
        let mut maximum = [0; 2];
        for a in 0..2 {
            let p = degree[a];
            let k = knots[a];
            let n = counts[a];
            if k[..=p].iter().any(|t| *t != k[p])
                || k[n..].iter().any(|t| *t != k[n])
                || k[p + 1..n].chunk_by(|x, y| x == y).any(|g| g.len() > p)
            {
                return Ok(None);
            }
            maximum[a] = p + k[p..=n].windows(2).filter(|w| w[0] < w[1]).count();
        }
        Ok(Some(Self {
            initial: degree.map(|p| p + 1),
            maximum,
        }))
    }

    pub const fn initial_control_count(&self) -> [usize; 2] {
        self.initial
    }
    pub const fn maximum_control_count(&self) -> [usize; 2] {
        self.maximum
    }

    /// Eligibility is independent of position, weights and tolerance after
    /// input admission. It does not replace fitting or certificate work limits.
    pub fn admits_control_count(&self, count: [usize; 2]) -> bool {
        count == self.initial
            || ((0..2).all(|a| count[a] >= self.initial[a] && count[a] <= self.maximum[a])
                && (0..2).any(|a| count[a] < self.maximum[a]))
    }
}

#[cfg(test)]
mod tests;
