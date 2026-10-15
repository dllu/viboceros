//! Actual command output compared with the explicit two-step native SDK profile.
use super::*;
use crate::object_source::ObjectSource;
#[derive(Clone, Debug, Deserialize, PartialEq)]
pub struct Fixture {
    pub source: ObjectSource,
}
pub(super) fn run(
    f: &Fixture,
    iterations: u32,
    tolerance: Tolerance,
) -> Result<(Value, u64), ProbeError> {
    let Geometry::Mesh(source) = f.source.geometry(tolerance)? else {
        return Err(ProbeError::FixtureInvariant(
            "Mesh cleanup requires mesh records",
        ));
    };
    measure_probe(iterations, || -> Result<Value, ProbeError> {
        let mut doc = Document::new(tolerance);
        let id = doc.add_geometry(Geometry::Mesh(source.clone()))?;
        doc.select_objects_direct([id], viboceros_document::SelectionMode::Replace)?;
        CommandRegistry::with_builtins().execute(&mut doc, "CullDegenerateMeshFaces")?;
        let mesh = match doc.object(id).map(|o| o.geometry()) {
            Some(Geometry::Mesh(m)) => Some(m),
            None => None,
            _ => {
                return Err(ProbeError::FixtureInvariant(
                    "Mesh cleanup returned another geometry family",
                ));
            }
        };
        let faces = mesh.map_or(0, |m| m.face_count());
        let vertices = mesh.map_or(0, |m| m.vertices().len());
        let quads = mesh.map_or(0, |m| {
            m.faces()
                .iter()
                .filter(|f| matches!(f, MeshFace::Quad(_)))
                .count()
        });
        Ok(
            json!({"empty":mesh.is_none(),"face_count":faces,"vertex_count":vertices,
            "triangle_count":faces-quads,"quad_count":quads,
            "removed_faces":source.face_count()-faces,"removed_vertices":source.vertices().len()-vertices,
            "area":mesh.map(|m| m.area()).transpose()?.unwrap_or(0.),
            "mesh":mesh.map(polygon_mesh_value).unwrap_or_else(|| json!({"faces":[],"vertices":[]}))}),
        )
    })
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn command_cleanup_fixtures_match_independent_count_and_area_expectations() {
        let request: ProbeRequest = serde_json::from_str(include_str!(
            "../../../tools/rhino_oracle/fixtures/cull_degenerate_mesh_faces.json"
        ))
        .unwrap();
        let result = run_request(&request).unwrap();
        // (faces, vertices, triangles, quads, removed faces, removed vertices, area)
        let expected = [
            (1, 3, 1, 0, 0, 0, 0.5),
            (0, 0, 0, 0, 1, 3, 0.),
            (0, 0, 0, 0, 1, 3, 0.),
            (1, 3, 1, 0, 0, 1, 0.5),
            (0, 0, 0, 0, 1, 4, 0.),
            (1, 3, 1, 0, 0, 1, 0.5),
            (1, 3, 1, 0, 1, 4, 0.5),
            (1, 3, 1, 0, 0, 1, 0.5),
            (2, 6, 2, 0, 0, 0, 1.),
            (1, 3, 1, 0, 0, 0, 0.5e-8),
            (1, 3, 1, 0, 0, 0, 0.5e-34),
            (0, 0, 0, 0, 1, 3, 0.),
            (1, 4, 0, 1, 0, 0, 1.),
        ];
        assert_eq!(result.results.len(), expected.len());
        for (row, (f, v, tri, q, rf, rv, area)) in result.results.iter().zip(expected) {
            let val = &row.value;
            assert_eq!(
                (
                    val["face_count"].as_u64().unwrap(),
                    val["vertex_count"].as_u64().unwrap(),
                    val["triangle_count"].as_u64().unwrap(),
                    val["quad_count"].as_u64().unwrap(),
                    val["removed_faces"].as_u64().unwrap(),
                    val["removed_vertices"].as_u64().unwrap()
                ),
                (f, v, tri, q, rf, rv),
                "{}",
                row.id
            );
            assert_eq!(val["empty"], f == 0, "{}", row.id);
            assert!(
                (val["area"].as_f64().unwrap() - area).abs() <= area.abs() * 2e-6,
                "{}",
                row.id
            );
        }
        assert_eq!(result.results[3].value["mesh"]["faces"], json!([[0, 1, 2]]));
        let raw = &result.results[8].value["mesh"]["vertices"];
        assert_eq!(raw[0], raw[3]); // Equal positions remain distinct occurrences.
        assert_ne!(
            result.results[9].value["mesh"]["vertices"][0],
            result.results[9].value["mesh"]["vertices"][1]
        );
    }
}
