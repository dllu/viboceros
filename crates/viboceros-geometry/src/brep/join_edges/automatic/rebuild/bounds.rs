//! Recompute proven boundary-image uncertainty; otherwise propagate it.
use super::*;

pub(super) fn update(
    source: &Brep,
    result: &mut Brep,
    tolerance: Tolerance,
    budget: &mut Budget,
) -> Result<(), GeometryError> {
    let changed_vertices = source
        .vertices
        .iter()
        .zip(&result.vertices)
        .map(|(a, b)| a.point != b.point)
        .collect::<Vec<_>>();
    let changed_edges = source
        .edges
        .iter()
        .zip(&result.edges)
        .map(|(a, b)| a.curve != b.curve)
        .collect::<Vec<_>>();
    let mut vertices = vec![Some(0_f64); source.vertices.len()];
    let mut edges = vec![Some(0_f64); source.edges.len()];
    for usage in source.trim_uses() {
        let trim = usage.trim;
        if !trim.vertices.iter().any(|&v| changed_vertices[v])
            && !trim.edge.is_some_and(|e| changed_edges[e])
        {
            continue;
        }
        let image = image::BoundaryImage::new(&source.faces[usage.face].surface, trim, budget)?;
        if let Some(e) = trim.edge.filter(|&e| changed_edges[e]) {
            let bound = if let Some(image) = &image {
                image.bound(&result.edges[e].curve, trim.reversed_3d, false, budget)?
            } else {
                None
            };
            edges[e] = edges[e].zip(bound).map(|(a, b)| a.max(b));
        }
        for (end, &v) in trim.vertices.iter().enumerate() {
            if !changed_vertices[v] {
                continue;
            }
            let bound = if let Some(image) = &image {
                image.endpoint_bound(result.vertices[v].point, end == 1, budget)?
            } else {
                None
            };
            vertices[v] = vertices[v].zip(bound).map(|(a, b)| a.max(b));
        }
    }
    for (i, vertex) in result.vertices.iter_mut().enumerate() {
        if !changed_vertices[i] {
            continue;
        }
        let bound = if let Some(bound) = vertices[i] {
            bound
        } else {
            let movement =
                certificate::point_bound(source.vertices[i].point, vertex.point, Real::MAX)
                    .ok_or_else(|| invalid("unrepresentable rebuilt vertex uncertainty"))?;
            certificate::add_bound(
                source.vertices[i].tolerance.max(tolerance.absolute()),
                movement,
            )?
        };
        vertex.tolerance = vertex
            .tolerance
            .max(crate::brep::tolerance::scaled_tolerance(bound, 1.001)?);
    }
    for (i, edge) in result.edges.iter_mut().enumerate() {
        if !changed_edges[i] {
            continue;
        }
        let bound = if let Some(bound) = edges[i] {
            bound
        } else {
            let movement =
                certificate::curve_bound(&source.edges[i].curve, &edge.curve, false, Real::MAX)
                    .ok_or_else(|| invalid("rebuilt edge lacks a displacement certificate"))?;
            certificate::add_bound(
                source.edges[i].tolerance.max(tolerance.absolute()),
                movement,
            )?
        };
        edge.tolerance = edge.tolerance.max(bound);
    }
    Ok(())
}
