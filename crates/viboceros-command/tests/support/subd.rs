use serde_json::{Value, json};
use viboceros_document::{Document, Geometry};
use viboceros_geometry::{MeshFace, MeshProjectionIndex, Point3, SubD, TriangleMesh};

pub fn rows() -> Vec<Value> {
    let mut result = Vec::new();
    for source in [
        include_str!("../../../../tools/rhino_oracle/observations/face_create_subd.json"),
        include_str!("../../../../tools/rhino_oracle/observations/face_create_subd_surface.json"),
    ] {
        let response: Value = serde_json::from_str(source).unwrap();
        let rows = response["results"].as_array().unwrap();
        assert_eq!(rows.len(), 10);
        result.extend(rows.iter().cloned());
    }
    result
}
pub fn setup() -> Document {
    let mut document = Document::default();
    let layer = document
        .add_layer(
            "3DFace output",
            viboceros_document::ColorRgb::new(40, 50, 60),
        )
        .unwrap();
    document.set_current_layer(layer).unwrap();
    document.clear_history().unwrap();
    document
}
pub fn header(value: &Value) -> String {
    let c = &value["input"];
    format!(
        "3DFace Output=SubD Interpolated=No PolygonType={} InPlane={} Mode={}",
        c["polygon"].as_str().unwrap_or("Quad"),
        if c["in_plane"] == true { "Yes" } else { "No" },
        if c["multiple"] == true {
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
            p.as_array()
                .unwrap()
                .iter()
                .map(|x| x.as_f64().unwrap().to_string())
                .collect::<Vec<_>>()
                .join(",")
        })
        .collect()
}
#[allow(dead_code)] // The command harness and getter harness share this helper.
pub fn command(value: &Value) -> String {
    format!("{} {} Enter", header(value), actions(value).join(" "))
}
pub fn check(document: &Document, value: &Value) -> Value {
    assert_eq!(document.objects().count(), 1);
    let object = document.objects().next().unwrap();
    let Geometry::SubD(subd) = object.geometry() else {
        panic!("expected a native SubD object")
    };
    let expected = &value["after_script"][0];
    assert_eq!(
        document
            .layer(object.attributes().layer_id())
            .unwrap()
            .name(),
        expected["layer"].as_str().unwrap()
    );
    assert_eq!(object.attributes().name(), expected["name"].as_str());
    assert_eq!(
        document.is_selected(object.id()),
        expected["selected"].as_bool().unwrap()
    );
    let mut result = compare(subd, &expected["subd"]);
    result["attributes_and_selection"] = json!(true);
    result
}
pub fn compare(subd: &SubD, native: &Value) -> Value {
    assert_eq!(native["valid"], true);
    let points = native["points"].as_array().unwrap();
    assert_eq!(subd.control_points().len(), points.len());
    let point = |p: &Value| {
        Point3::try_new(
            p[0].as_f64().unwrap(),
            p[1].as_f64().unwrap(),
            p[2].as_f64().unwrap(),
        )
        .unwrap()
    };
    let mut permutation = Vec::new();
    let mut maximum = 0f64;
    for (i, &p) in subd.control_points().iter().enumerate() {
        let (j, distance) = points
            .iter()
            .enumerate()
            .map(|(j, q)| (j, p.distance_to(point(q)).unwrap()))
            .min_by(|a, b| a.1.total_cmp(&b.1))
            .unwrap();
        assert!(!permutation.contains(&j));
        permutation.push(j);
        maximum = maximum.max(distance);
        assert_eq!(
            subd.vertex_tags()[i] as u64,
            native["vertex_tags"][j].as_u64().unwrap()
        );
        maximum = maximum.max(
            subd.surface_points()[i]
                .distance_to(point(&native["surface_points"][j]))
                .unwrap(),
        );
    }
    let cyclic = |face: Vec<usize>| {
        (0..face.len())
            .map(|offset| {
                face[offset..]
                    .iter()
                    .chain(&face[..offset])
                    .copied()
                    .collect::<Vec<_>>()
            })
            .min()
            .unwrap()
    };
    let mut actual = subd
        .faces()
        .iter()
        .map(|f| cyclic(f.iter().map(|&i| permutation[i as usize]).collect()))
        .collect::<Vec<_>>();
    let mut expected = native["faces"]
        .as_array()
        .unwrap()
        .iter()
        .map(|f| {
            cyclic(
                f.as_array()
                    .unwrap()
                    .iter()
                    .map(|i| i.as_u64().unwrap() as usize)
                    .collect(),
            )
        })
        .collect::<Vec<_>>();
    actual.sort();
    expected.sort();
    assert_eq!(actual, expected);
    let mut edges = subd
        .edges()
        .iter()
        .zip(subd.edge_tags())
        .map(|(e, &tag)| {
            let a = permutation[e[0] as usize];
            let b = permutation[e[1] as usize];
            (a.min(b), a.max(b), tag as u64)
        })
        .collect::<Vec<_>>();
    let mut native_edges = native["edges"]
        .as_array()
        .unwrap()
        .iter()
        .zip(native["edge_tags"].as_array().unwrap())
        .map(|(e, tag)| {
            let a = e[0].as_u64().unwrap() as usize;
            let b = e[1].as_u64().unwrap() as usize;
            (a.min(b), a.max(b), tag.as_u64().unwrap())
        })
        .collect::<Vec<_>>();
    edges.sort();
    native_edges.sort();
    assert_eq!(edges, native_edges);
    let mut surface_error = 0f64;
    if let Some(mesh) = native.get("surface_mesh") {
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
        let native_mesh = TriangleMesh::try_from_face_records(vertices, faces).unwrap();
        let mesh = subd.tessellate(2).unwrap();
        assert_eq!(mesh.faces().len(), native_mesh.faces().len());
        surface_error = directed(&mesh, &native_mesh).max(directed(&native_mesh, &mesh));
        assert!(surface_error <= 1e-6, "limit mesh error {surface_error}");
    }
    assert!(maximum <= 1e-6, "{maximum}");
    json!({"qualified":true,"maximum_control_or_limit_point_error":maximum,"maximum_sampled_surface_error":surface_error,"surface_sampled":native.get("surface_mesh").is_some(),"vertices":points.len(),"faces":expected.len(),"edges":edges.len()})
}

fn directed(a: &TriangleMesh, b: &TriangleMesh) -> f64 {
    let target = MeshProjectionIndex::new(b).unwrap();
    let source = MeshProjectionIndex::new(a).unwrap();
    let mut maximum = 0f64;
    for face in 0..a.triangles().len() {
        let [a, b, c] = source.triangle_points(face).unwrap().map(|p| p.to_array());
        for i in 0..=4 {
            for j in 0..=4 - i {
                let u = i as f64 / 4.;
                let v = j as f64 / 4.;
                let p = Point3::try_from(std::array::from_fn::<_, 3, _>(|k| {
                    (b[k] - a[k]).mul_add(u, (c[k] - a[k]).mul_add(v, a[k]))
                }))
                .unwrap();
                maximum = maximum.max(
                    p.distance_to(target.closest_point(p).unwrap().unwrap())
                        .unwrap(),
                );
            }
        }
    }
    maximum
}
