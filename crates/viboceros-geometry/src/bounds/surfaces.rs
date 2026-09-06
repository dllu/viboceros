use super::bezier::{self, Budget, Net};
use crate::{BoundingBox3, GeometryError, NurbsSurface, Tolerance};

impl NurbsSurface {
    /// Tolerance-controlled box of the complete active, untrimmed surface.
    /// Homogeneous tensor subdivision retains intermediate projective controls;
    /// unresolved poles or exhausted resource budgets return an error. This is
    /// floating-point refinement, not certified interval arithmetic.
    pub fn tight_bounds(&self, tolerance: Tolerance) -> Result<BoundingBox3, GeometryError> {
        let mut budget = Budget::default();
        let nodes = patches(self, &mut budget)?
            .into_iter()
            .map(|p| p.net)
            .collect();
        bezier::bounds(nodes, &mut budget, tolerance)
    }
}

pub(super) struct Patch {
    pub(super) net: Net,
    pub(super) domain: [[f64; 2]; 2],
    pub(super) full_order_sides: [[bool; 2]; 2],
}

pub(super) fn patches(
    surface: &NurbsSurface,
    budget: &mut Budget,
) -> Result<Vec<Patch>, GeometryError> {
    let mut nodes = Vec::new();
    let p = surface.degree_u();
    let q = surface.degree_v();
    let nu = surface.control_point_count_u();
    let nv = surface.control_point_count_v();
    for v in q..nv {
        if surface.knots_v()[v] == surface.knots_v()[v + 1] {
            continue;
        }
        for u in p..nu {
            if surface.knots_u()[u] == surface.knots_u()[u + 1] {
                continue;
            }
            budget.initial((p + 1).saturating_mul(q + 1))?;
            let controls = (v - q..=v)
                .flat_map(|j| (u - p..=u).map(move |i| surface.control_points()[j * nu + i]))
                .collect::<Vec<_>>();
            let mut net = Net::new([p, q], &controls)?;
            net.extract_axis(0, surface.knots_u(), u, budget)?;
            net.extract_axis(1, surface.knots_v(), v, budget)?;
            nodes.push(Patch {
                net,
                full_order_sides: [
                    full_order_sides(surface.knots_u(), p, u, nu),
                    full_order_sides(surface.knots_v(), q, v, nv),
                ],
                domain: [
                    [surface.knots_u()[u], surface.knots_u()[u + 1]],
                    [surface.knots_v()[v], surface.knots_v()[v + 1]],
                ],
            });
        }
    }
    Ok(nodes)
}

fn full_order_sides(knots: &[f64], degree: usize, span: usize, count: usize) -> [bool; 2] {
    [
        knots[span] > knots[degree]
            && knots[span - degree..=span]
                .iter()
                .all(|k| *k == knots[span]),
        knots[span + 1] < knots[count]
            && knots[span + 1..=span + degree + 1]
                .iter()
                .all(|k| *k == knots[span + 1]),
    ]
}

#[cfg(test)]
mod tests;
