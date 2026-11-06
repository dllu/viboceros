//! Exterior walks fill bounded network regions, retaining exact spatial curves.
use super::*;
use crate::{CurveSegment3, PolyCurve3};
mod certificates;
mod contacts;
pub(super) use certificates::only_shared_ends;
#[cfg(test)]
mod tests;
pub(super) use contacts::{line_contacts, same_line, sort_line_rays};

/// Fill all bounded regions of a planar curve network. Dangling branches and
/// bridges contribute no area; nested disconnected outlines are covered once.
/// Curves on other planes are ignored span by span. Endpoint reconciliation
/// is bounded by model tolerance. Surface-network storage and pair-work limits
/// apply before construction, with at most 128 exterior loops.
pub fn planar_curve_network_boundaries(
    paths: &[NurbsCurve],
    plane: Frame3,
    tolerance: Tolerance,
) -> Result<Vec<NurbsCurve>, GeometryError> {
    let mut curves = Vec::new();
    let mut controls = 0usize;
    let mut bounds = [[Real::INFINITY, Real::NEG_INFINITY]; 2];
    paths.iter().try_fold(0usize, |n, path| {
        let count = path
            .spans()
            .count()
            .saturating_mul(path.degree().saturating_add(1));
        n.checked_add(count)
            .filter(|n| *n <= 100_000)
            .ok_or(GeometryError::CurveJoinLimit {
                resource: "planar network controls",
                maximum: 100_000,
            })
    })?;
    for path in paths {
        for leaf in path.try_bezier_spans()? {
            let coplanar = leaf.control_points().iter().try_fold(true, |yes, p| {
                Ok::<_, GeometryError>(
                    yes && plane.coordinate_of(2, p.point())?.abs() <= tolerance.absolute(),
                )
            })?;
            if !coplanar {
                continue;
            }
            let sign = leaf.control_points()[0].weight().is_sign_positive();
            if leaf
                .control_points()
                .iter()
                .any(|p| p.weight().is_sign_positive() != sign)
            {
                return Err(GeometryError::InvalidPlanarFaceBoundary);
            }
            controls = controls.saturating_add(leaf.control_points().len());
            if controls > 100_000 {
                return Err(GeometryError::CurveJoinLimit {
                    resource: "planar network controls",
                    maximum: 100_000,
                });
            }
            for p in leaf.control_points() {
                for (i, coordinate) in plane
                    .projected_coordinates_of(p.point())?
                    .into_iter()
                    .enumerate()
                {
                    bounds[i][0] = bounds[i][0].min(coordinate);
                    bounds[i][1] = bounds[i][1].max(coordinate);
                }
            }
            curves.push(leaf);
        }
    }
    if curves.is_empty() {
        return Ok(Vec::new());
    }
    let span = (bounds[0][1] - bounds[0][0]).max(bounds[1][1] - bounds[1][0]);
    let padding = span.max(tolerance.absolute() * 64.) * 0.25;
    for range in &mut bounds {
        range[0] -= padding;
        range[1] += padding;
    }
    let corners = [
        [bounds[0][0], bounds[1][0]],
        [bounds[0][1], bounds[1][0]],
        [bounds[0][1], bounds[1][1]],
        [bounds[0][0], bounds[1][1]],
    ];
    let points = corners
        .map(|p| plane.point_at([p[0], p[1], 0.]))
        .into_iter()
        .collect::<Result<Vec<_>, _>>()?;
    let surface = NurbsSurface::try_bilinear(points.try_into().unwrap())?
        .try_reparameterized(bounds[0][0]..=bounds[0][1], bounds[1][0]..=bounds[1][1])?;
    let bytes = controls.saturating_mul(192).saturating_add(
        curves
            .iter()
            .map(|c| c.knots().len().saturating_mul(48))
            .sum::<usize>(),
    );
    let maximum = (STORAGE_LIMIT.saturating_sub(bytes) / 8192).min(crate::MAX_SURFACE_WIRES);
    if maximum < 4 || curves.len() > maximum {
        return invalid("a planar curve network exceeds its storage estimate limit");
    }
    let graph = graph::build_mode(
        &surface,
        bounds,
        curves,
        tolerance,
        graph::Mode::Filled,
        maximum,
        |_| Some(tolerance.angular()),
    )?;
    let mut boundaries = Vec::new();
    let mut output_controls = 0usize;
    let mut areas = Vec::new();
    let mut hulls = Vec::new();
    let mut witnesses = Vec::new();
    for region in &graph.regions {
        let degree = region
            .outer
            .iter()
            .map(|h| graph.edges[h / 2].spatial.degree())
            .max()
            .unwrap();
        output_controls =
            output_controls.saturating_add(region.outer.len().saturating_mul(degree + 1));
        if output_controls > 100_000 {
            return Err(GeometryError::CurveJoinLimit {
                resource: "planar network controls",
                maximum: 100_000,
            });
        }
        let mut segments = Vec::new();
        for &halfedge in &region.outer {
            let edge = &graph.edges[halfedge / 2];
            let nodes = surface_cut_halfedge_nodes(edge, halfedge);
            let c = if halfedge.is_multiple_of(2) {
                edge.spatial.clone()
            } else {
                edge.spatial.reversed()?
            };
            let c = c.try_with_endpoints(
                Some(graph.nodes[nodes[0]].point),
                Some(graph.nodes[nodes[1]].point),
            )?;
            segments.push(CurveSegment3::NurbsCurve(c));
        }
        let boundary = PolyCurve3::try_new(segments)?.to_nurbs()?;
        let face = Brep::try_planar_face(&boundary, tolerance)?;
        areas.push(face.area(tolerance)?);
        hulls.push(boundary.control_point_bounds());
        witnesses.push(
            boundary
                .spans()
                .map(|(lo, _)| boundary.evaluate(lo))
                .collect::<Result<Vec<_>, _>>()?,
        );
        boundaries.push(boundary);
    }
    let mut retained = Vec::new();
    for (i, boundary) in boundaries.iter().enumerate() {
        let mut covered = false;
        for (j, outer) in boundaries.iter().enumerate() {
            if i != j
                && areas[j] > areas[i]
                && witnesses[i].iter().all(|p| {
                    p.to_array()
                        .into_iter()
                        .zip(hulls[j].min().to_array())
                        .zip(hulls[j].max().to_array())
                        .all(|((x, lo), hi)| {
                            x >= lo - tolerance.absolute() && x <= hi + tolerance.absolute()
                        })
                })
            {
                match Brep::try_planar_face_with_holes(
                    outer,
                    std::slice::from_ref(boundary),
                    tolerance,
                ) {
                    Ok(_) => {
                        covered = true;
                        break;
                    }
                    Err(GeometryError::InvalidPlanarFaceBoundary) => {}
                    Err(error) => return Err(error),
                }
            }
        }
        if !covered {
            retained.push(boundary.clone());
        }
    }
    Ok(retained)
}

pub(super) fn from_cycles(
    nodes: Vec<SurfaceCutArrangementNode>,
    edges: Vec<SurfaceCutArrangementEdge>,
    cycles: Vec<Vec<usize>>,
) -> Result<SurfaceCutGraph, GeometryError> {
    let mut regions = Vec::new();
    for cycle in cycles {
        if cycle.iter().any(|&h| {
            matches!(
                edges[h / 2].kind,
                SurfaceCutArrangementEdgeKind::Boundary(_)
            )
        }) {
            continue;
        }
        let mut stack = Vec::<usize>::new();
        let mut positions = BTreeMap::new();
        let first = surface_cut_halfedge_nodes(&edges[cycle[0] / 2], cycle[0])[0];
        positions.insert(first, 0usize);
        for h in cycle {
            stack.push(h);
            let end = surface_cut_halfedge_nodes(&edges[h / 2], h)[1];
            if let Some(&i) = positions.get(&end) {
                let walk = stack[i..].to_vec();
                if sampled_surface_cut_cycle_signed_area(&edges, &walk)?
                    .is_some_and(|area| area < 0.)
                {
                    if regions.len() == 128 {
                        return Err(GeometryError::CurveJoinLimit {
                            resource: "planar network boundaries",
                            maximum: 128,
                        });
                    }
                    regions.push(SurfaceCutArrangementRegion {
                        outer: walk.into_iter().rev().map(|h| h ^ 1).collect(),
                        holes: Vec::new(),
                    });
                }
                for &halfedge in &stack[i..] {
                    let end = surface_cut_halfedge_nodes(&edges[halfedge / 2], halfedge)[1];
                    positions.remove(&end);
                }
                stack.truncate(i);
                positions.insert(end, i);
            } else {
                positions.insert(end, stack.len());
            }
        }
    }
    Ok(SurfaceCutGraph {
        nodes,
        edges,
        regions,
        source_breaks: Vec::new(),
        source_info: SurfaceCutArrangementSourceInfo {
            node_rank: Vec::new(),
            first_closed: None,
            closed: Vec::new(),
            first_nodes: Vec::new(),
            first_tangent_nodes: Vec::new(),
            first_overlap_nodes: Vec::new(),
            first_opposite_overlap_nodes: Vec::new(),
            first_boundary_overlap_edges: Vec::new(),
            first_maximum_segment: 0,
        },
    })
}
