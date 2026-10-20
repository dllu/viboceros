use serde_json::Value;
use viboceros_document::{ColorRgb, Document, Geometry, ObjectId, SelectionMode};
use viboceros_geometry::{
    Brep, NurbsCurve, NurbsSurface, Point3, Polyline3, Tolerance, WeightedPoint3,
};
pub fn rows() -> Vec<(Value, Value)> {
    let q: Value = serde_json::from_str(include_str!(
        "../../../../tools/rhino_oracle/fixtures/short_path_command.json"
    ))
    .unwrap();
    let r: Value = serde_json::from_str(include_str!(
        "../../../../tools/rhino_oracle/observations/short_path_command.json"
    ))
    .unwrap();
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
pub fn boundary_rows() -> Vec<(Value, Value)> {
    let q: Value = serde_json::from_str(include_str!(
        "../../../../tools/rhino_oracle/fixtures/short_path_boundaries.json"
    ))
    .unwrap();
    let r: Value = serde_json::from_str(include_str!(
        "../../../../tools/rhino_oracle/observations/short_path_boundaries.json"
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
            assert_eq!(row["value"]["input_trace"]["errors"], serde_json::json!([]));
            assert_eq!(row["value"]["success"], true);
            (op.clone(), row["value"].clone())
        })
        .collect()
}
pub fn setup_boundary(op: &Value, v: &Value) -> (Document, ObjectId) {
    let (mut doc, id) = setup(v);
    let polygons = v["boundaries"]
        .as_array()
        .unwrap()
        .iter()
        .map(|loop_| {
            let points = loop_
                .as_array()
                .unwrap()
                .iter()
                .map(|p| {
                    Point3::try_from(serde_json::from_value::<[f64; 3]>(p.clone()).unwrap())
                        .unwrap()
                })
                .collect();
            Polyline3::try_new(points, doc.tolerance())
                .unwrap()
                .to_nurbs()
                .unwrap()
        })
        .collect::<Vec<_>>();
    let mut brep =
        Brep::try_planar_face_with_holes(&polygons[0], &polygons[1..], doc.tolerance()).unwrap();
    if op["case"] == "hole_reversed" {
        brep.reverse_orientation();
    }
    doc.replace_object_geometries([(id, Geometry::Brep(brep))])
        .unwrap();
    doc.clear_history().unwrap();
    (doc, id)
}
fn controls(v: &Value) -> Vec<WeightedPoint3> {
    v.as_array()
        .unwrap()
        .iter()
        .map(|c| {
            WeightedPoint3::try_new(
                Point3::try_from(serde_json::from_value::<[f64; 3]>(c["point"].clone()).unwrap())
                    .unwrap(),
                c["weight"].as_f64().unwrap(),
            )
            .unwrap()
        })
        .collect()
}
fn curve(v: &Value) -> NurbsCurve {
    NurbsCurve::try_new_rational(
        v["degree"].as_u64().unwrap() as usize,
        controls(&v["control_points"]),
        serde_json::from_value(v["knots"].clone()).unwrap(),
    )
    .unwrap()
}
pub fn setup(v: &Value) -> (Document, ObjectId) {
    let mut doc = Document::new(Tolerance::try_new(1e-6, 1e-12, 1e-6).unwrap());
    let source = doc
        .add_layer("ShortPath source", ColorRgb::new(10, 20, 30))
        .unwrap();
    let output = doc
        .add_layer("ShortPath output", ColorRgb::new(40, 50, 60))
        .unwrap();
    doc.set_current_layer(source).unwrap();
    let s = &v["surface"];
    let surface = NurbsSurface::try_new_rational(
        s["degree"][0].as_u64().unwrap() as usize,
        s["degree"][1].as_u64().unwrap() as usize,
        s["control_count"][0].as_u64().unwrap() as usize,
        s["control_count"][1].as_u64().unwrap() as usize,
        controls(&s["control_points"]),
        serde_json::from_value(s["knots_u"].clone()).unwrap(),
        serde_json::from_value(s["knots_v"].clone()).unwrap(),
    )
    .unwrap();
    let id = doc.add_geometry(Geometry::NurbsSurface(surface)).unwrap();
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
    let mut maximum = 0_f64;
    for (a, b) in actual.into_iter().zip(expected) {
        let role = if a.id() == target { "target" } else { "output" };
        assert_eq!(role, b["role"].as_str().unwrap());
        assert_eq!(
            doc.is_selected(a.id()),
            b["selected"].as_bool().unwrap(),
            "{phase} {role}"
        );
        assert_eq!(a.attributes().name(), b["name"].as_str());
        assert_eq!(
            doc.layer(a.attributes().layer_id()).unwrap().name(),
            b["layer"].as_str().unwrap()
        );
        if role == "target" {
            continue;
        }
        let current = a.geometry().curve_ref().unwrap().to_nurbs().unwrap();
        let native = curve(&b["curve"]);
        for (from, to) in [(&current, &native), (&native, &current)] {
            for i in 0..=64 {
                let p = from
                    .evaluate(from.parameter_at(i as f64 / 64.).unwrap())
                    .unwrap();
                let t = to
                    .closest_parameter(p, Tolerance::try_new(1e-10, 1e-12, 1e-9).unwrap())
                    .unwrap();
                maximum = maximum.max(p.distance_to(to.evaluate(t).unwrap()).unwrap());
            }
        }
    }
    maximum
}
