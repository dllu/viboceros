//! Exact rectangular surface arrangements, including interior cutter junctions.

use super::*;
mod edges;
mod graph;
mod index;
pub(super) use graph::build;

#[cfg(test)]
mod tests;

const STORAGE_LIMIT: usize = 64 * 1024 * 1024;

pub(super) fn split(
    surface: NurbsSurface,
    u: RangeInclusive<Real>,
    v: RangeInclusive<Real>,
    cut_curves: impl IntoIterator<Item = NurbsCurve>,
    reversed: bool,
    tolerance: Tolerance,
) -> Result<Vec<Brep>, GeometryError> {
    require_finite(
        [*u.start(), *u.end(), *v.start(), *v.end()],
        "rectangular surface cutting network bounds",
    )?;
    let surface_bytes = surface
        .control_points()
        .len()
        .saturating_mul(32)
        .saturating_add(surface.knots_u().len().saturating_mul(8))
        .saturating_add(surface.knots_v().len().saturating_mul(8));
    // Account for retained source curves, restrictions, p-curves, graph indexes,
    // and one surface copy per output face. Check before collecting or cloning.
    let edge_bytes = |degree: usize| {
        surface_bytes
            .saturating_add(degree.saturating_add(1).saturating_mul(192))
            .saturating_add(4096)
    };
    let mut bytes = 0usize;
    let mut degree = 1usize;
    let mut curves = Vec::new();
    for curve in cut_curves {
        degree = degree.max(curve.degree());
        bytes = bytes
            .saturating_add(curve.control_points().len().saturating_mul(192))
            .saturating_add(curve.knots().len().saturating_mul(48));
        if bytes.saturating_add(edge_bytes(degree).saturating_mul(4)) > STORAGE_LIMIT
            || curves.len() >= crate::MAX_SURFACE_WIRES
        {
            return invalid("a surface cutting network exceeds its storage estimate limit");
        }
        curves.push(curve);
    }
    let maximum =
        (STORAGE_LIMIT.saturating_sub(bytes) / edge_bytes(degree)).min(crate::MAX_SURFACE_WIRES);
    if curves.len() > maximum || maximum < 4 {
        return invalid("a surface cutting network exceeds its storage estimate limit");
    }
    surface.try_trimmed(u.clone(), v.clone())?;
    let graph = build(
        &surface,
        [[*u.start(), *u.end()], [*v.start(), *v.end()]],
        curves,
        tolerance,
        true,
        maximum,
        |_| Some(tolerance.angular()),
    )?;
    graph
        .regions
        .iter()
        .map(|region| {
            try_surface_cut_arrangement_face(
                surface.clone(),
                reversed,
                &graph.nodes,
                &graph.edges,
                region,
                &graph.source_info,
                tolerance,
            )
        })
        .collect()
}
