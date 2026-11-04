//! Dyadic soft-crease spans with localized extraordinary-corner approximation.
use super::*;
mod levels;
pub(super) use levels::continuity_enabled;

fn failure(context: &'static str) -> GeometryError {
    GeometryError::InvalidBrepTopology { context }
}

fn refined(mut surface: NurbsSurface, side: usize) -> Result<NurbsSurface, GeometryError> {
    for axis in 0..2 {
        for i in 1..side {
            let t = i as f64 / side as f64;
            let knots = if axis == 0 {
                surface.knots_u()
            } else {
                surface.knots_v()
            };
            if !knots.contains(&t) {
                surface = if axis == 0 {
                    surface.try_insert_knot_u(t, 1)?
                } else {
                    surface.try_insert_knot_v(t, 1)?
                };
            }
        }
    }
    Ok(surface)
}

pub(super) fn build(source: &SubD) -> Result<Vec<SubDNurbsPatch>, GeometryError> {
    let sharp = source
        .edge_sharpness()
        .iter()
        .filter(|ends| ends.iter().any(|&s| s > 0.))
        .collect::<Vec<_>>();
    if sharp.len() != 1 || sharp[0][0] != sharp[0][1] {
        return Err(failure(
            "Multiple or variable-end soft creases require further NURBS qualification",
        ));
    }
    let levels = levels::corners(source)?;
    let highest = *levels.iter().flatten().max().unwrap_or(&3);
    let mut exact = source.regular_nurbs_patches()?;
    let mut fine = viboceros_opennurbs::subd_patch_stencils(source.archive(), highest)?;
    let mut surfaces = fine
        .iter()
        .map(stencil::surface)
        .collect::<Result<Vec<_>, _>>()?;
    for level in 3..highest {
        let coarse = viboceros_opennurbs::subd_partial_patch_stencils(source.archive(), level)?;
        if coarse.len() != fine.len() {
            return Err(failure("Inconsistent soft SubD stencil levels"));
        }
        for (index, partial) in coarse.iter().enumerate() {
            let net = &partial.stencil;
            let high = &fine[index];
            if (net.face, net.corner) != (high.face, high.corner) {
                return Err(failure("Inconsistent soft SubD stencil lineage"));
            }
            let desired = if let Some(corner) = net.corner {
                [levels[net.face][corner], highest, 3, highest]
            } else {
                std::array::from_fn(|i| levels[net.face][i])
            };
            if !desired.contains(&level) {
                continue;
            }
            let old = refined(stencil::surface(net)?, high.side)?;
            let surface = &surfaces[index];
            let n = surface.control_point_count_u();
            if old.knots_u() != surface.knots_u() || old.knots_v() != surface.knots_v() {
                return Err(failure("Inconsistent soft SubD tensor basis"));
            }
            let mut points = surface
                .control_points()
                .iter()
                .map(|p| p.point())
                .collect::<Vec<_>>();
            for (corner, &desired) in desired.iter().enumerate() {
                if desired != level {
                    continue;
                }
                if partial.unresolved_corners[corner] {
                    return Err(failure("Unresolved soft SubD corner neighborhood"));
                }
                let reverse_u = corner == 1 || corner == 2;
                let reverse_v = corner >= 2;
                let radius = 2. / net.side as f64;
                let contained = |i: usize, reverse: bool, knots: &[f64]| {
                    if reverse {
                        knots[i] >= 1. - radius
                    } else {
                        knots[i + 4] <= radius
                    }
                };
                for v in 0..n {
                    for u in 0..n {
                        if contained(u, reverse_u, surface.knots_u())
                            && contained(v, reverse_v, surface.knots_v())
                        {
                            points[v * n + u] = old.control_points()[v * n + u].point();
                        }
                    }
                }
            }
            surfaces[index] = NurbsSurface::try_new(
                3,
                3,
                n,
                n,
                points,
                surface.knots_u().to_vec(),
                surface.knots_v().to_vec(),
            )?;
        }
    }
    fine.drain(..)
        .zip(surfaces)
        .map(|(net, mut surface)| {
            if net.corner.is_none()
                && let Some(regular) = exact[net.face].take()
            {
                surface = regular;
            }
            // These baseline breaks define the native continuity processing
            // neighborhood, even when one happens to be geometrically redundant.
            let base = if net.corner.is_some() { 4 } else { 8 };
            let baseline = (1..base)
                .map(|i| i as f64 / base as f64)
                .collect::<Vec<_>>();
            let mut protected = [baseline.clone(), baseline];
            let desired = if let Some(corner) = net.corner {
                [levels[net.face][corner], highest, 3, highest]
            } else {
                std::array::from_fn(|i| levels[net.face][i])
            };
            for (corner, level) in desired.into_iter().enumerate() {
                for level in 4..=level {
                    let side = 1usize << (level - u32::from(net.corner.is_some()));
                    for (axis, reverse) in [corner == 1 || corner == 2, corner >= 2]
                        .into_iter()
                        .enumerate()
                    {
                        protected[axis].extend((1..=4).map(|i| {
                            let t = i as f64 / side as f64;
                            if reverse { 1. - t } else { t }
                        }));
                    }
                }
            }
            let surface =
                polynomial_cleanup::compact_protected(surface, [&protected[0], &protected[1]])?;
            Ok(SubDNurbsPatch {
                face: net.face,
                corner: net.corner,
                surface,
            })
        })
        .collect()
}
