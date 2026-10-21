use serde_json::{Value, json};
use viboceros_document::{
    ColorRgb, Document, Geometry, GeometrySnapshot, ObjectAttributes, ObjectColorSource, ObjectId,
    SelectionMode,
};
use viboceros_geometry::{
    Circle3, Ellipse3, LineSegment, MeshFace, MeshProjectionIndex, Point3, Polyline3, Real,
    Tolerance, TriangleMesh, UnitVector3,
};

pub struct Scene {
    pub document: Document,
    pub sources: Vec<ObjectId>,
    pub parents: Vec<GeometrySnapshot>,
}
pub fn rows() -> Vec<Value> {
    let r: Value = serde_json::from_str(include_str!(
        "../../../../tools/rhino_oracle/observations/planar_mesh.json"
    ))
    .unwrap();
    let rows = r["results"].as_array().unwrap().clone();
    assert_eq!(rows.len(), 21);
    rows
}
pub fn post_rows() -> Vec<Value> {
    let r: Value = serde_json::from_str(include_str!(
        "../../../../tools/rhino_oracle/observations/planar_mesh_postselect.json"
    ))
    .unwrap();
    let rows = r["results"].as_array().unwrap().clone();
    assert_eq!(rows.len(), 3);
    rows
}
fn point(value: &Value) -> Point3 {
    Point3::try_from(serde_json::from_value::<[Real; 3]>(value.clone()).unwrap()).unwrap()
}
fn geometry(recipe: &Value) -> Geometry {
    match recipe["kind"].as_str().unwrap() {
        "polyline" | "nurbs" => {
            let p = Polyline3::try_new(
                recipe["points"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .map(point)
                    .collect(),
                Tolerance::NUMERICAL_VALIDATION,
            )
            .unwrap();
            if recipe["kind"] == "nurbs" {
                Geometry::NurbsCurve(p.to_nurbs().unwrap().try_change_degree(3, false).unwrap())
            } else {
                Geometry::Polyline(p)
            }
        }
        "line" => Geometry::Line(
            LineSegment::try_new(
                point(&recipe["points"][0]),
                point(&recipe["points"][1]),
                Tolerance::NUMERICAL_VALIDATION,
            )
            .unwrap(),
        ),
        "circle" => Geometry::Circle(
            Circle3::try_new(
                point(&recipe["center"]),
                recipe["radius"].as_f64().unwrap(),
                UnitVector3::try_new(0., 0., 1., Tolerance::NUMERICAL_VALIDATION).unwrap(),
                Tolerance::NUMERICAL_VALIDATION,
            )
            .unwrap(),
        ),
        "ellipse" => Geometry::Ellipse(
            Ellipse3::try_new(
                point(&recipe["center"]),
                recipe["radii"][0].as_f64().unwrap(),
                recipe["radii"][1].as_f64().unwrap(),
                UnitVector3::try_new(1., 0., 0., Tolerance::NUMERICAL_VALIDATION).unwrap(),
                UnitVector3::try_new(0., 1., 0., Tolerance::NUMERICAL_VALIDATION).unwrap(),
                Tolerance::NUMERICAL_VALIDATION,
            )
            .unwrap(),
        ),
        kind => panic!("unknown source {kind}"),
    }
}
pub fn setup(value: &Value) -> Scene {
    let mut document = Document::new(Tolerance::try_new(1e-6, 1e-12, 1e-6).unwrap());
    let source = document
        .add_layer("PlanarMesh source", ColorRgb::new(10, 20, 30))
        .unwrap();
    let output = document
        .add_layer("PlanarMesh output", ColorRgb::new(40, 50, 60))
        .unwrap();
    let sources = value["input"]["curves"]
        .as_array()
        .unwrap()
        .iter()
        .enumerate()
        .map(|(i, c)| {
            document
                .add_geometry_with_attributes(
                    geometry(c),
                    ObjectAttributes::on_layer(source)
                        .with_name(format!("Input {i}"))
                        .with_object_color(ColorRgb::new(37, 83, 149)),
                )
                .unwrap()
        })
        .collect::<Vec<_>>();
    let parents = sources
        .iter()
        .map(|id| document.object(*id).unwrap().geometry_snapshot().clone())
        .collect();
    document.set_current_layer(output).unwrap();
    document
        .select_objects_direct(
            if value["input"]["postselect"] == true {
                vec![]
            } else {
                sources.clone()
            },
            SelectionMode::Replace,
        )
        .unwrap();
    document.clear_history().unwrap();
    Scene {
        document,
        sources,
        parents,
    }
}
pub fn native_success(value: &Value) -> bool {
    value["events"]
        .as_array()
        .unwrap()
        .iter()
        .any(|e| e["name"] == "PlanarMesh" && e["result"] == "Success")
}
pub fn output_count(value: &Value) -> usize {
    value["after_script"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|o| o["role"] == "output")
        .count()
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
            .map(|f| {
                let indices: Vec<u32> = serde_json::from_value(f.clone()).unwrap();
                match indices.len() {
                    3 => MeshFace::Triangle(indices.try_into().unwrap()),
                    4 => MeshFace::Quad(indices.try_into().unwrap()),
                    _ => panic!("invalid native face"),
                }
            })
            .collect(),
        Tolerance::MESH_VALIDATION,
    )
    .unwrap()
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
        assert_eq!(doc.is_selected(*id), target["selected"].as_bool().unwrap());
        assert_eq!(object.attributes().name(), target["name"].as_str());
        assert_eq!(
            object.attributes().color_source(),
            ObjectColorSource::Object
        );
        assert_eq!(
            object.attributes().object_color(),
            ColorRgb::new(37, 83, 149)
        );
        assert_eq!(
            doc.layer(object.attributes().layer_id()).unwrap().name(),
            "PlanarMesh source"
        );
    }
    let mut expected = native
        .iter()
        .filter(|o| o["role"] == "output")
        .enumerate()
        .map(|(i, o)| (i, read_mesh(&o["mesh"])))
        .collect::<Vec<_>>();
    let native_meshes = expected.len();
    let mut comparisons = Vec::new();
    let mut maximum: Real = 0.;
    let actual = doc
        .objects()
        .filter(|o| !sources.contains(&o.id()))
        .collect::<Vec<_>>();
    let mut counts_match = actual.len() == expected.len();
    for object in &actual {
        assert!(!doc.is_selected(object.id()));
        assert!(object.attributes().name().is_none());
        assert_eq!(object.attributes().color_source(), ObjectColorSource::Layer);
        assert_eq!(
            doc.layer(object.attributes().layer_id()).unwrap().name(),
            "PlanarMesh output"
        );
        let Geometry::Mesh(mesh) = object.geometry() else {
            panic!("mesh output required")
        };
        let Some((index, _)) = expected
            .iter()
            .enumerate()
            .map(|(i, (_, target))| (i, bounds_error(mesh, target)))
            .min_by(|a, b| a.1.total_cmp(&b.1))
        else {
            continue;
        };
        let (native_index, target) = expected.remove(index);
        let error = directed(mesh, &target).max(directed(&target, mesh));
        maximum = maximum.max(error);
        let same_counts = mesh.vertices().len() == target.vertices().len()
            && mesh.faces().len() == target.faces().len()
            && mesh.faces().iter().filter(|f| f.is_triangle()).count()
                == target.faces().iter().filter(|f| f.is_triangle()).count();
        counts_match &= same_counts;
        comparisons.push(
            json!({"native_index":native_index,"maximum_bidirectional_sample_error":error,"counts_match":same_counts,
            "actual_vertices":mesh.vertices().len(),"native_vertices":target.vertices().len(),
            "actual_faces":mesh.faces().len(),"native_faces":target.faces().len(),
            "actual_triangles":mesh.faces().iter().filter(|f|f.is_triangle()).count(),
            "native_triangles":target.faces().iter().filter(|f|f.is_triangle()).count()}),
        );
    }
    let geometry = actual.len() == native_meshes && maximum <= 1e-6;
    json!({"qualified":geometry&&counts_match,"geometry_qualified":geometry,"counts_match":counts_match,
           "maximum_bidirectional_sample_error":if actual.len()==native_meshes{Some(maximum)}else{None},
           "actual_meshes":actual.len(),"native_meshes":native_meshes,"meshes":comparisons,
           "sources_and_attributes_preserved":true})
}
fn bounds_error(a: &TriangleMesh, b: &TriangleMesh) -> Real {
    a.bounds()
        .min()
        .to_array()
        .into_iter()
        .chain(a.bounds().max().to_array())
        .zip(
            b.bounds()
                .min()
                .to_array()
                .into_iter()
                .chain(b.bounds().max().to_array()),
        )
        .map(|(a, b)| (a - b).abs())
        .sum()
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
