//! Local roundoff-bounded knot cleanup before column/row basis unification.
use super::*;

pub(super) fn compact(mut surface: NurbsSurface) -> Result<NurbsSurface, GeometryError> {
    for axis in 0..2 {
        let knots = if axis == 0 {
            surface.knots_u()
        } else {
            surface.knots_v()
        };
        let candidates = knots
            .iter()
            .copied()
            .filter(|&t| t > 0. && t < 1.)
            .collect::<Vec<_>>();
        for t in candidates {
            let knots = if axis == 0 {
                surface.knots_u()
            } else {
                surface.knots_v()
            };
            let multiplicity = knots.iter().filter(|&&k| k == t).count();
            if multiplicity == 0 {
                continue;
            }
            let candidate = if axis == 0 {
                surface.try_remove_knot_u_near(t)
            } else {
                surface.try_remove_knot_v_near(t)
            };
            let Ok(candidate) = candidate else {
                continue;
            };
            let restored = if axis == 0 {
                candidate.try_insert_knot_u(t, multiplicity)?
            } else {
                candidate.try_insert_knot_v(t, multiplicity)?
            };
            if restored.knots_u() != surface.knots_u() || restored.knots_v() != surface.knots_v() {
                continue;
            }
            let anchor = surface.control_points()[0].point();
            let (mut delta, mut weights, mut radius, mut minimum) =
                (0_f64, 0_f64, 0_f64, f64::INFINITY);
            for (a, b) in restored
                .control_points()
                .iter()
                .zip(surface.control_points())
            {
                if a.weight() <= 0. {
                    return Err(failure("Nonpositive SubD packing refinement weight"));
                }
                delta = delta.max(a.point().distance_to(b.point())?);
                weights = weights.max((a.weight() - b.weight()).abs());
                radius = radius.max(anchor.distance_to(b.point())?);
                minimum = minimum.min(a.weight());
            }
            let bound = delta + 2. * weights * radius / minimum;
            if bound <= 64. * f64::EPSILON * radius.max(1.) {
                surface = candidate;
            }
        }
    }
    Ok(surface)
}

pub(super) fn groups(
    surfaces: &[NurbsSurface],
    grid: [usize; 2],
    axis: usize,
) -> Vec<Vec<(f64, usize)>> {
    let mut result = vec![Vec::new(); grid[axis]];
    for (i, s) in surfaces.iter().enumerate() {
        let at = if axis == 0 { i % grid[0] } else { i / grid[0] };
        let knots = if axis == 0 { s.knots_u() } else { s.knots_v() };
        for group in knots.chunk_by(|a, b| a == b) {
            if group[0] > 0. && group[0] < 1. {
                result[at].push((group[0], group.len()));
            }
        }
    }
    for groups in &mut result {
        groups.sort_by(|a, b| a.0.total_cmp(&b.0));
        *groups = groups
            .chunk_by(|a, b| a.0 == b.0)
            .map(|g| (g[0].0, g.iter().map(|t| t.1).max().unwrap()))
            .collect();
    }
    result
}

pub(super) fn apply(
    mut surface: NurbsSurface,
    axis: usize,
    groups: &[(f64, usize)],
) -> Result<NurbsSurface, GeometryError> {
    for &(t, multiplicity) in groups {
        let knots = if axis == 0 {
            surface.knots_u()
        } else {
            surface.knots_v()
        };
        if knots.iter().filter(|&&k| k == t).count() >= multiplicity {
            continue;
        }
        surface = if axis == 0 {
            surface.try_insert_knot_u(t, multiplicity)?
        } else {
            surface.try_insert_knot_v(t, multiplicity)?
        };
    }
    Ok(surface)
}
