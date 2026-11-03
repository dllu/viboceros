//! Common planar cells for the selected boundary and coplanar neighbors.
use super::*;

pub(super) struct Partition {
    pub index: usize,
    pub flip: bool,
    pub cells: Vec<usize>,
    pub main: usize,
}

pub(super) struct Plan {
    pub graph: SurfaceCutGraph,
    pub cells: Vec<usize>,
    pub main: usize,
    pub sources: Vec<Option<input::Source>>,
    pub neighbors: Vec<Partition>,
}

pub(super) fn plan(
    brep: &Brep,
    face: &BrepFace,
    boundary: &BrepLoop,
    cap: &[NurbsCurve],
    connectors: &[NurbsCurve],
    tolerance: Tolerance,
    work: &mut Work,
) -> Result<Plan, GeometryError> {
    let input::Input {
        curves,
        sources,
        neighbors,
        cap,
    } = input::collect(brep, face, boundary, cap, connectors, tolerance, work)?;
    let (surface, bounds) = input::padded_surface(&face.surface, &curves, tolerance)?;
    let graph = surface_cut_graph(&surface, bounds, curves, tolerance, true, 8192, |i| {
        sources[i].is_none().then_some(std::f64::consts::PI / 180.)
    })?;
    if graph.source_info.closed.len() != sources.len() {
        return Err(invalid(
            "inset network contains duplicate complete source curves",
        ));
    }
    let mut left = vec![None; graph.edges.len() * 2];
    for (i, region) in graph.regions.iter().enumerate() {
        for &h in region.outer.iter().chain(region.holes.iter().flatten()) {
            if left[h].replace(i).is_some() {
                return Err(invalid("inset network cell incidence is inconsistent"));
            }
        }
    }
    let owners = regions::owners(&graph, &left, &sources)?;
    let cap = regions::cap(&graph, &left, cap)?;
    let mut by_face = BTreeMap::<usize, Vec<usize>>::new();
    for (i, owner) in owners.into_iter().enumerate() {
        if let Some(face) = owner {
            by_face.entry(face).or_default().push(i);
        }
    }
    let cells = by_face
        .remove(&work.face)
        .ok_or_else(|| invalid("inset network has no source material"))?;
    let retained = cells
        .iter()
        .copied()
        .filter(|&i| cap[i])
        .collect::<Vec<_>>();
    let main = largest(&graph, &retained)?;
    let mut partitions = Vec::new();
    for neighbor in neighbors {
        let cells = by_face
            .remove(&neighbor.index)
            .ok_or_else(|| invalid("an inset neighbor has no material cells"))?;
        if cells.len() > 1 {
            let main = largest(&graph, &cells)?;
            partitions.push(Partition {
                index: neighbor.index,
                flip: neighbor.flip,
                cells,
                main,
            });
        }
    }
    Ok(Plan {
        graph,
        cells,
        main,
        sources,
        neighbors: partitions,
    })
}

fn largest(graph: &SurfaceCutGraph, cells: &[usize]) -> Result<usize, GeometryError> {
    cells
        .iter()
        .copied()
        .map(|i| {
            let region = &graph.regions[i];
            let mut area =
                sampled_surface_cut_cycle_signed_area(&graph.edges, &region.outer)?.unwrap_or(0.);
            for hole in &region.holes {
                area += sampled_surface_cut_cycle_signed_area(&graph.edges, hole)?.unwrap_or(0.);
            }
            Ok((i, area))
        })
        .collect::<Result<Vec<_>, GeometryError>>()?
        .into_iter()
        .max_by(|a, b| a.1.total_cmp(&b.1))
        .map(|(i, _)| i)
        .ok_or_else(|| invalid("inset network has no retained cap cell"))
}
