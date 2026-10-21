use serde_json::Value;
use viboceros_document::{
    ColorRgb, Document, Geometry, GeometrySnapshot, ObjectAttributes, ObjectId, SelectionMode,
};
use viboceros_geometry::{
    LineSegment, MeshFace, MeshProjectionIndex, Point3, Polyline3, Real, Tolerance, TriangleMesh,
};
pub struct Scene {
    pub document: Document,
    pub sources: Vec<ObjectId>,
    pub parents: Vec<(ObjectId, GeometrySnapshot)>,
}
pub fn groups() -> Vec<(&'static str, Vec<Value>)> {
    [
        (
            "MESH_FROM_LINES",
            include_str!("../../../../tools/rhino_oracle/observations/mesh_from_lines.json"),
            18,
        ),
        (
            "MESH_FROM_LINES_VARIANTS",
            include_str!(
                "../../../../tools/rhino_oracle/observations/mesh_from_lines_variants.json"
            ),
            16,
        ),
        (
            "MESH_FROM_LINES_CONTROLS",
            include_str!(
                "../../../../tools/rhino_oracle/observations/mesh_from_lines_controls.json"
            ),
            18,
        ),
        (
            "MESH_FROM_LINES_EDGES",
            include_str!("../../../../tools/rhino_oracle/observations/mesh_from_lines_edges.json"),
            8,
        ),
    ]
    .into_iter()
    .map(|(prefix, source, count)| {
        let value: Value = serde_json::from_str(source).unwrap();
        let rows = value["results"].as_array().unwrap().clone();
        assert_eq!(rows.len(), count);
        (prefix, rows)
    })
    .collect()
}
fn point(value: &Value) -> Point3 {
    Point3::try_from(serde_json::from_value::<[Real; 3]>(value.clone()).unwrap()).unwrap()
}
pub fn setup(value: &Value) -> Scene {
    let mut document = Document::new(Tolerance::try_new(1e-6, 1e-12, 1e-6).unwrap());
    let source = document
        .add_layer("MeshFromLines source", ColorRgb::new(10, 20, 30))
        .unwrap();
    let output = document
        .add_layer("MeshFromLines output", ColorRgb::new(40, 50, 60))
        .unwrap();
    let mut sources = Vec::new();
    for line in value["input"]["lines"].as_array().unwrap() {
        sources.push(
            document
                .add_geometry_with_attributes(
                    Geometry::Line(
                        LineSegment::try_new(
                            point(&line[0]),
                            point(&line[1]),
                            Tolerance::NUMERICAL_VALIDATION,
                        )
                        .unwrap(),
                    ),
                    ObjectAttributes::on_layer(source).with_name("Input"),
                )
                .unwrap(),
        );
    }
    if let Some(polylines) = value["input"]["polylines"].as_array() {
        for p in polylines {
            sources.push(
                document
                    .add_geometry_with_attributes(
                        Geometry::Polyline(
                            Polyline3::try_new(
                                p.as_array().unwrap().iter().map(point).collect(),
                                Tolerance::NUMERICAL_VALIDATION,
                            )
                            .unwrap(),
                        ),
                        ObjectAttributes::on_layer(source).with_name("Input"),
                    )
                    .unwrap(),
            );
        }
    }
    let parents = sources
        .iter()
        .map(|id| {
            (
                *id,
                document.object(*id).unwrap().geometry_snapshot().clone(),
            )
        })
        .collect();
    document.set_current_layer(output).unwrap();
    document
        .select_objects_direct(sources.clone(), SelectionMode::Replace)
        .unwrap();
    document.clear_history().unwrap();
    Scene {
        document,
        sources,
        parents,
    }
}
pub fn command(value: &Value) -> String {
    // Captures retain rejected option spellings in their command history.
    // Replays use the effective accepted settings, with token parity reported
    // separately instead of treating those rejected tokens as qualified.
    format!(
        "MeshFromLines MaxSidesPerFace={} DeleteInput={} OutputLayer=Current",
        value["input"]["max_sides"].as_u64().unwrap_or(4),
        if value["input"]["delete"] == true {
            "Yes"
        } else {
            "No"
        }
    )
}
pub fn native_options_supported(value: &Value) -> bool {
    !value["history"]
        .as_str()
        .unwrap()
        .contains("Unknown command:")
}
pub fn has_output(value: &Value) -> bool {
    value["after_script"]
        .as_array()
        .unwrap()
        .iter()
        .any(|o| o["role"] == "output")
}
pub fn check(
    doc: &Document,
    scene: &[(ObjectId, GeometrySnapshot)],
    value: &Value,
    phase: &str,
) -> Real {
    let native = value[phase].as_array().unwrap();
    assert_eq!(doc.objects().count(), native.len());
    let native_sources = native
        .iter()
        .filter(|o| o["role"] == "source")
        .collect::<Vec<_>>();
    let actual_sources = scene
        .iter()
        .filter(|(id, _)| doc.object(*id).is_some())
        .collect::<Vec<_>>();
    assert_eq!(actual_sources.len(), native_sources.len());
    for ((id, snapshot), reference) in actual_sources.into_iter().zip(native_sources) {
        let object = doc.object(*id).unwrap();
        assert!(snapshot.shares_storage_with(object.geometry_snapshot()));
        attributes(doc, *id, reference);
    }
    let output = doc
        .objects()
        .filter(|o| !scene.iter().any(|(id, _)| *id == o.id()))
        .collect::<Vec<_>>();
    let native_output = native
        .iter()
        .filter(|o| o["role"] == "output")
        .collect::<Vec<_>>();
    assert_eq!(output.len(), native_output.len());
    let mut maximum: Real = 0.;
    for (object, reference) in output.into_iter().zip(native_output) {
        attributes(doc, object.id(), reference);
        let Geometry::Mesh(mesh) = object.geometry() else {
            panic!("mesh output required")
        };
        let expected = read_mesh(&reference["mesh"]);
        assert_eq!(mesh.vertices().len(), expected.vertices().len());
        assert_eq!(mesh.faces().len(), expected.faces().len());
        maximum = maximum
            .max(directed(mesh, &expected))
            .max(directed(&expected, mesh));
    }
    maximum
}
fn attributes(doc: &Document, id: ObjectId, reference: &Value) {
    let object = doc.object(id).unwrap();
    assert_eq!(
        doc.is_selected(id),
        reference["selected"].as_bool().unwrap()
    );
    assert_eq!(object.attributes().name(), reference["name"].as_str());
    assert_eq!(
        doc.layer(object.attributes().layer_id()).unwrap().name(),
        reference["layer"].as_str().unwrap()
    );
}
fn read_mesh(value: &Value) -> TriangleMesh {
    TriangleMesh::try_new_faces(
        value["vertices"]
            .as_array()
            .unwrap()
            .iter()
            .map(point)
            .collect(),
        value["faces"]
            .as_array()
            .unwrap()
            .iter()
            .map(|v| {
                let f: Vec<u32> = serde_json::from_value(v.clone()).unwrap();
                if f.len() == 3 {
                    MeshFace::Triangle(f.try_into().unwrap())
                } else {
                    MeshFace::Quad(f.try_into().unwrap())
                }
            })
            .collect(),
        Tolerance::MESH_VALIDATION,
    )
    .unwrap()
}
fn directed(a: &TriangleMesh, b: &TriangleMesh) -> Real {
    let index = MeshProjectionIndex::new(b).unwrap();
    let mut maximum: Real = 0.;
    for face in a.triangles() {
        let [a, b, c] = face.map(|i| a.vertices()[i as usize].to_array());
        for i in 0..=8 {
            for j in 0..=8 - i {
                let u = i as Real / 8.;
                let v = j as Real / 8.;
                let p = Point3::try_from(std::array::from_fn::<_, 3, _>(|k| {
                    (b[k] - a[k]).mul_add(u, (c[k] - a[k]).mul_add(v, a[k]))
                }))
                .unwrap();
                maximum = maximum.max(
                    p.distance_to(index.closest_point(p).unwrap().unwrap())
                        .unwrap(),
                );
            }
        }
    }
    maximum
}
