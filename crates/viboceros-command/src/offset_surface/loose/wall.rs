//! Boundary correspondence is established by the retained UV trim order.
use super::*;
use viboceros_geometry::{GeometryError, join_breps};

fn invalid(context: &'static str) -> CommandError {
    GeometryError::UnsupportedLooseSurfaceOffset { context }.into()
}

// The retained source rim uses piecewise-Bezier endpoint weight normalization;
// the new offset rim keeps the underlying surface's original weights. Their
// different parameter speeds affect the intervening ruled surface.
pub(super) fn normalized_source_boundaries(
    source: &Brep,
    tolerance: Tolerance,
) -> Result<Brep, CommandError> {
    let mut edges = Vec::with_capacity(source.edges().len());
    for edge in source.edges() {
        let c = edge.curve();
        let first = c.control_points()[0].weight();
        let last = c.control_points().last().unwrap().weight();
        let curve = if first == 1. && last == 1. {
            c.clone()
        } else {
            c.try_trimmed_with_normalized_end_weights(c.domain())?
        };
        edges.push(viboceros_geometry::BrepEdge::try_new(
            edge.vertices(),
            curve,
            edge.tolerance(),
        )?);
    }
    Ok(Brep::try_new(
        source.vertices().to_vec(),
        edges,
        source.faces().to_vec(),
        tolerance,
    )?)
}

pub(super) fn assemble(a: &Brep, b: &Brep, tolerance: Tolerance) -> Result<Brep, CommandError> {
    let mut pieces = vec![a.clone(), b.reversed()];
    if a.is_solid() && b.is_solid() {
        let shell = Brep::try_disjoint_union(pieces, tolerance)?;
        return Ok(
            if shell.solid_orientation()? == viboceros_geometry::BrepSolidOrientation::Inward {
                shell.reversed()
            } else {
                shell
            },
        );
    }
    let af = &a.faces()[0];
    let bf = &b.faces()[0];
    if af.loops().len() != bf.loops().len() {
        return Err(invalid("offset loop correspondence changed"));
    }
    for (al, bl) in af.loops().iter().zip(bf.loops()) {
        if al.trims().len() != bl.trims().len() {
            return Err(invalid("offset trim correspondence changed"));
        }
        for (at, bt) in al.trims().iter().zip(bl.trims()) {
            if at.trim_type() == viboceros_geometry::BrepTrimType::Seam {
                continue;
            }
            let (Some(ae), Some(be)) = (at.edge(), bt.edge()) else {
                if at.edge().is_none() && bt.edge().is_none() {
                    continue;
                }
                return Err(invalid("a singular trim opened during offset"));
            };
            let curve =
                |brep: &Brep, index: usize, reversed: bool| -> Result<NurbsCurve, CommandError> {
                    let c = brep.edges()[index].curve();
                    Ok(if reversed { c.reversed()? } else { c.clone() })
                };
            let ac = curve(a, ae, at.is_reversed_3d())?;
            let bc = curve(b, be, bt.is_reversed_3d())?;
            pieces.push(Brep::try_surface_face(ruled(&ac, &bc)?, tolerance)?);
        }
    }
    let mut joined = join_breps(
        &pieces.iter().collect::<Vec<_>>(),
        tolerance.absolute(),
        tolerance,
    )?;
    if joined.len() != 1 || !joined[0].brep.is_solid() {
        return Err(invalid(
            "ruled walls could not form one certified closed shell",
        ));
    }
    Ok(joined.remove(0).brep)
}

fn ruled(a: &NurbsCurve, b: &NurbsCurve) -> Result<NurbsSurface, CommandError> {
    Ok(NurbsSurface::try_ruled_from_curves(a, b)?.try_swapped_uv()?)
}
