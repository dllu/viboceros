//! Exact tensor concatenation after local orientation and common-basis refinement.
use super::*;
use std::collections::BTreeMap;

pub(super) fn oriented(surface: NurbsSurface, turns: u8) -> Result<NurbsSurface, GeometryError> {
    let surface = match turns {
        0 => return Ok(surface),
        1 => surface.try_swapped_uv()?.try_reversed_u()?,
        2 => surface.try_reversed_u()?.try_reversed_v()?,
        3 => surface.try_swapped_uv()?.try_reversed_v()?,
        _ => return Err(failure("Invalid SubD packing chart rotation")),
    };
    surface.try_reparameterized(0. ..=1., 0. ..=1.)
}

pub(super) fn surface(
    plan: &Plan,
    patches: &mut BTreeMap<(usize, Option<usize>), NurbsSurface>,
    budget: usize,
) -> Result<NurbsSurface, GeometryError> {
    let mut cells = Vec::with_capacity(plan.tiles.len());
    for tile in &plan.tiles {
        let surface = patches
            .remove(&(tile.face, tile.corner))
            .ok_or_else(|| failure("Missing SubD packing tile"))?;
        cells.push(oriented(surface, tile.quarter_turns)?);
    }
    if cells.len() == 1 {
        let surface = cells.pop().unwrap();
        if surface.control_points().len() > budget {
            return Err(failure("Packed SubD control point budget exceeded"));
        }
        return Ok(surface);
    }
    cells = cells
        .into_iter()
        .map(refine::compact)
        .collect::<Result<Vec<_>, _>>()?;
    let du = cells.iter().map(NurbsSurface::degree_u).max().unwrap();
    let dv = cells.iter().map(NurbsSurface::degree_v).max().unwrap();
    for cell in &mut cells {
        if cell.degree_u() != du || cell.degree_v() != dv {
            *cell = cell.try_change_degree(du, dv, false)?;
        }
    }
    let ug = refine::groups(&cells, plan.grid_size, 0);
    let vg = refine::groups(&cells, plan.grid_size, 1);
    let counts = |groups: &[Vec<(f64, usize)>], degree| {
        groups
            .iter()
            .map(|g| degree + 1 + g.iter().map(|t| t.1).sum::<usize>())
            .collect::<Vec<_>>()
    };
    let uc = counts(&ug, du);
    let vc = counts(&vg, dv);
    let offsets = |counts: &[usize]| {
        std::iter::once(0)
            .chain(counts.iter().scan(0, |sum, &n| {
                *sum += n - 1;
                Some(*sum)
            }))
            .collect::<Vec<_>>()
    };
    let uo = offsets(&uc);
    let vo = offsets(&vc);
    let nu = uo.last().unwrap() + 1;
    let nv = vo.last().unwrap() + 1;
    if nu * nv > budget {
        return Err(failure("Packed SubD control point budget exceeded"));
    }
    // Admit the complete tensor before allocating the common-basis cells.
    cells = cells
        .into_iter()
        .enumerate()
        .map(|(i, cell)| {
            let cell = refine::apply(cell, 0, &ug[i % plan.grid_size[0]])?;
            refine::apply(cell, 1, &vg[i / plan.grid_size[0]])
        })
        .collect::<Result<Vec<_>, GeometryError>>()?;
    let mut controls: Vec<Option<Point3>> = vec![None; nu * nv];
    for (i, cell) in cells.iter().enumerate() {
        let (x, y) = (i % plan.grid_size[0], i / plan.grid_size[0]);
        if (cell.control_point_count_u(), cell.control_point_count_v()) != (uc[x], vc[y]) {
            return Err(failure("Inconsistent SubD packing basis"));
        }
        for v in 0..vc[y] {
            for u in 0..uc[x] {
                let p = cell.control_points()[v * uc[x] + u];
                if (p.weight() - 1.).abs() > 1024. * f64::EPSILON {
                    return Err(failure("SubD packing requires polynomial patches"));
                }
                let at = (vo[y] + v) * nu + uo[x] + u;
                if let Some(old) = controls[at] {
                    let scale = old
                        .to_array()
                        .into_iter()
                        .chain(p.point().to_array())
                        .map(f64::abs)
                        .fold(1., f64::max);
                    if old.distance_to(p.point())? > 2048. * f64::EPSILON * scale {
                        return Err(failure("Inconsistent shared SubD packing control"));
                    }
                } else {
                    controls[at] = Some(p.point());
                }
            }
        }
    }
    let controls = controls
        .into_iter()
        .map(|p| p.ok_or_else(|| failure("Incomplete SubD packing control net")))
        .collect::<Result<Vec<_>, _>>()?;
    let knots = |groups: &[Vec<(f64, usize)>], degree| {
        let mut knots = vec![0.; degree + 1];
        for (i, group) in groups.iter().enumerate() {
            for &(t, n) in group {
                knots.extend(std::iter::repeat_n((i as f64 + t) / groups.len() as f64, n));
            }
            if i + 1 < groups.len() {
                knots.extend(std::iter::repeat_n(
                    (i + 1) as f64 / groups.len() as f64,
                    degree,
                ));
            }
        }
        knots.extend(std::iter::repeat_n(1., degree + 1));
        knots
    };
    NurbsSurface::try_new(du, dv, nu, nv, controls, knots(&ug, du), knots(&vg, dv))
}
