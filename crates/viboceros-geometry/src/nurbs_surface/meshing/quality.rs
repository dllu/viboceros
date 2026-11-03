//! Select a rectangular grid from rational Bezier error estimates.
use super::*;
mod net;
use net::Net;
#[cfg(test)]
mod tests;

const MAX_VERTICES: usize = 1_048_576;
const MAX_WORK: usize = 67_108_864;

/// Same normalized-density size rule as public OpenNURBS
/// ON_MeshParameters::ToleranceFromObjectSize. Zero disables this criterion.
pub(super) fn chord_height(density: Real, size: Real) -> Real {
    if density == 0. {
        return 0.;
    }
    let exponent = if density < 0.5 {
        1. + density * (6. - 4. * density)
    } else {
        2. + 2. * density
    };
    size * 10_f64.powf(-exponent)
}

pub(super) fn counts(surface: &NurbsSurface, density: Real) -> Result<[usize; 2], GeometryError> {
    let mut counts = [
        if surface.degree_u() > 1 { 4 } else { 1 },
        if surface.degree_v() > 1 { 4 } else { 1 },
    ];
    if density == 0. {
        return Ok(counts);
    }
    let bounds = surface.control_point_bounds();
    let height = chord_height(density, bounds.min().distance_to(bounds.max())?);
    if height <= 0. || !height.is_finite() {
        return Err(GeometryError::MeshSubdivisionLimit);
    }
    let patches = surface
        .try_bezier_patches()?
        .iter()
        .map(Net::new)
        .collect::<Result<Vec<_>, _>>()?;
    let mut budget = Budget::default();
    loop {
        let vertices = counts[0]
            .checked_add(1)
            .and_then(|u| counts[1].checked_add(1).and_then(|v| u.checked_mul(v)))
            .and_then(|n| n.checked_mul(patches.len()));
        if vertices.is_none_or(|n| n > MAX_VERTICES) {
            return Err(GeometryError::MeshSubdivisionLimit);
        }
        let depths = counts.map(|n| n.ilog2());
        let mut failure = None;
        for net in &patches {
            failure = needs_refinement(net.clone(), depths, height, &mut budget)?;
            if failure.is_some() {
                break;
            }
        }
        let Some(axis) = failure else {
            return Ok(counts);
        };
        counts[axis] = counts[axis]
            .checked_mul(2)
            .ok_or(GeometryError::MeshSubdivisionLimit)?;
    }
}

#[derive(Default)]
struct Budget(usize);
impl Budget {
    fn charge(&mut self, net: &Net) -> Result<(), GeometryError> {
        self.0 = self.0.saturating_add(net.work());
        if self.0 > MAX_WORK {
            Err(GeometryError::MeshSubdivisionLimit)
        } else {
            Ok(())
        }
    }
}

fn needs_refinement(
    net: Net,
    depths: [u32; 2],
    height: Real,
    budget: &mut Budget,
) -> Result<Option<usize>, GeometryError> {
    budget.charge(&net)?;
    if depths != [0, 0] {
        let axis = usize::from(depths[1] > depths[0]);
        let mut depths = depths;
        depths[axis] -= 1;
        let [a, b] = net.split(axis);
        if let Some(axis) = needs_refinement(a, depths, height, budget)? {
            return Ok(Some(axis));
        }
        return needs_refinement(b, depths, height, budget);
    }
    if net.chord_bound().is_some_and(|error| error <= height) {
        return Ok(None);
    }
    // Compare the geometric improvement from each axis. Extrusions should
    // refine around their curved profile without duplicating straight rows.
    let scores = [0, 1].map(|axis| {
        net.clone()
            .split(axis)
            .iter()
            .map(|n| n.chord_bound().unwrap_or(Real::INFINITY))
            .fold(0., Real::max)
    });
    let axis = if scores[0].is_infinite() && scores[1].is_infinite() {
        net.weight_variation_axis()
    } else {
        usize::from(scores[1] < scores[0])
    };
    Ok(Some(axis))
}
