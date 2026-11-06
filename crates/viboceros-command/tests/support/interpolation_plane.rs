//! Strict native document checks with whole-span bidirectional curve samples.
use serde_json::{Value, json};
use viboceros_document::{
    ColorRgb, Document, Geometry, ObjectAttributes, ObjectColorSource, ObjectId,
};
use viboceros_geometry::{NurbsCurve, Point3, Tolerance, WeightedPoint3};
pub fn read(name: &str) -> Value {
    use std::io::Read;
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    let base = if root.join("tools/rhino_oracle").is_dir() {
        root.join("tools/rhino_oracle/observations")
    } else {
        root.join("../../tools/rhino_oracle/observations")
    };
    let path = base.join(format!("{name}.json"));
    let bytes = if path.exists() {
        std::fs::read(path).unwrap()
    } else {
        let file = std::fs::File::open(base.join(format!("{name}.json.gz"))).unwrap();
        let mut bytes = Vec::new();
        flate2::read::GzDecoder::new(file)
            .read_to_end(&mut bytes)
            .unwrap();
        bytes
    };
    serde_json::from_slice(&bytes).unwrap()
}
pub fn curve(v: &Value) -> NurbsCurve {
    let result = NurbsCurve::try_new_rational(
        v["degree"].as_u64().unwrap() as usize,
        v["control_points"]
            .as_array()
            .unwrap()
            .iter()
            .map(|p| {
                WeightedPoint3::try_new(
                    Point3::try_from(
                        serde_json::from_value::<[f64; 3]>(p["point"].clone()).unwrap(),
                    )
                    .unwrap(),
                    p["weight"].as_f64().unwrap(),
                )
                .unwrap()
            })
            .collect(),
        serde_json::from_value(v["knots"].clone()).unwrap(),
    )
    .unwrap();
    assert_eq!(
        serde_json::to_value([*result.domain().start(), *result.domain().end()]).unwrap(),
        v["domain"]
    );
    result
}
pub fn setup(v: &Value) -> (Document, Option<ObjectId>) {
    let mut doc = Document::new(Tolerance::try_new(1e-6, 1e-12, 1e-6).unwrap());
    let input = doc
        .add_layer("Interpolation plane source", ColorRgb::BLACK)
        .unwrap();
    let output = doc
        .add_layer("Interpolation plane output", ColorRgb::BLACK)
        .unwrap();
    let ids = v["before"]["objects"]
        .as_array()
        .unwrap()
        .iter()
        .map(|source| {
            doc.add_geometry_with_attributes(
                Geometry::NurbsCurve(curve(&source["curve"])),
                ObjectAttributes::on_layer(input)
                    .with_name(source["attributes"]["name"].as_str().unwrap()),
            )
            .unwrap()
        })
        .collect::<Vec<_>>();
    if !ids.is_empty() {
        doc.add_group(
            Some("Interpolation plane sources".into()),
            ids.iter().copied(),
        )
        .unwrap();
    }
    doc.set_current_layer(output).unwrap();
    doc.clear_history().unwrap();
    (doc, ids.first().copied())
}
pub fn command(v: &Value, definition: &str) -> String {
    let mut points: Vec<[f64; 3]> = Vec::new();
    for r in v["input_trace"][0]["sent"].as_array().unwrap() {
        let s = r["token"].as_str().unwrap();
        if s == "_Undo" {
            points.pop();
        } else if let Some(s) = s.strip_prefix('w') {
            points.push(
                s.split(',')
                    .map(|s| s.parse::<f64>().unwrap())
                    .collect::<Vec<_>>()
                    .try_into()
                    .unwrap(),
            );
        } else if let Some(s) = s.strip_prefix('r') {
            // The relative-coordinate audit uses the captured WorldXY CPlane.
            let delta = s
                .split(',')
                .map(|s| s.parse::<f64>().unwrap())
                .collect::<Vec<_>>();
            let previous = *points.last().unwrap();
            points.push(std::array::from_fn(|i| {
                previous[i] + delta.get(i).copied().unwrap_or(0.)
            }));
        }
    }
    let closed = v["input_trace"][0]["sent"]
        .as_array()
        .unwrap()
        .iter()
        .any(|r| r["token"] == "_Close");
    format!(
        "InterpCrvOnSrf {} InfinitePlane={definition} Close={}",
        points
            .iter()
            .map(|p| p
                .iter()
                .map(|v| v.to_string())
                .collect::<Vec<_>>()
                .join(","))
            .collect::<Vec<_>>()
            .join(" "),
        if closed { "Yes" } else { "No" }
    )
}
pub fn definition(doc: &Document, v: &Value) -> String {
    let picker = v["picker"].as_array().unwrap();
    if picker.iter().any(|p| p == "Object" || p == "AroundCurve") {
        let object = doc
            .objects()
            .find(|o| o.attributes().name() == Some("plane-reference"))
            .unwrap();
        if picker.iter().any(|p| p == "Object") {
            return format!("Object:{}", object.id());
        }
        let origin = std::array::from_fn::<_, 3, _>(|i| v["plane"][i].as_f64().unwrap());
        let parameter = object
            .geometry()
            .curve_ref()
            .unwrap()
            .closest_parameter(Point3::try_from(origin).unwrap(), doc.tolerance())
            .unwrap();
        return format!("AroundCurve:{}:{parameter}", object.id());
    }
    v["plane"]
        .as_array()
        .unwrap()
        .iter()
        .map(|n| n.as_f64().unwrap().to_string())
        .collect::<Vec<_>>()
        .join(",")
}
pub fn check(doc: &Document, source: Option<ObjectId>, value: &Value, phase: &str) -> f64 {
    let expected = value[phase]["objects"].as_array().unwrap();
    assert_eq!(doc.objects().len(), expected.len(), "{phase}");
    assert_eq!(
        doc.group_by_name("Interpolation plane sources").is_some(),
        value[phase]["group_alive"].as_bool().unwrap()
    );
    let mut maximum = 0_f64;
    for (object, native) in doc.objects().zip(expected) {
        assert_eq!(
            if Some(object.id()) == source {
                "source"
            } else if object.attributes().name() == Some("plane-reference") {
                "reference"
            } else {
                "output"
            },
            native["role"]
        );
        assert_eq!(
            if doc.is_selected(object.id()) { 2 } else { 0 },
            native["selected"]
        );
        let a = object.attributes();
        let color = a.object_color();
        assert_eq!(
            json!({"name":a.name().unwrap_or(""),"layer":doc.layer(a.layer_id()).unwrap().name(),
            "color_source":if a.color_source()==ObjectColorSource::Object{"ColorFromObject"}else{"ColorFromLayer"},
            "color":[color.red,color.green,color.blue],"groups":object.group_ids().iter().map(|g|doc.group(*g).unwrap().name().unwrap_or("")).collect::<Vec<_>>()}),
            native["attributes"]
        );
        assert_eq!(native["native_valid"], true);
        let actual = object.geometry().curve_ref().unwrap().to_nurbs().unwrap();
        let target = curve(&native["curve"]);
        assert_eq!(actual.is_closed().unwrap(), native["closed"]);
        if native["role"] != "output" {
            assert_eq!(actual.degree(), target.degree());
            assert_eq!(actual.knots(), target.knots());
            assert_eq!(actual.control_points(), target.control_points());
            assert_eq!(actual.domain(), target.domain());
            continue;
        }
        for (a, b) in [(&actual, &target), (&target, &actual)] {
            let query = b.prepare_closest_parameter_search().unwrap();
            for (lo, hi) in a.spans() {
                for i in 0..=8 {
                    let p = a.evaluate(lo + (hi - lo) * i as f64 / 8.).unwrap();
                    let t = query
                        .closest_parameter(p, Tolerance::try_new(1e-10, 1e-12, 1e-9).unwrap())
                        .unwrap();
                    maximum = maximum.max(p.distance_to(b.evaluate(t).unwrap()).unwrap());
                }
            }
        }
    }
    maximum
}
