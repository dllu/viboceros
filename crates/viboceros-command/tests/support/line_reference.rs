//! Prescribed source geometry is built independently of captured outputs.
use super::native;
use serde_json::Value;
use viboceros_command::CommandRegistry;
use viboceros_document::{Document, Geometry, ObjectId};
use viboceros_geometry::{
    Circle3, CircularArc3, CurveSegment3, LineSegment, NurbsCurve, PolyCurve3, Polyline3,
    Tolerance, Vector3, WeightedPoint3,
};
pub const FAMILIES: [&str; 4] = [
    "line_reference",
    "line_reference_holdouts",
    "line_reference_completion",
    "line_reference_final",
];
pub fn sources() -> Value {
    serde_json::from_str(include_str!(
        "../../../../tools/rhino_oracle/fixtures/line_reference_sources.json"
    ))
    .unwrap()
}
pub fn geometry(v: &Value) -> Geometry {
    let p = native::point;
    let t = Tolerance::NUMERICAL_VALIDATION;
    match v["type"].as_str().unwrap() {
        "polycurve" => {
            let segments: Vec<CurveSegment3> = v["segments"]
                .as_array()
                .unwrap()
                .iter()
                .map(|s| match geometry(s) {
                    Geometry::Line(c) => c.into(),
                    Geometry::Arc(c) => c.into(),
                    Geometry::Polyline(c) => c.into(),
                    Geometry::NurbsCurve(c) => c.into(),
                    other => panic!("unsupported leaf: {other:?}"),
                })
                .collect();
            Geometry::PolyCurve(PolyCurve3::try_new(segments).unwrap())
        }
        "line" => Geometry::Line(LineSegment::try_new(p(&v["start"]), p(&v["end"]), t).unwrap()),
        "arc" => Geometry::Arc(
            CircularArc3::try_from_three_points(
                p(&v["points"][0]),
                p(&v["points"][1]),
                p(&v["points"][2]),
                t,
            )
            .unwrap(),
        ),
        "nurbs" => {
            let mut knots: Vec<f64> = serde_json::from_value(v["knots"].clone()).unwrap();
            // The public Native setter consumes full_knots[1..len-1]. Its
            // reader pads the two unused outer entries from their neighbours.
            // Reproduce that input contract without consulting Native output.
            let last = knots.len() - 1;
            knots[0] = knots[1];
            knots[last] = knots[last - 1];
            Geometry::NurbsCurve(
                NurbsCurve::try_new_rational(
                    v["degree"].as_u64().unwrap() as usize,
                    v["control_points"]
                        .as_array()
                        .unwrap()
                        .iter()
                        .map(|q| {
                            WeightedPoint3::try_new(p(&q["point"]), q["weight"].as_f64().unwrap())
                                .unwrap()
                        })
                        .collect(),
                    knots,
                )
                .unwrap(),
            )
        }
        "polyline" => Geometry::Polyline(
            Polyline3::try_new(v["vertices"].as_array().unwrap().iter().map(p).collect(), t)
                .unwrap(),
        ),
        "circle" => {
            let unit = |v: &Value| {
                Vector3::try_from(p(v).to_array())
                    .unwrap()
                    .normalized_nonzero()
                    .unwrap()
            };
            let c = Circle3::try_from_frame(
                p(&v["center"]),
                v["radius"].as_f64().unwrap(),
                unit(&v["x_axis"]),
                unit(&v["normal"]),
                t,
            )
            .unwrap();
            Geometry::Arc(CircularArc3::try_from_circle_sweep(c, std::f64::consts::TAU).unwrap())
        }
        other => panic!("unsupported prescribed Line reference: {other}"),
    }
}
pub fn setup(v: &Value, source: &Value) -> Document {
    let mut doc = native::scene(v);
    let tol = &v["model_tolerance"];
    doc.set_tolerance(
        Tolerance::try_new(
            tol["absolute"].as_f64().unwrap(),
            tol["relative"].as_f64().unwrap(),
            tol["angular"].as_f64().unwrap(),
        )
        .unwrap(),
    );
    for (i, definition) in source["curves"]
        .as_array()
        .into_iter()
        .flatten()
        .enumerate()
    {
        let id = guide(&doc, i);
        doc.replace_object_geometries([(id, geometry(definition))])
            .unwrap();
    }
    doc.clear_undo_records().unwrap();
    if source["pending_redo"] == true {
        let r = CommandRegistry::with_builtins();
        r.execute(&mut doc, "Point 90,91,92").unwrap();
        doc.undo().unwrap();
    }
    doc
}
pub fn guide(doc: &Document, index: usize) -> ObjectId {
    let name = format!("guide{index}");
    doc.objects()
        .find(|o| o.attributes().name() == Some(name.as_str()))
        .unwrap()
        .id()
}
pub fn token(value: &Value) -> String {
    if let Some(p) = value.as_array() {
        p.iter()
            .map(|n| n.as_f64().unwrap().to_string())
            .collect::<Vec<_>>()
            .join(",")
    } else if let Some(s) = value.as_str() {
        s.into()
    } else {
        value.to_string()
    }
}
pub fn command(doc: &Document, source: &Value) -> String {
    let mut tokens = vec!["Line".into()];
    for value in source["actions"].as_array().unwrap() {
        if value.is_object() {
            tokens.push(format!(
                "Curve={}",
                guide(doc, value["curve"].as_u64().unwrap() as usize)
            ));
            tokens.push(token(&value["pick"]));
        } else {
            tokens.push(token(value));
        }
    }
    tokens.join(" ")
}
