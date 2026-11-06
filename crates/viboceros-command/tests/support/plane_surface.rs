//! Native Plane coefficients, document policy and prescribed macro replay.
use serde_json::{Value, json};
use viboceros_document::{ColorRgb, Document, Geometry, ObjectAttributes, ObjectColorSource};
use viboceros_geometry::{
    Frame3, NurbsCurve, NurbsSurface, Point3, Tolerance, Vector3, WeightedPoint3,
};

pub fn read(name: &str) -> Value {
    use std::io::Read;
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    let base = if root.join("tools").is_dir() {
        root.join("tools/rhino_oracle/observations")
    } else {
        root.join("../../tools/rhino_oracle/observations")
    };
    let path = base.join(format!("{name}.json"));
    let bytes = if path.exists() {
        std::fs::read(path).unwrap()
    } else {
        let mut bytes = Vec::new();
        flate2::read::GzDecoder::new(
            std::fs::File::open(base.join(format!("{name}.json.gz"))).unwrap(),
        )
        .read_to_end(&mut bytes)
        .unwrap();
        bytes
    };
    serde_json::from_slice(&bytes).unwrap()
}
pub fn point(v: &Value) -> Point3 {
    Point3::try_from(serde_json::from_value::<[f64; 3]>(v.clone()).unwrap()).unwrap()
}
fn controls(v: &Value) -> Vec<WeightedPoint3> {
    v["control_points"]
        .as_array()
        .unwrap()
        .iter()
        .map(|p| {
            WeightedPoint3::try_new(point(&p["point"]), p["weight"].as_f64().unwrap()).unwrap()
        })
        .collect()
}
pub fn curve(v: &Value) -> NurbsCurve {
    NurbsCurve::try_new_rational(
        v["degree"].as_u64().unwrap() as usize,
        controls(v),
        serde_json::from_value(v["knots"].clone()).unwrap(),
    )
    .unwrap()
}
pub fn surface(v: &Value) -> NurbsSurface {
    NurbsSurface::try_new_rational(
        v["degree"][0].as_u64().unwrap() as usize,
        v["degree"][1].as_u64().unwrap() as usize,
        v["control_count"][0].as_u64().unwrap() as usize,
        v["control_count"][1].as_u64().unwrap() as usize,
        controls(v),
        serde_json::from_value(v["knots_u"].clone()).unwrap(),
        serde_json::from_value(v["knots_v"].clone()).unwrap(),
    )
    .unwrap()
}
pub fn frame(v: &Value) -> Frame3 {
    let p = &v["construction_plane"];
    Frame3::try_from_directions(
        point(&p[0]),
        Vector3::try_from(point(&p[1]).to_array()).unwrap(),
        Vector3::try_from(point(&p[2]).to_array()).unwrap(),
        Tolerance::NUMERICAL_VALIDATION,
    )
    .unwrap()
}
pub fn setup(v: &Value) -> Document {
    let mut doc = scene(v);
    doc.clear_history().unwrap();
    doc
}
/// Assemble the prescribed existing scene before choosing its history policy.
pub fn scene(v: &Value) -> Document {
    scene_with_geometry(v, |row| {
        if row["source"] == true {
            Geometry::Point(point(&row["point"]))
        } else {
            Geometry::NurbsCurve(curve(&row["curve"]))
        }
    })
}
/// Keep scene metadata independent from each family's prescribed geometry.
pub fn scene_with_geometry(v: &Value, geometry: impl Fn(&Value) -> Geometry) -> Document {
    let mut doc = Document::new(Tolerance::try_new(1e-6, 1e-9, 1e-6).unwrap());
    let input = doc.add_layer("Plane sources", ColorRgb::BLACK).unwrap();
    let output = doc.add_layer("Plane output", ColorRgb::BLACK).unwrap();
    let mut ids = Vec::new();
    let mut selected = Vec::new();
    for row in v["before"].as_array().unwrap() {
        let mut attrs = ObjectAttributes::on_layer(input)
            .with_name(row["attributes"]["name"].as_str().unwrap());
        if row["attributes"]["color_source"] == "ColorFromObject" {
            let c = &row["attributes"]["color"];
            attrs = attrs.with_object_color(ColorRgb {
                red: c[0].as_u64().unwrap() as u8,
                green: c[1].as_u64().unwrap() as u8,
                blue: c[2].as_u64().unwrap() as u8,
            });
        }
        let geometry = geometry(row);
        let id = doc.add_geometry_with_attributes(geometry, attrs).unwrap();
        ids.push(id);
        if row["selected"] != 0 {
            selected.push(id);
        }
    }
    doc.add_group(Some("Plane source group".into()), ids)
        .unwrap();
    doc.set_transient_selection(selected).unwrap();
    doc.set_current_layer(output).unwrap();
    doc
}
/// Tokens are prescribed inputs recorded by the frozen producer, not derived
/// from native output geometry. Replace only the guide's runtime identity.
pub fn tokens(doc: &Document, v: &Value) -> Vec<String> {
    let words = v["macro"]
        .as_str()
        .unwrap()
        .split_whitespace()
        .collect::<Vec<_>>();
    assert_eq!(words[0], "_Plane");
    let mut result = Vec::new();
    let mut i = 1;
    while i < words.len() {
        if words[i] == "'_-SelID" {
            result.push(format!(
                "Curve={}",
                doc.objects()
                    .find(|o| o.attributes().name() == Some("guide"))
                    .unwrap()
                    .id()
            ));
            i += 2;
        } else {
            result.push(words[i].trim_start_matches('_').to_owned());
            i += 1;
        }
    }
    result
}
pub fn check(doc: &Document, v: &Value, phase: &str) -> f64 {
    let expected = v[phase].as_array().unwrap();
    assert_eq!(doc.objects().len(), expected.len(), "{phase}");
    let mut error = 0_f64;
    for (object, target) in doc.objects().zip(expected) {
        let a = object.attributes();
        assert_eq!(
            if doc.is_selected(object.id()) { 2 } else { 0 },
            target["selected"],
            "{phase}"
        );
        assert_eq!(
            json!({"name":a.name().unwrap_or(""),"layer":doc.layer(a.layer_id()).unwrap().name(),
            "color_source":if a.color_source()==ObjectColorSource::Object {"ColorFromObject"} else {"ColorFromLayer"},
            "color":[a.object_color().red,a.object_color().green,a.object_color().blue],
            "groups":object.group_ids().iter().map(|g|doc.group(*g).unwrap().name().unwrap()).collect::<Vec<_>>()}),
            target["attributes"],
            "{phase}"
        );
        assert_eq!(target["native_valid"], true);
        assert_eq!(a.name() == Some("sentinel"), target["source"]);
        assert_eq!(a.name() == Some("guide"), target["guide"]);
        if target["source"] == true {
            assert_eq!(object.geometry(), &Geometry::Point(point(&target["point"])));
        } else if target["guide"] == true {
            assert_eq!(
                object.geometry().curve_ref().unwrap().to_nurbs().unwrap(),
                curve(&target["curve"])
            );
        } else {
            assert_eq!(target["face_count"], 1);
            let Geometry::NurbsSurface(actual) = object.geometry() else {
                panic!("expected plane surface")
            };
            let target = surface(&target["surfaces"][0]);
            assert_eq!(
                [actual.degree_u(), actual.degree_v()],
                [target.degree_u(), target.degree_v()]
            );
            assert_eq!(
                [
                    actual.control_point_count_u(),
                    actual.control_point_count_v()
                ],
                [
                    target.control_point_count_u(),
                    target.control_point_count_v()
                ]
            );
            for (a, b) in actual.control_points().iter().zip(target.control_points()) {
                assert_eq!(a.weight(), b.weight());
                error = error.max(a.point().distance_to(b.point()).unwrap());
            }
            for (a, b) in [
                (actual.knots_u(), target.knots_u()),
                (actual.knots_v(), target.knots_v()),
            ] {
                assert_eq!(a.len(), b.len());
                for (a, b) in a.iter().zip(b) {
                    error = error.max((a - b).abs());
                }
            }
            for i in 0..=8 {
                for j in 0..=8 {
                    let u = i as f64 / 8.;
                    let w = j as f64 / 8.;
                    let at = |s: &NurbsSurface| {
                        s.evaluate(
                            *s.domain_u().start() + (s.domain_u().end() - s.domain_u().start()) * u,
                            *s.domain_v().start() + (s.domain_v().end() - s.domain_v().start()) * w,
                        )
                        .unwrap()
                    };
                    error = error.max(at(actual).distance_to(at(&target)).unwrap());
                }
            }
            if error > 1e-6 {
                eprintln!(
                    "Plane discrepancy: domains actual {:?} {:?}, native {:?} {:?}; maximum control error {}",
                    actual.domain_u(),
                    actual.domain_v(),
                    target.domain_u(),
                    target.domain_v(),
                    actual
                        .control_points()
                        .iter()
                        .zip(target.control_points())
                        .map(|(a, b)| a.point().distance_to(b.point()).unwrap())
                        .fold(0_f64, f64::max)
                );
            }
        }
    }
    error
}

/// Archive selection is transient. Geometry, attributes and groups still use
/// the same complete native checks as the live document.
#[allow(dead_code)]
pub fn check_archive(doc: &Document, v: &Value) -> f64 {
    let mut archive = v.clone();
    for object in archive["after"].as_array_mut().unwrap() {
        object["selected"] = json!(0);
    }
    check(doc, &archive, "after")
}
