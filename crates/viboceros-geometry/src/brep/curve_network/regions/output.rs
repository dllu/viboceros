//! Exact boundary walks and archive-valid bridge topology.
use super::*;
use crate::{CurveSegment3, PolyCurve3};

pub(super) fn region(
    surface: &NurbsSurface,
    graph: &SurfaceCutGraph,
    region: &SurfaceCutArrangementRegion,
    tolerance: Tolerance,
) -> Result<PlanarCurveRegion, GeometryError> {
    let boundaries = std::iter::once(&region.outer)
        .chain(&region.holes)
        .map(|cycle| {
            let segments = cycle
                .iter()
                .map(|&h| {
                    let edge = &graph.edges[h / 2];
                    let nodes = surface_cut_halfedge_nodes(edge, h);
                    let curve = if h.is_multiple_of(2) {
                        edge.spatial.clone()
                    } else {
                        edge.spatial.reversed()?
                    };
                    Ok(CurveSegment3::NurbsCurve(curve.try_with_endpoints(
                        Some(graph.nodes[nodes[0]].point),
                        Some(graph.nodes[nodes[1]].point),
                    )?))
                })
                .collect::<Result<Vec<_>, GeometryError>>()?;
            PolyCurve3::try_new(segments)?.to_nurbs()
        })
        .collect::<Result<Vec<_>, _>>()?;
    let repeated = std::iter::once(&region.outer)
        .chain(&region.holes)
        .any(|cycle| {
            let mut nodes = BTreeSet::new();
            cycle
                .iter()
                .any(|&h| !nodes.insert(surface_cut_halfedge_nodes(&graph.edges[h / 2], h)[0]))
        });
    let brep = if repeated {
        expanded_face(surface, graph, region, tolerance)?
    } else {
        try_surface_cut_arrangement_face(
            surface.clone(),
            false,
            &graph.nodes,
            &graph.edges,
            region,
            &graph.source_info,
            tolerance,
        )?
    };
    Ok(PlanarCurveRegion { boundaries, brep })
}

/// A bridge has two distinct boundary uses. An interior bridge is not a
/// periodic-surface seam: duplicating its vertices/edges keeps the walk valid
/// in both the Rust kernel and OpenNURBS, including after 3DM serialization.
fn expanded_face(
    surface: &NurbsSurface,
    graph: &SurfaceCutGraph,
    region: &SurfaceCutArrangementRegion,
    tolerance: Tolerance,
) -> Result<Brep, GeometryError> {
    let mut vertices = Vec::new();
    let mut edges = Vec::new();
    let mut loops = Vec::new();
    for (i, cycle) in std::iter::once(&region.outer)
        .chain(&region.holes)
        .enumerate()
    {
        let base = vertices.len();
        let mut trims = Vec::new();
        for &h in cycle {
            vertices.push(BrepVertex::try_new(
                graph.nodes[surface_cut_halfedge_nodes(&graph.edges[h / 2], h)[0]].point,
                0.,
            )?);
        }
        for (j, &h) in cycle.iter().enumerate() {
            let edge = &graph.edges[h / 2];
            let nodes = surface_cut_halfedge_nodes(edge, h);
            let (spatial, parameter) = if h.is_multiple_of(2) {
                (edge.spatial.clone(), edge.parameter.clone())
            } else {
                (edge.spatial.reversed()?, edge.parameter.reversed()?)
            };
            let spatial = spatial.try_with_endpoints(
                Some(graph.nodes[nodes[0]].point),
                Some(graph.nodes[nodes[1]].point),
            )?;
            let pair = [base + j, base + (j + 1) % cycle.len()];
            let id = edges.len();
            edges.push(BrepEdge::try_new(pair, spatial, 0.)?);
            trims.push(BrepTrim::try_new(
                pair,
                Some(id),
                false,
                parameter,
                BrepTrimType::Boundary,
                edge.iso,
                [0.; 2],
            )?);
        }
        loops.push(BrepLoop::try_new(
            if i == 0 {
                BrepLoopType::Outer
            } else {
                BrepLoopType::Inner
            },
            trims,
        )?);
    }
    Brep::try_new(
        vertices,
        edges,
        vec![BrepFace::try_new(surface.clone(), false, loops)?],
        tolerance,
    )
}
