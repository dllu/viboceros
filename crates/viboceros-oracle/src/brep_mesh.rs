//! Geometry/invariant comparison of independently meshed refined box-face subsets.

use super::{ProbeError, measure};
use serde::Deserialize;
use serde_json::{Value, json};
use viboceros_command::CommandRegistry;
use viboceros_document::{Document, Geometry, SelectionMode};
use viboceros_geometry::{
    Brep, BrepFace, Frame3, GeometryError, MeshEdgeFilter, Point3, Tolerance, TriangleMesh, Vector3,
};

#[derive(Clone, Debug, Deserialize, PartialEq)]
pub struct RefinedBoxFace {
    /// Unit-box local face center, independent of engine-specific face numbering.
    pub center: [f64; 3],
    #[serde(default)]
    pub knots_u: Vec<f64>,
    #[serde(default)]
    pub knots_v: Vec<f64>,
}

#[derive(Clone, Debug, Deserialize, PartialEq)]
pub struct BrepMeshFixture {
    #[serde(default)]
    pub origin: [f64; 3],
    pub faces: Vec<RefinedBoxFace>,
    pub density: f64,
    pub simple_planes: bool,
}

fn build(fixture: &BrepMeshFixture, tolerance: Tolerance) -> Result<Brep, ProbeError> {
    let origin = Point3::try_from(fixture.origin)?;
    let frame = Frame3::try_from_directions(
        origin,
        Vector3::try_new(1.0, 0.0, 0.0)?,
        Vector3::try_new(0.0, 1.0, 0.0)?,
        tolerance,
    )?;
    let source = Brep::try_box(frame, [[0.0, 1.0]; 3], tolerance)?;
    let mut indices = Vec::new();
    for requested in &fixture.faces {
        let center = origin.translated(Vector3::try_from(requested.center)?)?;
        let mut found = None;
        for (i, face) in source.faces().iter().enumerate() {
            let s = face.surface();
            if s.evaluate(s.parameter_at_u(0.5)?, s.parameter_at_v(0.5)?)?
                .distance_to(center)?
                <= tolerance.absolute()
            {
                found = Some(i);
                break;
            }
        }
        indices.push(found.ok_or(ProbeError::FixtureInvariant("unknown box face center"))?);
    }
    let source = source.duplicate_faces(&indices, tolerance)?;
    let faces = source
        .faces()
        .iter()
        .zip(&fixture.faces)
        .map(|(face, requested)| {
            let mut surface = face.surface().clone();
            for &t in &requested.knots_u {
                surface = surface.try_insert_knot_u(surface.parameter_at_u(t)?, 1)?;
            }
            for &t in &requested.knots_v {
                surface = surface.try_insert_knot_v(surface.parameter_at_v(t)?, 1)?;
            }
            BrepFace::try_new(surface, face.is_reversed(), face.loops().to_vec())
        })
        .collect::<Result<Vec<_>, GeometryError>>()?;
    Ok(Brep::try_new(
        source.vertices().to_vec(),
        source.edges().to_vec(),
        faces,
        tolerance,
    )?)
}

pub(super) fn run(
    fixture: &BrepMeshFixture,
    iterations: u32,
    tolerance: Tolerance,
) -> Result<(Value, u64), ProbeError> {
    let brep = build(fixture, tolerance)?;
    let (mesh, elapsed) = measure(iterations, || {
        brep.polygon_mesh(fixture.density, fixture.simple_planes, false, tolerance)
    })?;
    let mut document = Document::new(tolerance);
    let id = document.add_geometry(Geometry::Brep(brep.clone()))?;
    document.select_object(id, SelectionMode::Replace)?;
    let original = document.object(id).cloned();
    CommandRegistry::with_builtins().execute(
        &mut document,
        &format!(
            "Mesh Density={} SimplePlanes={} JaggedSeams=No",
            fixture.density,
            if fixture.simple_planes { "Yes" } else { "No" },
        ),
    )?;
    let generated = document.objects().find(|object| object.id() != id);
    if document.objects().len() != 2
        || document.object(id) != original.as_ref()
        || !document.is_selected(id)
        || generated.is_none_or(|object| {
            document.is_selected(object.id())
                || !matches!(object.geometry(), Geometry::Mesh(result) if result == &mesh)
        })
    {
        return Err(ProbeError::FixtureInvariant(
            "Mesh command did not preserve its source or match direct meshing",
        ));
    }
    Ok((record(&brep, &mesh, tolerance)?, elapsed))
}

fn record(brep: &Brep, mesh: &TriangleMesh, tolerance: Tolerance) -> Result<Value, GeometryError> {
    let topology = mesh.topology();
    let lines = mesh.filtered_edge_lines(MeshEdgeFilter::Naked, tolerance)?;
    let loops = mesh.filtered_edge_polylines(MeshEdgeFilter::Naked, tolerance)?;
    let boundary_length = lines
        .iter()
        .map(|l| l.length())
        .collect::<Result<Vec<_>, _>>()?
        .iter()
        .sum::<f64>();
    let mut queries = Vec::new();
    for (i, edge) in brep.edges().iter().enumerate() {
        if brep.edge_use_count(i) == Some(1) {
            for s in [0.0, 0.25, 0.5, 0.75, 1.0] {
                queries.push(
                    edge.curve()
                        .evaluate(edge.curve().parameter_at(s)?)?
                        .to_array(),
                );
            }
        }
    }
    queries.sort_by(|a, b| {
        a[0].total_cmp(&b[0])
            .then(a[1].total_cmp(&b[1]))
            .then(a[2].total_cmp(&b[2]))
    });
    queries.dedup();
    let mut samples = Vec::new();
    for query in queries {
        let query = Point3::try_from(query)?;
        let mut best: Option<(f64, Point3)> = None;
        for &line in &lines {
            let point = line.closest_point(query, tolerance)?;
            let distance = point.distance_to(query)?;
            if best.is_none_or(|b| distance < b.0) {
                best = Some((distance, point));
            }
        }
        let (distance, point) = best.ok_or(GeometryError::InvalidBrepTopology {
            context: "mesh lost a box boundary",
        })?;
        if distance > tolerance.absolute() {
            return Err(GeometryError::InvalidBrepTopology {
                context: "box boundary leaves its mesh",
            });
        }
        samples.push(point.to_array());
    }
    Ok(
        json!({"area": mesh.area()?, "boundary_length": boundary_length,
            "boundary_loops": loops.len(), "boundaries_closed": loops.iter().all(|l| l.is_closed()),
            "closed": topology.is_closed(), "manifold": topology.is_manifold(), "oriented": topology.is_oriented(),
            "boundary_samples": samples,
        }),
    )
}

#[cfg(test)]
mod tests {
    #[test]
    fn refined_box_face_fixture_preserves_expected_open_and_closed_mesh_boundaries() {
        let request: crate::ProbeRequest = serde_json::from_str(include_str!(
            "../../../tools/rhino_oracle/fixtures/brep_mesh_boundaries.json"
        ))
        .unwrap();
        let response = crate::run_request(&request).unwrap();
        assert_eq!(response.results.len(), 5);
        for (result, (area, perimeter, loops)) in response.results.iter().zip([
            (2.0, 6.0, 1),
            (5.0, 4.0, 1),
            (6.0, 0.0, 0),
            (2.0, 8.0, 2),
            (2.0, 6.0, 1),
        ]) {
            assert!((result.value["area"].as_f64().unwrap() - area).abs() < 1e-10);
            assert!((result.value["boundary_length"].as_f64().unwrap() - perimeter).abs() < 1e-10);
            assert_eq!(result.value["boundary_loops"], loops);
        }
    }
}
