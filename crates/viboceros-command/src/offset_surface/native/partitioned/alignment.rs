//! Knot insertion gives corresponding fitted boundaries a common basis.
use super::*;
use std::collections::BTreeMap;
use viboceros_geometry::SurfaceKnotDirection;
#[cfg(test)]
mod tests;

pub(super) fn supports(
    fitted: &mut [NurbsSurface],
    sources: &[&NurbsSurface],
    station_budget: usize,
) -> Result<(), CommandError> {
    let mut targets = fitted
        .iter()
        .map(|s| [s.knots_u().to_vec(), s.knots_v().to_vec()])
        .collect::<Vec<_>>();
    for axis in [0, 1] {
        let mut groups = BTreeMap::<[u64; 2], Vec<usize>>::new();
        for (i, s) in fitted.iter().enumerate() {
            let domain = if axis == 0 {
                s.domain_u()
            } else {
                s.domain_v()
            };
            groups
                .entry([domain.start().to_bits(), domain.end().to_bits()])
                .or_default()
                .push(i);
        }
        for indices in groups.values().filter(|indices| indices.len() > 1) {
            let degree = |i: usize| {
                if axis == 0 {
                    fitted[i].degree_u()
                } else {
                    fitted[i].degree_v()
                }
            };
            if !indices.iter().all(|&i| degree(i) == degree(indices[0])) {
                continue;
            }
            let mut knots = indices
                .iter()
                .flat_map(|&i| {
                    targets[i][axis]
                        .chunk_by(|a, b| a == b)
                        .map(|g| (g[0], g.len()))
                })
                .collect::<Vec<_>>();
            knots.sort_by(|a, b| a.0.total_cmp(&b.0));
            let union = knots
                .chunk_by(|a, b| a.0 == b.0)
                .flat_map(|g| std::iter::repeat_n(g[0].0, g.iter().map(|v| v.1).max().unwrap()))
                .collect::<Vec<_>>();
            for &i in indices {
                targets[i][axis] = union.clone();
            }
        }
    }
    // Preflight the existing field budget before expanding any support.
    let mut remaining = station_budget;
    for (source, target) in sources.iter().zip(&targets) {
        let axes = station_axes(source, &target[0], &target[1]);
        remaining = axes[0]
            .len()
            .checked_mul(axes[1].len())
            .and_then(|n| remaining.checked_sub(n))
            .ok_or_else(|| failure("Offset fit exceeds its station budget"))?;
    }
    for (surface, target) in fitted.iter_mut().zip(targets) {
        for (axis, knots) in target.into_iter().enumerate() {
            let direction = if axis == 0 {
                SurfaceKnotDirection::U
            } else {
                SurfaceKnotDirection::V
            };
            let domain = if axis == 0 {
                surface.domain_u()
            } else {
                surface.domain_v()
            };
            for group in knots.chunk_by(|a, b| a == b) {
                let parameter = group[0];
                if parameter <= *domain.start() || parameter >= *domain.end() {
                    continue;
                }
                let current = if axis == 0 {
                    surface.knots_u()
                } else {
                    surface.knots_v()
                };
                let count = current.partition_point(|&k| k <= parameter)
                    - current.partition_point(|&k| k < parameter);
                if count < group.len() {
                    *surface = viboceros_smlib::insert_surface_knot(
                        surface,
                        direction,
                        parameter,
                        group.len(),
                    )?;
                }
            }
            let current = if axis == 0 {
                surface.knots_u()
            } else {
                surface.knots_v()
            };
            if current != knots {
                return Err(failure("Offset support refinement changed its knot clock"));
            }
        }
    }
    Ok(())
}
