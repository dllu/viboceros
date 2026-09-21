//! Recompute proven natural-boundary uncertainty; otherwise propagate it.
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
        let image = natural_image(&source.faces[usage.face].surface, trim, budget)?;
        if let Some(e) = trim.edge.filter(|&e| changed_edges[e]) {
            let bound = image.as_ref().and_then(|(curve, backwards)| {
                certificate::curve_bound(
                    &result.edges[e].curve,
                    curve,
                    backwards ^ trim.reversed_3d,
                    Real::MAX,
                )
            });
            edges[e] = edges[e].zip(bound).map(|(a, b)| a.max(b));
        }
        for (end, &v) in trim.vertices.iter().enumerate() {
            if !changed_vertices[v] {
                continue;
            }
            let bound = image.as_ref().and_then(|(curve, backwards)| {
                let controls = curve.control_points();
                let index = if (end == 1) ^ backwards {
                    controls.len() - 1
                } else {
                    0
                };
                certificate::point_bound(
                    result.vertices[v].point,
                    controls[index].point(),
                    Real::MAX,
                )
            });
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

/// A complete natural isocurve is an exact row/column of a clamped surface.
/// Do not use rounded general isocurve extraction as an exact certificate.
pub(super) fn natural_image(
    surface: &NurbsSurface,
    trim: &BrepTrim,
    budget: &mut Budget,
) -> Result<Option<(NurbsCurve, bool)>, GeometryError> {
    budget.charge(trim.curve.control_points().len())?;
    let uv = crate::brep::trim_image::LiftedTrim::new(trim, surface)?.curve;
    let Some([start, end]) = certificate::linear_endpoints(&uv) else {
        return Ok(None);
    };
    let [a, b] = [start, end].map(Point3::to_array);
    let varying = if a[1] == b[1] {
        0
    } else if a[0] == b[0] {
        1
    } else {
        return Ok(None);
    };
    let domains = [surface.domain_u(), surface.domain_v()];
    let knots = [surface.knots_u(), surface.knots_v()];
    let degree = [surface.degree_u(), surface.degree_v()];
    let counts = [
        surface.control_point_count_u(),
        surface.control_point_count_v(),
    ];
    let fixed = 1 - varying;
    let row = if a[fixed] == *domains[fixed].start()
        && knots[fixed][..=degree[fixed]]
            .iter()
            .all(|k| *k == a[fixed])
    {
        0
    } else if a[fixed] == *domains[fixed].end()
        && knots[fixed][knots[fixed].len() - degree[fixed] - 1..]
            .iter()
            .all(|k| *k == a[fixed])
    {
        counts[fixed] - 1
    } else {
        return Ok(None);
    };
    if a[varying].min(b[varying]) != *domains[varying].start()
        || a[varying].max(b[varying]) != *domains[varying].end()
    {
        return Ok(None);
    }
    budget.charge(counts[varying])?;
    let curve = NurbsCurve::try_new_rational(
        degree[varying],
        (0..counts[varying])
            .map(|i| {
                if varying == 0 {
                    surface.control_point(i, row).unwrap()
                } else {
                    surface.control_point(row, i).unwrap()
                }
            })
            .collect(),
        knots[varying].to_vec(),
    )?;
    if !clamped(&curve) {
        return Ok(None);
    }
    Ok(Some((curve, a[varying] > b[varying])))
}
