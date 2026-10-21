use serde_json::Value;
use viboceros_document::{ColorRgb, Document, Geometry, GeometrySnapshot, ObjectId, SelectionMode};
use viboceros_geometry::{
    Circle3, CurveRef, MeshProjectionIndex, NurbsSurface, Point3, PointCloud3, Polyline3, Real,
    Tolerance, TriangleMesh, Vector3,
};
pub struct Scene {
    pub document: Document,
    pub sources: Vec<ObjectId>,
    pub holes: Vec<ObjectId>,
    pub surface: Option<ObjectId>,
    pub parents: Vec<(ObjectId, GeometrySnapshot)>,
}
pub fn rows() -> Vec<(Value, Value)> {
    captured(
        include_str!("../../../../tools/rhino_oracle/fixtures/mesh_patch.json"),
        include_str!("../../../../tools/rhino_oracle/observations/mesh_patch.json"),
        21,
    )
}
pub fn crossing_groups() -> Vec<(&'static str, Vec<(Value, Value)>)> {
    vec![
        (
            "CROSSING",
            captured(
                include_str!("../../../../tools/rhino_oracle/fixtures/mesh_patch_crossings.json"),
                include_str!(
                    "../../../../tools/rhino_oracle/observations/mesh_patch_crossings.json"
                ),
                11,
            ),
        ),
        (
            "VERIFY",
            captured(
                include_str!(
                    "../../../../tools/rhino_oracle/fixtures/mesh_patch_crossings_verify.json"
                ),
                include_str!(
                    "../../../../tools/rhino_oracle/observations/mesh_patch_crossings_verify.json"
                ),
                21,
            ),
        ),
        (
            "UNTRIMMED",
            captured(
                include_str!(
                    "../../../../tools/rhino_oracle/fixtures/mesh_patch_crossings_untrimmed.json"
                ),
                include_str!(
                    "../../../../tools/rhino_oracle/observations/mesh_patch_crossings_untrimmed.json"
                ),
                9,
            ),
        ),
        (
            "SEEDED",
            captured(
                include_str!(
                    "../../../../tools/rhino_oracle/fixtures/mesh_patch_crossings_seeded.json"
                ),
                include_str!(
                    "../../../../tools/rhino_oracle/observations/mesh_patch_crossings_seeded.json"
                ),
                7,
            ),
        ),
    ]
}
fn captured(q: &str, r: &str, count: usize) -> Vec<(Value, Value)> {
    let q: Value = serde_json::from_str(q).unwrap();
    let r: Value = serde_json::from_str(r).unwrap();
    assert_eq!(q["operations"].as_array().unwrap().len(), count);
    assert_eq!(r["results"].as_array().unwrap().len(), count);
    q["operations"]
        .as_array()
        .unwrap()
        .iter()
        .zip(r["results"].as_array().unwrap())
        .map(|(op, r)| {
            assert_eq!(op["id"], r["id"]);
            assert!(r.get("error").is_none());
            (op.clone(), r["value"].clone())
        })
        .collect()
}
pub fn assert_known_gaps(failures: &[String]) {
    let known = [
        "shared_endpoint",
        "overlap",
        "self_cross",
        "cross_h_reverse",
        "two_crossings",
        "cross_seed_axis",
    ];
    let unexpected = failures
        .iter()
        .filter(|id| !known.contains(&id.as_str()))
        .collect::<Vec<_>>();
    assert!(
        unexpected.is_empty(),
        "new MeshPatch compatibility gaps: {unexpected:?}"
    );
}
fn point(v: &Value) -> Point3 {
    Point3::try_from(serde_json::from_value::<[Real; 3]>(v.clone()).unwrap()).unwrap()
}
pub fn surface(v: &Value) -> Option<NurbsSurface> {
    v["input"]["surface"].as_array().map(|corners| {
        let c: Vec<_> = corners.iter().map(point).collect();
        let s = &v["surface"];
        NurbsSurface::try_bilinear(c.try_into().unwrap())
            .unwrap()
            .try_reparameterized(
                s["domain_u"][0].as_f64().unwrap()..=s["domain_u"][1].as_f64().unwrap(),
                s["domain_v"][0].as_f64().unwrap()..=s["domain_v"][1].as_f64().unwrap(),
            )
            .unwrap()
    })
}
pub fn setup(v: &Value) -> Scene {
    let mut document = Document::new(Tolerance::try_new(1e-6, 1e-12, 1e-6).unwrap());
    let source = document
        .add_layer("MeshPatch source", ColorRgb::new(10, 20, 30))
        .unwrap();
    let output = document
        .add_layer("MeshPatch output", ColorRgb::new(40, 50, 60))
        .unwrap();
    document.set_current_layer(source).unwrap();
    let input = &v["input"];
    let mut sources = Vec::new();
    let mut holes = Vec::new();
    let mut add = |g: Geometry| {
        let id = document.add_geometry(g).unwrap();
        document
            .set_object_names([(id, Some("Input".to_owned()))])
            .unwrap();
        id
    };
    let points = input["points"]
        .as_array()
        .map(|p| p.iter().map(point).collect::<Vec<_>>())
        .unwrap_or_default();
    if input["cloud"] == true {
        sources.push(add(Geometry::PointCloud(
            PointCloud3::try_new(points).unwrap(),
        )));
    } else {
        for p in points {
            sources.push(add(Geometry::Point(p)));
        }
    }
    if let Some(curves) = input["curves"].as_array() {
        for curve in curves {
            sources.push(add(Geometry::Polyline(
                Polyline3::try_new(
                    curve.as_array().unwrap().iter().map(point).collect(),
                    Tolerance::NUMERICAL_VALIDATION,
                )
                .unwrap(),
            )));
        }
    }
    if let Some(c) = input["circle"].as_array() {
        let circle = Circle3::try_new(
            Point3::try_new(
                c[0].as_f64().unwrap(),
                c[1].as_f64().unwrap(),
                c[2].as_f64().unwrap(),
            )
            .unwrap(),
            c[3].as_f64().unwrap(),
            Vector3::try_new(0., 0., 1.)
                .unwrap()
                .normalized_nonzero()
                .unwrap(),
            Tolerance::DEFAULT,
        )
        .unwrap();
        sources.push(add(Geometry::NurbsCurve(
            CurveRef::Circle(&circle).to_nurbs().unwrap(),
        )));
    }
    if let Some(curves) = input["holes"].as_array() {
        for curve in curves {
            holes.push(add(Geometry::Polyline(
                Polyline3::try_new(
                    curve.as_array().unwrap().iter().map(point).collect(),
                    Tolerance::NUMERICAL_VALIDATION,
                )
                .unwrap(),
            )));
        }
    }
    let surface = surface(v).map(|s| add(Geometry::NurbsSurface(s)));
    let parents = sources
        .iter()
        .chain(&holes)
        .chain(surface.iter())
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
        holes,
        surface,
        parents,
    }
}
pub fn command(scene: &Scene, v: &Value) -> String {
    let input = &v["input"];
    let mut s = format!(
        "MeshPatch AngleTolerance=15 TrimBack={} MainSegmentCount={} Sources={}",
        if input["trimback"] == false {
            "No"
        } else {
            "Yes"
        },
        input["divisions"].as_u64().unwrap_or(1),
        scene
            .sources
            .iter()
            .map(ToString::to_string)
            .collect::<Vec<_>>()
            .join(",")
    );
    if !scene.holes.is_empty() {
        s.push_str(&format!(
            " Holes={}",
            scene
                .holes
                .iter()
                .map(ToString::to_string)
                .collect::<Vec<_>>()
                .join(",")
        ));
    }
    if let Some(id) = scene.surface {
        s.push_str(&format!(" StartingSurface={id}"));
    }
    s
}
fn mesh(v: &Value) -> TriangleMesh {
    TriangleMesh::try_new(
        v["vertices"]
            .as_array()
            .unwrap()
            .iter()
            .map(point)
            .collect(),
        serde_json::from_value(v["faces"].clone()).unwrap(),
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
pub fn check(
    doc: &Document,
    parents: &[(ObjectId, GeometrySnapshot)],
    v: &Value,
    phase: &str,
) -> Real {
    let result = compare(doc, parents, v, phase);
    assert!(result.counts_match);
    result.error
}
#[derive(Default)]
pub struct Comparison {
    pub error: Real,
    pub counts_match: bool,
}
impl Comparison {
    pub fn merge(&mut self, other: Self) {
        self.error = self.error.max(other.error);
        self.counts_match &= other.counts_match;
    }
    pub fn qualified(&self) -> bool {
        self.error < 1e-6 && self.counts_match
    }
}
pub fn compare(
    doc: &Document,
    parents: &[(ObjectId, GeometrySnapshot)],
    v: &Value,
    phase: &str,
) -> Comparison {
    let expected = v[phase].as_array().unwrap();
    assert_eq!(doc.objects().count(), expected.len());
    let native_sources = expected
        .iter()
        .filter(|o| o["role"] != "output")
        .collect::<Vec<_>>();
    assert_eq!(native_sources.len(), parents.len());
    // Scene insertion follows the captured native runtime-serial order. Object
    // iteration may use a different ID order, so retain that order explicitly.
    for ((id, parent), native) in parents.iter().zip(native_sources) {
        let o = doc.object(*id).unwrap();
        assert!(parent.shares_storage_with(o.geometry_snapshot()));
        attributes(doc, *id, native);
    }
    let outputs = doc
        .objects()
        .filter(|o| !parents.iter().any(|(id, _)| *id == o.id()))
        .collect::<Vec<_>>();
    let native_outputs = expected
        .iter()
        .filter(|o| o["role"] == "output")
        .collect::<Vec<_>>();
    assert_eq!(outputs.len(), native_outputs.len());
    let mut result = Comparison {
        error: 0.,
        counts_match: true,
    };
    for (o, native) in outputs.into_iter().zip(native_outputs) {
        attributes(doc, o.id(), native);
        let Geometry::Mesh(actual) = o.geometry() else {
            panic!("mesh output required")
        };
        let expected = mesh(&native["mesh"]);
        result.counts_match &= actual.vertices().len() == expected.vertices().len()
            && actual.faces().len() == expected.faces().len();
        result.error = result
            .error
            .max(directed(actual, &expected))
            .max(directed(&expected, actual));
    }
    result
}
fn attributes(doc: &Document, id: ObjectId, native: &Value) {
    let o = doc.object(id).unwrap();
    assert_eq!(doc.is_selected(id), native["selected"].as_bool().unwrap());
    assert_eq!(o.attributes().name(), native["name"].as_str());
    assert_eq!(
        doc.layer(o.attributes().layer_id()).unwrap().name(),
        native["layer"].as_str().unwrap()
    );
}
