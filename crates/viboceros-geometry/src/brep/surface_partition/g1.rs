//! Adaptive G1 witnesses between the public query's transverse samples.
use super::*;
mod cone;
mod exact_ray;
mod extract;

pub(super) fn augment(
    surface: &NurbsSurface,
    angle: Real,
    cuts: &mut [Vec<Real>; 2],
    budget: &mut usize,
) -> Result<(), GeometryError> {
    if angle.cos() <= -1. {
        return Ok(());
    }
    let axes = [
        (
            surface.degree_u(),
            surface.knots_u(),
            surface.control_point_count_u(),
        ),
        (
            surface.degree_v(),
            surface.knots_v(),
            surface.control_point_count_v(),
        ),
    ];
    for axis in 0..2 {
        let (p, knots, count) = axes[axis];
        let (_, transverse, other_count) = axes[1 - axis];
        let q = axes[1 - axis].0;
        let known = cuts[axis].clone();
        for group in knots.chunk_by(|a, b| a == b) {
            let t = group[0];
            if group.len() < p
                || t <= knots[p]
                || t >= knots[count]
                || known
                    .binary_search_by(|x| x.partial_cmp(&t).unwrap())
                    .is_ok()
            {
                continue;
            }
            let left = knots.partition_point(|x| *x < t) - 1;
            let right = knots.partition_point(|x| *x <= t) - 1;
            for span in q..other_count {
                if transverse[span] == transverse[span + 1] {
                    continue;
                }
                let seam = extract::seam(surface, axis, [left, right], span, budget)?;
                if cone::has_break(seam, angle.cos(), budget)? {
                    cuts[axis].push(t);
                    break;
                }
            }
        }
        cuts[axis].sort_by(Real::total_cmp);
    }
    Ok(())
}

struct Seam {
    directions: [Vec<[Real; 3]>; 2],
    weights: Vec<[Real; 1]>,
    depth: u8,
}

fn unresolved<T>() -> Result<T, GeometryError> {
    Err(GeometryError::InvalidBrepTopology {
        context: "adaptive surface crease continuity could not be resolved",
    })
}
