//! Assemble selected border cells and any subdivided coplanar neighbors.
use super::*;

#[allow(clippy::too_many_arguments)]
pub(super) fn build(
    brep: &mut Brep,
    face: &BrepFace,
    boundary: &BrepLoop,
    cap: Vec<NurbsCurve>,
    connectors: Vec<NurbsCurve>,
    plan: plan::Plan,
    tolerance: Tolerance,
    work: &mut Work,
) -> Result<Boundary, GeometryError> {
    let plan::Plan {
        graph,
        cells,
        main,
        sources,
        neighbors,
    } = plan;
    let region = &graph.regions[main];
    let main_cycle = if boundary.loop_type == BrepLoopType::Outer {
        if !region.holes.is_empty() {
            return Err(invalid("inset cap cell contains additional holes"));
        }
        &region.outer
    } else {
        if region.holes.len() != 1 {
            return Err(invalid("inset hole cap has multiple boundary components"));
        }
        &region.holes[0]
    };
    let mut active = vec![false; graph.edges.len()];
    for &h in main_cycle {
        active[h / 2] = true;
    }
    let extra_cells = neighbors.iter().flat_map(|n| n.cells.iter().copied());
    for i in cells
        .iter()
        .copied()
        .filter(|&i| i != main)
        .chain(extra_cells)
    {
        let r = &graph.regions[i];
        for &h in r.outer.iter().chain(r.holes.iter().flatten()) {
            active[h / 2] = true;
        }
    }
    let topology = topology::Topology::build(brep, &graph, &sources, &active, tolerance, work)?;
    let inset = topology.cycle(main_cycle, &face.surface, false, None)?;
    // The caller already charged one border per cap section before graph work.
    work.charge_extra_strips(
        &face.surface,
        cells.len().saturating_sub(1).saturating_sub(cap.len()),
    )?;
    let borders = cells
        .into_iter()
        .filter(|&i| i != main)
        .map(|i| topology.face(i, face, false, None))
        .collect::<Result<Vec<_>, _>>()?;
    for neighbor in neighbors {
        work.charge_extra_strips(
            &brep.faces[neighbor.index].surface,
            neighbor.cells.len() - 1,
        )?;
        let original = brep.faces[neighbor.index].clone();
        let mut pullback =
            topology::Pullback::new(&original.surface, graph.edges.len(), tolerance)?;
        let mut replacement = None;
        let mut extra = Vec::new();
        for i in neighbor.cells {
            let piece = topology.face(i, &original, neighbor.flip, Some(&mut pullback))?;
            if i == neighbor.main {
                replacement = Some(piece);
            } else {
                extra.push(piece);
            }
        }
        work.source_edges.remove_face(brep, neighbor.index);
        brep.faces[neighbor.index] =
            replacement.ok_or_else(|| invalid("inset neighbor has no retained face"))?;
        let first = brep.faces.len();
        work.source_edges.append_faces(neighbor.index, extra.len());
        brep.faces.extend(extra);
        work.source_edges.add_faces(
            brep,
            std::iter::once(neighbor.index).chain(first..brep.faces.len()),
        );
    }
    if work.keep_curves {
        let cap = PolyCurve3::try_new(
            cap.into_iter()
                .map(CurveSegment3::NurbsCurve)
                .collect::<Vec<_>>(),
        )?
        .to_nurbs()?;
        work.charge_curve(&cap)?;
        work.network_curves.push(
            NetworkCurves {
                face: work.face,
                boundary: work.boundary,
                cap,
                connectors,
                edges: topology.produced,
            },
            brep.edges.len(),
        );
    }
    Ok(Boundary { inset, borders })
}
