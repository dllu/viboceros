//! Actual mesh command diagnostics, with raw facets retained for comparison.
use super::*;
pub(super) fn run(
    source: &Geometry,
    iterations: u32,
    tolerance: Tolerance,
) -> Result<(Value, u64), ProbeError> {
    let Geometry::Mesh(mesh) = source else {
        unreachable!()
    };
    let count = |m: &TriangleMesh| {
        m.face_count() - m.ngons().iter().map(|g| g.faces().len() - 1).sum::<usize>()
    };
    let before = count(mesh);
    measure_probe(iterations, || -> Result<Value, ProbeError> {
        let mut doc = Document::new(tolerance);
        let id = doc.add_geometry(source.clone())?;
        doc.select_objects_direct([id], viboceros_document::SelectionMode::Replace)?;
        CommandRegistry::with_builtins().execute(&mut doc, "MergeAllCoplanarFaces")?;
        let Geometry::Mesh(m) = doc.object(id).unwrap().geometry() else {
            unreachable!()
        };
        let bounds = m.bounds();
        let closed = m.topology().is_closed();
        let mut ngon_faces = m
            .ngons()
            .iter()
            .map(|g| g.faces().len())
            .collect::<Vec<_>>();
        ngon_faces.sort_unstable();
        Ok(
            json!({"logical_face_count":count(m),"face_count":m.face_count(),"vertex_count":m.vertices().len(),"ngon_count":m.ngons().len(),"ngon_face_counts":ngon_faces,"removed_faces":before-count(m),"is_closed":closed,"area":m.area()?,"volume":if closed{Some(m.signed_volume()?.abs())}else{None},"bounds":[bounds.min().to_array(),bounds.max().to_array()],"mesh":polygon_mesh_value(m)}),
        )
    })
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn mesh_coplanar_recipes_match_independent_regions() {
        let q: ProbeRequest = serde_json::from_str(include_str!(
            "../../../tools/rhino_oracle/fixtures/coplanar_mesh.json"
        ))
        .unwrap();
        let response = run_request(&q).unwrap();
        for row in response.results {
            let (faces, removed, area) = match row.id.as_str() {
                "rectangle" | "shifted" | "seam" => (1, 1, 12.),
                "annulus" => (1, 7, 8.),
                "box" => (6, 6, 52.),
                "disconnected" => (2, 2, 2.),
                "concave" => (1, 1, 1.),
                _ => panic!(),
            };
            assert_eq!(row.value["logical_face_count"], faces, "{}", row.id);
            assert_eq!(row.value["removed_faces"], removed, "{}", row.id);
            assert!(
                (row.value["area"].as_f64().unwrap() - area).abs() < 1e-10,
                "{}",
                row.id
            );
            if row.id == "box" {
                assert!((row.value["volume"].as_f64().unwrap() - 24.).abs() < 1e-10);
            }
        }
    }
}
