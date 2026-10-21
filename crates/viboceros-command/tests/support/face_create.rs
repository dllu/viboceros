use serde_json::{Value, json};
use viboceros_document::{
    ColorRgb, Document, Geometry, GeometrySnapshot, ObjectAttributes, ObjectId, SelectionMode,
};
use viboceros_geometry::{
    MeshFace, MeshNgon, MeshProjectionIndex, Point3, Real, Tolerance, TriangleMesh,
};

pub struct Scene {
    pub document: Document,
    pub source: Option<ObjectId>,
    pub original: Option<GeometrySnapshot>,
}
pub fn rows() -> Vec<Value> {
    let mut rows = Vec::new();
    for (source, count) in [
        (
            include_str!("../../../../tools/rhino_oracle/observations/face_create.json"),
            18,
        ),
        (
            include_str!("../../../../tools/rhino_oracle/observations/face_create_variants.json"),
            7,
        ),
    ] {
        let r: Value = serde_json::from_str(source).unwrap();
        assert_eq!(r["results"].as_array().unwrap().len(), count);
        rows.extend(r["results"].as_array().unwrap().iter().cloned());
    }
    rows
}
#[allow(dead_code)] // Controls are exercised by the application getter tests.
pub fn controls() -> Vec<Value> {
    let response: Value = serde_json::from_str(include_str!(
        "../../../../tools/rhino_oracle/observations/face_create_controls.json"
    ))
    .unwrap();
    let rows = response["results"].as_array().unwrap();
    assert_eq!(rows.len(), 5);
    rows.clone()
}
#[allow(dead_code)]
pub fn drafts() -> Vec<Value> {
    let mut rows = vec![];
    for (source, count) in [
        (
            include_str!("../../../../tools/rhino_oracle/observations/face_create_drafts.json"),
            12,
        ),
        (
            include_str!(
                "../../../../tools/rhino_oracle/observations/face_create_draft_extras.json"
            ),
            6,
        ),
    ] {
        let data: Value = serde_json::from_str(source).unwrap();
        let group = data["results"].as_array().unwrap();
        assert_eq!(group.len(), count);
        rows.extend(group.iter().cloned());
    }
    rows
}
#[allow(dead_code)]
pub fn edge_index(mesh: &TriangleMesh, pick: &Value) -> usize {
    let pick = point(pick);
    mesh.diagnostic_edges()
        .into_iter()
        .filter(|e| e.face_count == 1)
        .find(|e| {
            let [a, b] = e.points;
            let ab = a.vector_to(b).unwrap();
            let ap = a.vector_to(pick).unwrap();
            let t = ap.dot(ab).unwrap() / ab.dot(ab).unwrap();
            (0. ..=1.).contains(&t)
                && a.translated(ab.scaled(t).unwrap())
                    .unwrap()
                    .distance_to(pick)
                    .unwrap()
                    < 1e-8
        })
        .expect("captured boundary edge")
        .index
}
#[allow(dead_code)]
pub fn draft_header(value: &Value) -> String {
    format!(
        "{} Chain={}",
        header(value).replace("FromEdge=No", "FromEdge=Yes"),
        if value["input"]["chain"] == true {
            "Yes"
        } else {
            "No"
        }
    )
}
fn point(v: &Value) -> Point3 {
    Point3::try_from(serde_json::from_value::<[Real; 3]>(v.clone()).unwrap()).unwrap()
}
fn try_mesh(v: &Value, ngons: &Value) -> Result<TriangleMesh, viboceros_geometry::GeometryError> {
    let mesh = TriangleMesh::try_new_faces(
        v["vertices"]
            .as_array()
            .unwrap()
            .iter()
            .map(point)
            .collect(),
        v["faces"]
            .as_array()
            .unwrap()
            .iter()
            .map(|f| {
                let f: Vec<u32> = serde_json::from_value(f.clone()).unwrap();
                match f.len() {
                    3 => MeshFace::Triangle(f.try_into().unwrap()),
                    _ => MeshFace::Quad(f.try_into().unwrap()),
                }
            })
            .collect(),
        Tolerance::MESH_VALIDATION,
    )?;
    mesh.try_with_ngons(
        ngons
            .as_array()
            .unwrap()
            .iter()
            .map(|n| {
                MeshNgon::from_parts(
                    serde_json::from_value(n["vertices"].clone()).unwrap(),
                    serde_json::from_value(n["faces"].clone()).unwrap(),
                )
            })
            .collect(),
    )
}
fn mesh(v: &Value, ngons: &Value) -> TriangleMesh {
    try_mesh(v, ngons).unwrap()
}
fn comparison_mesh(v: &Value, ngons: &Value) -> (TriangleMesh, Option<String>) {
    match try_mesh(v, ngons) {
        Ok(mesh) => (mesh, None),
        Err(error) => {
            let vertices = v["vertices"]
                .as_array()
                .unwrap()
                .iter()
                .map(point)
                .collect::<Vec<_>>();
            let mut parts = vec![];
            for face in v["faces"].as_array().unwrap() {
                let indices: Vec<usize> = serde_json::from_value(face.clone()).unwrap();
                let splits = if indices.len() == 3 {
                    vec![[indices[0], indices[1], indices[2]]]
                } else {
                    let [a, b, c, d]: [usize; 4] = indices.try_into().unwrap();
                    if vertices[a].distance_to(vertices[c]).unwrap()
                        <= vertices[b].distance_to(vertices[d]).unwrap()
                    {
                        vec![[a, b, c], [a, c, d]]
                    } else {
                        vec![[a, b, d], [b, c, d]]
                    }
                };
                for face in splits {
                    if let Ok(mesh) = TriangleMesh::try_new(
                        face.map(|i| vertices[i]).to_vec(),
                        vec![[0, 1, 2]],
                        Tolerance::MESH_VALIDATION,
                    ) {
                        parts.push(mesh);
                    }
                }
            }
            let mesh = TriangleMesh::try_append(&parts.iter().collect::<Vec<_>>()).unwrap();
            (mesh, Some(error.to_string()))
        }
    }
}
pub fn setup(value: &Value) -> Scene {
    let mut document = Document::new(Tolerance::try_new(1e-6, 1e-12, 1e-6).unwrap());
    let source_layer = document
        .add_layer("3DFace source", ColorRgb::new(10, 20, 30))
        .unwrap();
    let output = document
        .add_layer("3DFace output", ColorRgb::new(40, 50, 60))
        .unwrap();
    let source = if value["input"]["append"] == true {
        let reference = &value["before"][0];
        Some(
            document
                .add_geometry_with_attributes(
                    Geometry::Mesh(mesh(&reference["mesh"], &reference["ngons"])),
                    ObjectAttributes::on_layer(source_layer).with_name("Input"),
                )
                .unwrap(),
        )
    } else {
        None
    };
    let original = source.map(|id| document.object(id).unwrap().geometry_snapshot().clone());
    document.set_current_layer(output).unwrap();
    if let Some(id) = source {
        document
            .select_objects_direct([id], SelectionMode::Replace)
            .unwrap();
    }
    document.clear_history().unwrap();
    Scene {
        document,
        source,
        original,
    }
}
pub fn header(value: &Value) -> String {
    let v = &value["input"];
    format!(
        "3DFace Output=Mesh PolygonType={} FromEdge=No InPlane={} WeldAngle={} Mode={}",
        v["polygon"].as_str().unwrap_or("Quad"),
        if v["in_plane"] == true { "Yes" } else { "No" },
        v["weld_angle"].as_f64().unwrap_or(180.),
        if v["multiple"] == true {
            "MultipleFaces"
        } else {
            "SingleFace"
        }
    )
}
pub fn actions(value: &Value) -> Vec<String> {
    value["input"]["points"]
        .as_array()
        .unwrap()
        .iter()
        .map(|p| {
            if let Some(word) = p.as_str() {
                word.to_owned()
            } else {
                let p: [Real; 3] = serde_json::from_value(p.clone()).unwrap();
                p.iter()
                    .map(ToString::to_string)
                    .collect::<Vec<_>>()
                    .join(",")
            }
        })
        .collect()
}
pub fn command(value: &Value) -> String {
    format!("{} {} Enter", header(value), actions(value).join(" "))
}
pub fn check(
    doc: &Document,
    source: Option<ObjectId>,
    original: Option<&GeometrySnapshot>,
    value: &Value,
    phase: &str,
) -> Value {
    let targets = value[phase].as_array().unwrap();
    assert_eq!(doc.objects().count(), targets.len());
    let mut comparisons = Vec::new();
    let mut metadata = true;
    for (object, target) in doc.objects().zip(targets) {
        metadata &= source.is_none_or(|id| object.id() == id)
            && doc.is_selected(object.id()) == target["selected"].as_bool().unwrap()
            && object.attributes().name() == target["name"].as_str()
            && doc.layer(object.attributes().layer_id()).unwrap().name()
                == target["layer"].as_str().unwrap();
        if phase == "undo" && source == Some(object.id()) {
            assert!(
                original
                    .unwrap()
                    .shares_storage_with(object.geometry_snapshot())
            );
        }
        let Geometry::Mesh(actual) = object.geometry() else {
            panic!("mesh required")
        };
        let (expected, admission) = comparison_mesh(&target["mesh"], &target["ngons"]);
        let error = directed(actual, &expected).max(directed(&expected, actual));
        let counts = actual.vertices().len()
            == target["mesh"]["vertices"].as_array().unwrap().len()
            && actual.faces().len() == target["mesh"]["faces"].as_array().unwrap().len()
            && actual.faces().iter().filter(|f| f.is_triangle()).count()
                == target["mesh"]["faces"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .filter(|f| f.as_array().unwrap().len() == 3)
                    .count();
        let ngons = actual.ngons().len() == expected.ngons().len()
            && actual.ngons().iter().zip(expected.ngons()).all(|(a, b)| {
                let a = a
                    .vertices()
                    .iter()
                    .map(|&i| actual.vertices()[i as usize])
                    .collect::<Vec<_>>();
                let b = b
                    .vertices()
                    .iter()
                    .map(|&i| expected.vertices()[i as usize])
                    .collect::<Vec<_>>();
                a.len() == b.len()
                    && (0..b.len()).any(|start| {
                        a.iter()
                            .zip(b.iter().cycle().skip(start))
                            .all(|(a, b)| a.distance_to(*b).unwrap() < 1e-6)
                    })
            });
        comparisons.push(json!({"qualified":error<=1e-6&&counts&&ngons,"native_legacy_diagonal_error":admission,"maximum_bidirectional_sample_error":error,"counts_match":counts,"ngons_match":ngons,
            "actual_vertices":actual.vertices().len(),"native_vertices":target["mesh"]["vertices"].as_array().unwrap().len(),"actual_faces":actual.faces().len(),"native_faces":target["mesh"]["faces"].as_array().unwrap().len(),
            "actual_ngons":actual.ngons().len(),"native_ngons":expected.ngons().len()}));
    }
    json!({"qualified":metadata&&comparisons.iter().all(|r|r["qualified"]==true),"meshes":comparisons,"attributes_and_identity_preserved":metadata})
}
fn directed(a: &TriangleMesh, b: &TriangleMesh) -> Real {
    let index = MeshProjectionIndex::new(b).unwrap();
    let mut maximum: Real = 0.;
    let (triangulated, _) = a.triangulate_quads(Tolerance::MESH_VALIDATION).unwrap();
    let a = &triangulated;
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
