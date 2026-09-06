use super::bezier::{self, Budget, Net};
use crate::{BoundingBox3, GeometryError, NurbsSurface, Tolerance};

impl NurbsSurface {
    /// Tolerance-controlled box of the complete active, untrimmed surface.
    /// Homogeneous tensor subdivision retains intermediate projective controls;
    /// unresolved poles or exhausted resource budgets return an error. This is
    /// floating-point refinement, not certified interval arithmetic.
    pub fn tight_bounds(&self, tolerance: Tolerance) -> Result<BoundingBox3, GeometryError> {
        let mut budget = Budget::default();
        let mut nodes = Vec::new();
        let p = self.degree_u();
        let q = self.degree_v();
        let nu = self.control_point_count_u();
        let nv = self.control_point_count_v();
        for v in q..nv {
            if self.knots_v()[v] == self.knots_v()[v + 1] {
                continue;
            }
            for u in p..nu {
                if self.knots_u()[u] == self.knots_u()[u + 1] {
                    continue;
                }
                budget.initial((p + 1).saturating_mul(q + 1))?;
                let controls = (v - q..=v)
                    .flat_map(|j| (u - p..=u).map(move |i| self.control_points()[j * nu + i]))
                    .collect::<Vec<_>>();
                let mut net = Net::new([p, q], &controls)?;
                net.extract_axis(0, self.knots_u(), u, &mut budget)?;
                net.extract_axis(1, self.knots_v(), v, &mut budget)?;
                nodes.push(net);
            }
        }
        bezier::bounds(nodes, budget, tolerance)
    }
}

#[cfg(test)]
mod tests;
