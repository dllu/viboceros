//! Public command captures, shared by command execution and viewport getters.
use crate::morph_fixtures as morph;
use serde_json::Value;
use viboceros_document::{ColorRgb, Document, Geometry, GeometrySnapshot, ObjectId, SelectionMode};
use viboceros_geometry::{Brep, NurbsSurface, Point3, Polyline3, Tolerance};

pub fn rows() -> Vec<(Value, Value)> {
    let q: Value = serde_json::from_str(include_str!(
        "../../../../tools/rhino_oracle/fixtures/surface_offset_command.json"
    ))
    .unwrap();
    let r: Value = serde_json::from_str(include_str!(
        "../../../../tools/rhino_oracle/observations/surface_offset_command.json"
    ))
    .unwrap();
    assert_eq!(q["operations"].as_array().unwrap().len(), 8);
    assert_eq!(r["results"].as_array().unwrap().len(), 8);
    q["operations"]
        .as_array()
        .unwrap()
        .iter()
        .zip(r["results"].as_array().unwrap())
        .map(|(op, row)| {
            assert_eq!(op["id"], row["id"]);
            assert!(row["error"].is_null());
            let v = &row["value"];
            assert_eq!(v["success"], true);
            assert_eq!(v["input_trace"]["errors"], serde_json::json!([]));
            assert_eq!(v["input_trace"]["completed"], v["input_trace"]["expected"]);
            (op.clone(), v.clone())
        })
        .collect()
}
pub struct Scene {
    pub source: ObjectId,
    pub target: ObjectId,
    parents: [GeometrySnapshot; 2],
}
fn p(x: f64, y: f64) -> Point3 {
    Point3::try_new(x, y, 0.).unwrap()
}
pub fn setup(op: &Value, v: &Value) -> (Document, Scene) {
    let mut doc = Document::new(Tolerance::try_new(1e-6, 1e-12, 1e-6).unwrap());
    let default = doc.current_layer_id();
    let source_layer = doc
        .add_layer("Surface offset source", ColorRgb::new(10, 20, 30))
        .unwrap();
    let output_layer = doc
        .add_layer("Surface offset output", ColorRgb::new(40, 50, 60))
        .unwrap();
    doc.set_current_layer(source_layer).unwrap();
    let source = doc
        .add_geometry(Geometry::NurbsCurve(morph::curve(&v["before"][0]["curve"])))
        .unwrap();
    doc.set_current_layer(default).unwrap();
    let geometry = if op["case"].as_str().unwrap().starts_with("trimmed_") {
        let polygon = |points| {
            Polyline3::try_new(points, doc.tolerance())
                .unwrap()
                .to_nurbs()
                .unwrap()
        };
        let outer = polygon(vec![
            p(-5., -5.),
            p(5., -5.),
            p(5., 5.),
            p(-5., 5.),
            p(-5., -5.),
        ]);
        let hole = polygon(vec![
            p(-1., 0.5),
            p(1., 0.5),
            p(1., 1.5),
            p(-1., 1.5),
            p(-1., 0.5),
        ]);
        Geometry::Brep(Brep::try_planar_face_with_holes(&outer, &[hole], doc.tolerance()).unwrap())
    } else {
        Geometry::Brep(
            Brep::try_surface_face(
                NurbsSurface::try_bilinear([
                    p(-10., -10.),
                    p(10., -10.),
                    p(10., 10.),
                    p(-10., 10.),
                ])
                .unwrap(),
                doc.tolerance(),
            )
            .unwrap(),
        )
    };
    let target = doc.add_geometry(geometry).unwrap();
    doc.set_object_names([
        (source, Some("Source".into())),
        (target, Some("Target".into())),
    ])
    .unwrap();
    if v["before"][0]["selected"] == true {
        doc.select_object(source, SelectionMode::Replace).unwrap();
    }
    doc.set_current_layer(output_layer).unwrap();
    doc.clear_history().unwrap();
    let parents = [
        doc.object(source).unwrap().geometry_snapshot().clone(),
        doc.object(target).unwrap().geometry_snapshot().clone(),
    ];
    (
        doc,
        Scene {
            source,
            target,
            parents,
        },
    )
}
pub fn number(v: &Value) -> usize {
    v["input_trace"]["prompts"]
        .as_array()
        .unwrap()
        .last()
        .unwrap()
        .as_str()
        .unwrap()
        .split("Number=")
        .nth(1)
        .unwrap()
        .split(|c: char| !c.is_ascii_digit())
        .next()
        .unwrap()
        .parse()
        .unwrap()
}
pub fn check(doc: &Document, scene: &Scene, v: &Value, phase: &str) -> f64 {
    let expected = v[phase].as_array().unwrap();
    let actual = doc.objects().collect::<Vec<_>>();
    assert_eq!(actual.len(), expected.len(), "{phase}");
    let mut maximum = 0f64;
    for (a, b) in actual.into_iter().zip(expected) {
        let role = if a.id() == scene.source {
            "source"
        } else if a.id() == scene.target {
            "target"
        } else {
            "output"
        };
        assert_eq!(role, b["role"].as_str().unwrap());
        assert_eq!(doc.is_selected(a.id()), b["selected"].as_bool().unwrap());
        assert_eq!(a.attributes().name(), b["name"].as_str());
        assert_eq!(
            doc.layer(a.attributes().layer_id()).unwrap().name(),
            b["layer"].as_str().unwrap()
        );
        if role != "output" {
            let index = usize::from(role == "target");
            assert!(scene.parents[index].shares_storage_with(a.geometry_snapshot()));
            continue;
        }
        let native = morph::curve(&b["curve"]);
        let current = a.geometry().curve_ref().unwrap().to_nurbs().unwrap();
        for (from, to) in [(&current, &native), (&native, &current)] {
            for i in 0..=128 {
                let point = from
                    .evaluate(from.parameter_at(i as f64 / 128.).unwrap())
                    .unwrap();
                let t = to.closest_parameter(point, doc.tolerance()).unwrap();
                maximum = maximum.max(point.distance_to(to.evaluate(t).unwrap()).unwrap());
            }
        }
        if current.degree() == 1 && native.degree() == 1 {
            for (a, b) in [
                (*current.domain().start(), *native.domain().start()),
                (*current.domain().end(), *native.domain().end()),
            ] {
                assert!((a - b).abs() < 1e-6, "{phase}: domain {a} != {b}");
            }
        }
    }
    maximum
}
pub fn qualify(op: &Value, maximum: f64) {
    // A diagnostic ceiling does not qualify the captured Bézier offset.
    let ceiling = if op["case"] == "bezier" { 1.3e-5 } else { 1e-6 };
    assert!(maximum < ceiling, "{}: {maximum:e}", op["id"]);
}
