use serde_json::Value;
use viboceros_document::{ColorRgb, Document, Geometry, ObjectId, SelectionMode};
use viboceros_geometry::{MeshFace, Point3, Polyline3, Tolerance, TriangleMesh};
pub fn point(v: &Value) -> Point3 {
    Point3::try_from(serde_json::from_value::<[f64; 3]>(v.clone()).unwrap()).unwrap()
}
pub fn rows() -> Vec<(Value, Value)> {
    captured(
        include_str!("../../../../tools/rhino_oracle/fixtures/polyline_on_mesh.json"),
        include_str!("../../../../tools/rhino_oracle/observations/polyline_on_mesh.json"),
        12,
    )
}
pub fn boundary_rows() -> Vec<(Value, Value)> {
    captured(
        include_str!("../../../../tools/rhino_oracle/fixtures/mesh_polyline_boundaries.json"),
        include_str!("../../../../tools/rhino_oracle/observations/mesh_polyline_boundaries.json"),
        11,
    )
}
pub fn tie_rows() -> Vec<(Value, Value)> {
    captured(
        include_str!("../../../../tools/rhino_oracle/fixtures/mesh_polyline_ties.json"),
        include_str!("../../../../tools/rhino_oracle/observations/mesh_polyline_ties.json"),
        6,
    )
}
fn captured(request: &str, response: &str, count: usize) -> Vec<(Value, Value)> {
    let q: Value = serde_json::from_str(request).unwrap();
    let r: Value = serde_json::from_str(response).unwrap();
    assert_eq!(q["operations"].as_array().unwrap().len(), count);
    assert_eq!(r["results"].as_array().unwrap().len(), count);
    q["operations"]
        .as_array()
        .unwrap()
        .iter()
        .zip(r["results"].as_array().unwrap())
        .map(|(op, row)| {
            assert_eq!(op["id"], row["id"]);
            assert_eq!(row["value"]["input_trace"]["errors"], serde_json::json!([]));
            (op.clone(), row["value"].clone())
        })
        .collect()
}
pub fn setup(v: &Value) -> (Document, ObjectId) {
    let mut doc = Document::new(Tolerance::try_new(1e-6, 1e-12, 1e-9).unwrap());
    let source = doc
        .add_layer("Mesh polyline source", ColorRgb::new(10, 20, 30))
        .unwrap();
    let output = doc
        .add_layer("Mesh polyline output", ColorRgb::new(40, 50, 60))
        .unwrap();
    doc.set_current_layer(source).unwrap();
    let mesh = TriangleMesh::try_new_faces(
        v["mesh"]["vertices"]
            .as_array()
            .unwrap()
            .iter()
            .map(point)
            .collect(),
        v["mesh"]["faces"]
            .as_array()
            .unwrap()
            .iter()
            .map(|f| {
                match serde_json::from_value::<Vec<u32>>(f.clone())
                    .unwrap()
                    .as_slice()
                {
                    [a, b, c] => MeshFace::Triangle([*a, *b, *c]),
                    [a, b, c, d] => MeshFace::Quad([*a, *b, *c, *d]),
                    _ => panic!(),
                }
            })
            .collect(),
        doc.tolerance(),
    )
    .unwrap();
    let id = doc.add_geometry(Geometry::Mesh(mesh)).unwrap();
    doc.set_object_names([(id, Some("Target".into()))]).unwrap();
    doc.select_object(id, SelectionMode::Replace).unwrap();
    doc.set_current_layer(output).unwrap();
    doc.clear_history().unwrap();
    (doc, id)
}
pub fn check(doc: &Document, target: ObjectId, v: &Value, phase: &str) -> f64 {
    let actual = doc.objects().collect::<Vec<_>>();
    let expected = v[phase].as_array().unwrap();
    assert_eq!(actual.len(), expected.len(), "{phase}");
    let mut error = 0_f64;
    for (a, b) in actual.into_iter().zip(expected) {
        let role = if a.id() == target { "target" } else { "output" };
        assert_eq!(b["role"], role);
        assert_eq!(doc.is_selected(a.id()), b["selected"].as_bool().unwrap());
        assert_eq!(a.attributes().name(), b["name"].as_str());
        assert_eq!(
            doc.layer(a.attributes().layer_id()).unwrap().name(),
            b["layer"].as_str().unwrap()
        );
        if role == "target" {
            continue;
        }
        let Geometry::Polyline(actual) = a.geometry() else {
            panic!()
        };
        let native = Polyline3::try_new(
            b["polyline"]
                .as_array()
                .unwrap()
                .iter()
                .map(point)
                .collect(),
            Tolerance::NUMERICAL_VALIDATION,
        )
        .unwrap();
        for (from, to) in [(actual, &native), (&native, actual)] {
            for segment in from.segments() {
                for i in 0..=16 {
                    let p = segment.point_at(i as f64 / 16.).unwrap();
                    error = error.max(
                        p.distance_to(
                            to.closest_point(p, Tolerance::NUMERICAL_VALIDATION)
                                .unwrap(),
                        )
                        .unwrap(),
                    );
                }
            }
        }
    }
    error
}
