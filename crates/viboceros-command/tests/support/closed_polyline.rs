use serde_json::{Value, json};
use viboceros_document::{
    ColorRgb, Document, Geometry, GeometrySnapshot, ObjectAttributes, ObjectColorSource, ObjectId,
    SelectionMode,
};
use viboceros_geometry::{
    MeshFace, MeshProjectionIndex, Point3, Polyline3, Real, Tolerance, TriangleMesh,
};

pub struct Scene {
    pub document: Document,
    pub sources: Vec<ObjectId>,
    pub parents: Vec<GeometrySnapshot>,
}
pub fn rows() -> Vec<Value> {
    let mut rows = Vec::new();
    for (source, count) in [
        (
            include_str!("../../../../tools/rhino_oracle/observations/mesh_polyline.json"),
            21,
        ),
        (
            include_str!("../../../../tools/rhino_oracle/observations/mesh_polyline_variants.json"),
            10,
        ),
    ] {
        let response: Value = serde_json::from_str(source).unwrap();
        assert_eq!(response["results"].as_array().unwrap().len(), count);
        rows.extend(response["results"].as_array().unwrap().iter().cloned());
    }
    rows
}
fn point(value: &Value) -> Point3 {
    Point3::try_from(serde_json::from_value::<[Real; 3]>(value.clone()).unwrap()).unwrap()
}
pub fn polyline(value: &Value) -> Polyline3 {
    Polyline3::try_new(
        value.as_array().unwrap().iter().map(point).collect(),
        Tolerance::NUMERICAL_VALIDATION,
    )
    .unwrap()
}
pub fn setup(value: &Value) -> Scene {
    let mut document = Document::new(Tolerance::try_new(1e-6, 1e-12, 1e-6).unwrap());
    let source = document
        .add_layer("MeshPolyline source", ColorRgb::new(10, 20, 30))
        .unwrap();
    let current = document
        .add_layer("MeshPolyline output", ColorRgb::new(40, 50, 60))
        .unwrap();
    let mut sources = Vec::new();
    for (index, points) in value["input"]["polylines"]
        .as_array()
        .unwrap()
        .iter()
        .enumerate()
    {
        let p = polyline(points);
        let geometry = if value["input"]["nurbs"] == true {
            Geometry::NurbsCurve(p.to_nurbs().unwrap())
        } else {
            Geometry::Polyline(p)
        };
        sources.push(
            document
                .add_geometry_with_attributes(
                    geometry,
                    ObjectAttributes::on_layer(source)
                        .with_name(format!("Input {index}"))
                        .with_object_color(ColorRgb::new(37, 83, 149)),
                )
                .unwrap(),
        );
    }
    let parents = sources
        .iter()
        .map(|id| document.object(*id).unwrap().geometry_snapshot().clone())
        .collect();
    document.set_current_layer(current).unwrap();
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
pub fn cancelled(value: &Value) -> bool {
    value["events"]
        .as_array()
        .unwrap()
        .iter()
        .any(|event| event["result"] == "Cancel")
}
pub fn output_count(value: &Value) -> usize {
    value["after_script"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|o| o["role"] == "output")
        .count()
}
pub fn check(
    doc: &Document,
    sources: &[ObjectId],
    parents: &[GeometrySnapshot],
    value: &Value,
    phase: &str,
) -> Value {
    let native = value[phase].as_array().unwrap();
    for (index, id) in sources.iter().enumerate() {
        let object = doc.object(*id).unwrap();
        assert!(parents[index].shares_storage_with(object.geometry_snapshot()));
        let target = native
            .iter()
            .find(|o| o["source_index"].as_u64() == Some(index as u64))
            .unwrap();
        attributes(doc, *id, target);
    }
    let output = doc
        .objects()
        .filter(|o| !sources.contains(&o.id()))
        .collect::<Vec<_>>();
    let expected = native
        .iter()
        .filter(|o| o["role"] == "output")
        .collect::<Vec<_>>();
    let mut qualified = output.len() == expected.len();
    let mut maximum: Real = 0.;
    let mut comparisons = Vec::new();
    for (object, target) in output.iter().zip(&expected) {
        attributes(doc, object.id(), target);
        let Geometry::Mesh(mesh) = object.geometry() else {
            panic!("mesh required")
        };
        let comparison = compare(Some(mesh), &target["mesh"]);
        qualified &= comparison["qualified"] == true;
        maximum = maximum.max(
            comparison["maximum_bidirectional_sample_error"]
                .as_f64()
                .unwrap_or(0.),
        );
        comparisons.push(comparison);
    }
    json!({"qualified":qualified,"maximum_bidirectional_sample_error":if output.len()==expected.len(){Some(maximum)}else{None},
           "actual_meshes":output.len(),"native_meshes":expected.len(),
           "sources_and_attributes_preserved":true,"meshes":comparisons})
}
fn attributes(doc: &Document, id: ObjectId, target: &Value) {
    let object = doc.object(id).unwrap();
    assert_eq!(doc.is_selected(id), target["selected"].as_bool().unwrap());
    let a = object.attributes();
    assert_eq!(a.name(), target["name"].as_str());
    assert_eq!(
        doc.layer(a.layer_id()).unwrap().name(),
        target["layer"].as_str().unwrap()
    );
    assert_eq!(a.color_source(), ObjectColorSource::Object);
    assert_eq!(a.object_color(), ColorRgb::new(37, 83, 149));
}
pub fn compare(actual: Option<&TriangleMesh>, target: &Value) -> Value {
    let Some(actual) = actual else {
        return json!({"qualified":target.is_null(),"local_empty":true,"native_empty":target.is_null()});
    };
    if target.is_null() {
        return json!({"qualified":false,"native_empty":true});
    }
    let expected = TriangleMesh::try_new_faces(
        target["vertices"]
            .as_array()
            .unwrap()
            .iter()
            .map(point)
            .collect(),
        target["faces"]
            .as_array()
            .unwrap()
            .iter()
            .map(|f| MeshFace::Triangle(serde_json::from_value::<[u32; 3]>(f.clone()).unwrap()))
            .collect(),
        Tolerance::MESH_VALIDATION,
    )
    .unwrap();
    let error = directed(actual, &expected).max(directed(&expected, actual));
    let counts = actual.vertices().len() == expected.vertices().len()
        && actual.faces().len() == expected.faces().len();
    json!({"qualified":error<=1e-6&&counts,"maximum_bidirectional_sample_error":error,
           "counts_match":counts,"actual_vertices":actual.vertices().len(),"native_vertices":expected.vertices().len(),
           "actual_faces":actual.faces().len(),"native_faces":expected.faces().len()})
}
pub fn directed(a: &TriangleMesh, b: &TriangleMesh) -> Real {
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
