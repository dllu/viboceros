use super::subd_native as native;
use serde_json::{Value, json};
use viboceros_document::{
    ColorRgb, Document, Geometry, GeometrySnapshot, ObjectAttributes, ObjectId, SelectionMode,
};
use viboceros_geometry::{MeshFace, Point3, SubD, TriangleMesh};

pub struct Scene {
    pub document: Document,
    pub source: Option<ObjectId>,
    pub original: Option<GeometrySnapshot>,
}
pub fn rows() -> Vec<Value> {
    let mut all = Vec::new();
    for (source, count) in [
        (
            include_str!(
                "../../../../tools/rhino_oracle/observations/face_create_subd_append.json"
            ),
            12,
        ),
        (
            include_str!(
                "../../../../tools/rhino_oracle/observations/face_create_subd_append_extras.json"
            ),
            10,
        ),
        (
            include_str!(
                "../../../../tools/rhino_oracle/observations/face_create_subd_drafts.json"
            ),
            10,
        ),
    ] {
        let data: Value = serde_json::from_str(source).unwrap();
        let rows = data["results"].as_array().unwrap();
        assert_eq!(rows.len(), count);
        all.extend(rows.iter().cloned());
    }
    all
}
fn point(value: &Value) -> Point3 {
    Point3::try_new(
        value[0].as_f64().unwrap(),
        value[1].as_f64().unwrap(),
        value[2].as_f64().unwrap(),
    )
    .unwrap()
}
fn geometry(value: &Value) -> Geometry {
    if let Some(subd) = value.get("subd") {
        let points = subd["points"]
            .as_array()
            .unwrap()
            .iter()
            .map(point)
            .collect();
        let faces = subd["faces"]
            .as_array()
            .unwrap()
            .iter()
            .map(|f| {
                f.as_array()
                    .unwrap()
                    .iter()
                    .map(|i| i.as_u64().unwrap() as u32)
                    .collect()
            })
            .collect();
        let mut geometry = SubD::try_from_polygons(points, faces).unwrap();
        let mut edges = Vec::new();
        for (native_edge, tag) in subd["edges"]
            .as_array()
            .unwrap()
            .iter()
            .zip(subd["edge_tags"].as_array().unwrap())
        {
            if tag == 2 {
                let a = native_edge[0].as_u64().unwrap() as u32;
                let b = native_edge[1].as_u64().unwrap() as u32;
                let i = geometry
                    .edges()
                    .iter()
                    .position(|e| *e == [a, b] || *e == [b, a])
                    .unwrap();
                if geometry.edge_tags()[i] != 2 {
                    edges.push((i, 2));
                }
            }
        }
        if !edges.is_empty() {
            geometry = geometry.try_with_edge_tags(&edges).unwrap();
        }
        let vertices = subd["vertex_tags"]
            .as_array()
            .unwrap()
            .iter()
            .enumerate()
            .filter_map(|(i, t)| {
                let tag = t.as_u64().unwrap() as u8;
                (geometry.vertex_tags()[i] != tag).then_some((i, tag))
            })
            .collect::<Vec<_>>();
        if !vertices.is_empty() {
            geometry = geometry.try_with_vertex_tags(&vertices).unwrap();
        }
        native::compare(&geometry, subd);
        Geometry::SubD(geometry)
    } else {
        let mesh = &value["mesh"];
        let vertices = mesh["vertices"]
            .as_array()
            .unwrap()
            .iter()
            .map(point)
            .collect();
        let faces = mesh["faces"]
            .as_array()
            .unwrap()
            .iter()
            .map(|f| {
                let f = f
                    .as_array()
                    .unwrap()
                    .iter()
                    .map(|i| i.as_u64().unwrap() as u32)
                    .collect::<Vec<_>>();
                if f.len() == 3 {
                    MeshFace::Triangle([f[0], f[1], f[2]])
                } else {
                    MeshFace::Quad([f[0], f[1], f[2], f[3]])
                }
            })
            .collect();
        Geometry::Mesh(TriangleMesh::try_from_face_records(vertices, faces).unwrap())
    }
}
pub fn setup(value: &Value) -> Scene {
    let mut document = Document::default();
    let source_layer = document
        .add_layer("3DFace source", ColorRgb::new(10, 20, 30))
        .unwrap();
    let output_layer = document
        .add_layer("3DFace output", ColorRgb::new(40, 50, 60))
        .unwrap();
    let source = value["before"].as_array().unwrap().first().map(|source| {
        let attributes = ObjectAttributes::on_layer(source_layer).with_name("Input");
        document
            .add_geometry_with_attributes(geometry(source), attributes)
            .unwrap()
    });
    let original = source.map(|id| document.object(id).unwrap().geometry_snapshot().clone());
    document.set_current_layer(output_layer).unwrap();
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
    let c = &value["input"];
    let from_edge =
        c["from_edge"] == true || c["points"].as_array().unwrap().iter().any(Value::is_object);
    format!(
        "3DFace Output={} Interpolated=No PolygonType={} InPlane={} Mode={} FromEdge={} Chain={}",
        c["output"].as_str().unwrap_or("SubD"),
        c["polygon"].as_str().unwrap_or("Quad"),
        if c["in_plane"] == true { "Yes" } else { "No" },
        if c["multiple"] == true {
            "MultipleFaces"
        } else {
            "SingleFace"
        },
        if from_edge { "Yes" } else { "No" },
        if c["chain"] == true { "Yes" } else { "No" }
    )
}
#[allow(dead_code)]
pub fn pick(value: &Value) -> Point3 {
    point(value)
}
#[allow(dead_code)] // Script-only entrypoint; getters use the individual actions.
pub fn command(scene: &Scene, value: &Value) -> String {
    use viboceros_command::face_create::{self as solver, Token};
    let header = header(value);
    let words = header.split_whitespace().skip(1).collect::<Vec<_>>();
    let input = solver::parse(&words, solver::Preferences::default()).unwrap();
    let mut draft = solver::Draft::with_source(
        input.preferences,
        solver::source(&scene.document, scene.source),
        None,
    )
    .unwrap();
    let mut actions = Vec::new();
    for action in value["input"]["points"].as_array().unwrap() {
        let (text, token) = if let Some(pick) = action.get("edge_pick") {
            let pick = point(pick);
            let mesh = draft.current_mesh().unwrap().unwrap();
            let index = mesh
                .diagnostic_edges()
                .into_iter()
                .find(|e| {
                    e.face_count == 1
                        && e.points[0]
                            .midpoint(e.points[1])
                            .unwrap()
                            .distance_to(pick)
                            .unwrap()
                            < 1e-8
                })
                .unwrap()
                .index;
            (format!("Edge={index}"), Token::Edge(index))
        } else if let Some(word) = action.as_str() {
            (
                word.to_owned(),
                if word == "Undo" {
                    Token::Undo
                } else {
                    Token::Enter
                },
            )
        } else {
            let text = action
                .as_array()
                .unwrap()
                .iter()
                .map(|x| x.as_f64().unwrap().to_string())
                .collect::<Vec<_>>()
                .join(",");
            (text, Token::Point(point(action)))
        };
        draft.accept(token).unwrap();
        actions.push(text);
    }
    format!("{} {} Enter", header, actions.join(" "))
}
pub fn check(scene: &Scene, value: &Value, phase: &str) -> Value {
    let expected = value[phase].as_array().unwrap();
    assert_eq!(scene.document.objects().count(), expected.len());
    let mut comparisons = Vec::new();
    let mut output = scene
        .document
        .objects()
        .filter(|o| Some(o.id()) != scene.source);
    for row in expected {
        let object = if row["role"] == "source" {
            scene.document.object(scene.source.unwrap()).unwrap()
        } else {
            output.next().unwrap()
        };
        assert_eq!(
            scene
                .document
                .layer(object.attributes().layer_id())
                .unwrap()
                .name(),
            row["layer"].as_str().unwrap()
        );
        assert_eq!(object.attributes().name(), row["name"].as_str());
        assert_eq!(
            scene.document.is_selected(object.id()),
            row["selected"].as_bool().unwrap()
        );
        if let Geometry::SubD(subd) = object.geometry() {
            comparisons.push(native::compare(subd, &row["subd"]));
        } else if let Geometry::Mesh(mesh) = object.geometry() {
            let Geometry::Mesh(expected) = geometry(row) else {
                panic!("native family differs")
            };
            assert_eq!(mesh.vertices().len(), expected.vertices().len());
            assert_eq!(mesh.faces().len(), expected.faces().len());
            let error = native::directed(mesh, &expected).max(native::directed(&expected, mesh));
            assert!(error <= 1e-6);
            comparisons.push(json!({"qualified":true,"mesh":true,"maximum_sampled_surface_error":error,"vertices":mesh.vertices().len(),"faces":mesh.faces().len()}));
        } else {
            panic!("unexpected face family")
        }
    }
    if phase == "undo"
        && let Some(id) = scene.source
    {
        assert!(
            scene
                .original
                .as_ref()
                .unwrap()
                .shares_storage_with(scene.document.object(id).unwrap().geometry_snapshot())
        );
    }
    json!({"qualified":true,"objects":comparisons,"attributes_selection_and_identity":true})
}

#[allow(dead_code)] // Command normal qualification shares this helper with getters.
pub fn normal_rows() -> Vec<Value> {
    let data: Value = serde_json::from_str(include_str!(
        "../../../../tools/rhino_oracle/observations/face_create_subd_normals.json"
    ))
    .unwrap();
    let rows = data["results"].as_array().unwrap();
    assert_eq!(rows.len(), 7);
    rows.clone()
}
#[allow(dead_code)] // The viewport independently checks how native normals are displayed.
pub fn check_normals(scene: &Scene, value: &Value) -> Value {
    let targets = value["after_script"].as_array().unwrap();
    let mut outputs = scene
        .document
        .objects()
        .filter(|o| Some(o.id()) != scene.source);
    let mut maximum = 0f64;
    let mut corners = 0usize;
    for target in targets {
        let object = if target["role"] == "source" {
            scene.document.object(scene.source.unwrap()).unwrap()
        } else {
            outputs.next().unwrap()
        };
        let Geometry::SubD(subd) = object.geometry() else {
            panic!("normal fixture requires SubD")
        };
        let display = subd.tessellate_with_normals(2).unwrap();
        let reference = &target["subd"];
        let mesh = &reference["surface_mesh"];
        let vertices = mesh["vertices"]
            .as_array()
            .unwrap()
            .iter()
            .map(point)
            .collect::<Vec<_>>();
        let normal_values = reference["surface_mesh_normals"].as_array().unwrap();
        assert_eq!(normal_values.len(), vertices.len());
        let faces = mesh["faces"]
            .as_array()
            .unwrap()
            .iter()
            .map(|f| {
                f.as_array()
                    .unwrap()
                    .iter()
                    .map(|i| i.as_u64().unwrap() as usize)
                    .collect::<Vec<_>>()
            })
            .collect::<Vec<_>>();
        assert_eq!(display.mesh.faces().len(), faces.len());
        assert_eq!(display.face_corner_normals.len(), faces.len());
        for (face, normals) in display.mesh.faces().iter().zip(display.face_corner_normals) {
            let indices = face.indices();
            let (matched, rotation) = faces
                .iter()
                .find_map(|candidate| {
                    (0..4)
                        .find(|&rotation| {
                            (0..4).all(|i| {
                                display.mesh.vertices()[indices[i] as usize]
                                    .distance_to(vertices[candidate[(rotation + i) % 4]])
                                    .unwrap()
                                    < 1e-6
                            })
                        })
                        .map(|rotation| (candidate, rotation))
                })
                .expect("matching native density-two quad");
            for (i, normal) in normals.into_iter().enumerate() {
                let expected = &normal_values[matched[(rotation + i) % 4]];
                let actual = normal.to_array();
                let error = (0..3)
                    .map(|k| (actual[k] - expected[k].as_f64().unwrap()).powi(2))
                    .sum::<f64>()
                    .sqrt();
                assert!(error <= 1e-6, "limit corner normal error {error}");
                maximum = maximum.max(error);
                corners += 1;
            }
        }
    }
    json!({"qualified":true,"maximum_corner_normal_error":maximum,"corners":corners})
}
