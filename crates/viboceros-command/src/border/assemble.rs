//! Border-specific representation and domain policy. No fitting or gap edits.
use super::*;
use viboceros_geometry::PolyCurve3;

pub(super) fn mesh_boundaries(
    mesh: &TriangleMesh,
    tolerance: Tolerance,
) -> Result<Vec<Geometry>, GeometryError> {
    let key = |p: Point3| p.to_array().map(|c| if c == 0.0 { 0 } else { c.to_bits() });
    let mut vertices = BTreeMap::new();
    for (index, &point) in mesh.vertices().iter().enumerate() {
        vertices.entry(key(point)).or_insert(index);
    }
    mesh.boundary_polylines(tolerance)?
        .into_iter()
        .map(|curve| {
            if !curve.is_closed() {
                return Ok(Geometry::Polyline(curve));
            }
            // Rhino traces the first canonical edge from smaller to larger
            // topology index independently of mesh face winding.
            if vertices[&key(curve.vertices()[0])] > vertices[&key(curve.vertices()[1])] {
                return Ok(Geometry::Polyline(Polyline3::try_new(
                    curve.vertices().iter().rev().copied().collect(),
                    tolerance,
                )?));
            }
            mesh_boundary(curve, tolerance)
        })
        .collect()
}

fn mesh_boundary(curve: Polyline3, tolerance: Tolerance) -> Result<Geometry, GeometryError> {
    if !curve.is_closed() {
        return Ok(Geometry::Polyline(curve));
    }
    // Mesh.GetNakedEdges starts at the larger index of the first naked edge.
    // Its polyline parameters count edges; surface-border parameters do not.
    let mut vertices = curve.vertices()[1..].to_vec();
    vertices.push(vertices[0]);
    Ok(Geometry::Polyline(Polyline3::try_new(vertices, tolerance)?))
}

pub(super) fn surface_components(
    surface: &NurbsSurface,
) -> Result<Vec<Vec<NurbsCurve>>, GeometryError> {
    let mut components = surface.natural_boundary_curve_loops()?;
    for component in &mut components {
        if component.len() > 1 && linear(component) {
            component.rotate_right(1);
        }
    }
    Ok(components)
}

fn linear(curves: &[NurbsCurve]) -> bool {
    curves
        .iter()
        .all(|curve| curve.degree() == 1 && curve.control_points().len() == 2)
}

pub(crate) fn assemble(
    curves: Vec<NurbsCurve>,
    tolerance: Tolerance,
) -> Result<Geometry, GeometryError> {
    if curves.len() == 1 {
        return Ok(Geometry::NurbsCurve(curves.into_iter().next().unwrap()));
    }
    if curves
        .iter()
        .all(|curve| curve.control_points().iter().all(|p| p.weight() > 0.0))
        && curves
            .iter()
            .map(NurbsCurve::is_linear_at_zero_tolerance)
            .collect::<Result<Vec<_>, _>>()?
            .into_iter()
            .all(|linear| linear)
    {
        let endpoints = curves
            .iter()
            .map(|curve| {
                Ok([
                    curve.evaluate(*curve.domain().start())?,
                    curve.evaluate(*curve.domain().end())?,
                ])
            })
            .collect::<Result<Vec<_>, GeometryError>>()?;
        if !endpoints.is_empty() && endpoints.windows(2).all(|pair| pair[0][1] == pair[1][0]) {
            let mut vertices = Vec::with_capacity(endpoints.len() + 1);
            vertices.push(endpoints[0][0]);
            vertices.extend(endpoints.into_iter().map(|points| points[1]));
            return Ok(Geometry::Polyline(
                Polyline3::try_new(vertices, tolerance)?.try_chord_length_parameterized()?,
            ));
        }
    }
    // Rhino assigns consecutive native domains to child curves as well as
    // the outer polycurve. Reparameterization leaves controls and weights intact.
    let mut parameter = curves.first().map_or(0.0, |curve| *curve.domain().start());
    let segments = curves
        .into_iter()
        .map(|curve| {
            let end = parameter + (*curve.domain().end() - *curve.domain().start());
            let result = curve.try_reparameterized(parameter..=end);
            parameter = end;
            result
        })
        .collect::<Result<Vec<_>, _>>()?;
    Ok(Geometry::PolyCurve(PolyCurve3::try_new(segments)?))
}
