//! Prescribed inputs and independent Native geometry/document expectations.
use super::{native, reference_inputs};
use serde_json::{Value, json};
use viboceros_command::fillet::Preferences;
use viboceros_document::{Document, Geometry, ObjectColorSource, ObjectId};
use viboceros_geometry::{CurveArcExtensionStyle, CurveOtherExtensionStyle, Point3};

pub const FAMILIES: [&str; 4] = [
    "fillet_getter_pilot",
    "fillet_getter_discovery",
    "fillet_getter_holdouts",
    "fillet_getter_final",
];

pub fn diagnostic(value: &Value) -> bool {
    value["input_trace"][0]["errors"] != json!([])
}
pub fn command_name(value: &Value) -> &str {
    value["events"]
        .as_array()
        .unwrap()
        .iter()
        .find_map(|e| {
            e["name"]
                .as_str()
                .filter(|n| ["Fillet", "Chamfer"].contains(n))
        })
        .unwrap()
}
pub fn success(value: &Value) -> bool {
    value["events"]
        .as_array()
        .unwrap()
        .iter()
        .find(|e| e["name"] == command_name(value))
        .unwrap()["result"]
        == "Success"
}
pub fn public_options(prompt: &str, mut defaults: Preferences) -> Preferences {
    let value = |key: &str| {
        prompt
            .split(&format!("{key}="))
            .nth(1)
            .map(|s| s.split_whitespace().next().unwrap().trim_end_matches(')'))
    };
    if let Some(v) = value("Radius") {
        defaults.radius = v.parse().unwrap();
    }
    if let Some(v) = value("Join") {
        defaults.join = v == "Yes";
    }
    if let Some(v) = value("Trim") {
        defaults.trim = v == "Yes";
    }
    if let Some(v) = value("Dynamic") {
        defaults.dynamic = v == "Yes";
    }
    if let Some(v) = value("ExtendArcsBy") {
        defaults.styles.arc = match v {
            "Arc" => CurveArcExtensionStyle::Arc,
            "Line" => CurveArcExtensionStyle::Line,
            _ => panic!("Native arc extension style"),
        };
    }
    if let Some(v) = value("ExtendOtherCurvesBy") {
        defaults.styles.other = match v {
            "Line" => CurveOtherExtensionStyle::Line,
            "Smooth" => CurveOtherExtensionStyle::Smooth,
            _ => panic!("Native other extension style"),
        };
    }
    defaults
}
pub fn initial(value: &Value, defaults: Preferences) -> Preferences {
    public_options(
        value["input_trace"][0]["prompts"][0].as_str().unwrap(),
        defaults,
    )
}
pub fn final_options(value: &Value, defaults: Preferences) -> Preferences {
    let mut options = initial(value, defaults);
    let mut radius_prompt = false;
    let mut start: Option<Point3> = None;
    for action in value["prescribed_case"]["actions"].as_array().unwrap() {
        if action.as_str() == Some("Radius") {
            radius_prompt = true;
        } else if let Some(v) = action.as_f64() {
            options.radius = v;
            radius_prompt = false;
        } else if action.is_array() {
            let point = native::point(action);
            if let Some(p) = start.take() {
                options.radius = p.distance_to(point).unwrap();
            } else {
                start = Some(point);
            }
        } else if let Some(text) = action.as_str().filter(|s| s.contains('=')) {
            if text.starts_with("Join=") && !options.trim {
                continue;
            }
            options = options
                .updated(&[text], &viboceros_geometry::LengthUnitSystem::Millimeters)
                .unwrap();
        }
    }
    assert!(!radius_prompt);
    options
}
pub fn picks(doc: &Document, value: &Value) -> [(ObjectId, Point3); 2] {
    if let Some(points) = value["prescribed_case"]["source_seed_points"].as_array() {
        return std::array::from_fn(|i| {
            (reference_inputs::guide(doc, i), native::point(&points[i]))
        });
    }
    value["input_trace"][0]["clicks"]
        .as_array()
        .unwrap()
        .iter()
        .take(2)
        .map(|c| {
            (
                reference_inputs::guide(doc, c["curve"].as_u64().unwrap() as usize),
                input_point(c),
            )
        })
        .collect::<Vec<_>>()
        .try_into()
        .unwrap()
}
pub fn input_point(click: &Value) -> Point3 {
    // A source need not lie in the viewport's construction plane. This public
    // pre-command witness measures the curve against the actual mouse ray.
    native::point(
        click
            .get("sight_point")
            .unwrap_or(&click["projected_point"]),
    )
}
pub fn setup(value: &Value) -> Document {
    reference_inputs::setup(value, &value["prescribed_case"])
}

pub fn check(doc: &Document, target: &Value, archive: bool) -> f64 {
    let mut rows: Vec<_> = target.as_array().unwrap().iter().collect();
    assert_eq!(doc.objects().len(), rows.len());
    let mut error = 0_f64;
    for object in doc.objects() {
        let a = object.attributes();
        // A joined fillet can copy the first closed source's name onto a new
        // object while retaining that source. Pair by name and start location.
        let start = match object.geometry() {
            Geometry::Point(p) => *p,
            g => {
                let c = g.curve_ref().unwrap();
                c.evaluate(*c.domain().start()).unwrap()
            }
        };
        let index = rows
            .iter()
            .enumerate()
            .filter(|(_, row)| row["attributes"]["name"] == a.name().unwrap_or(""))
            .min_by(|(_, a), (_, b)| {
                let point = |r: &&Value| {
                    if r.get("point").is_some() {
                        native::point(&r["point"])
                    } else {
                        native::point(&r["samples"][0]["point"])
                    }
                };
                start
                    .distance_to(point(a))
                    .unwrap()
                    .total_cmp(&start.distance_to(point(b)).unwrap())
            })
            .expect("Native object with matching name")
            .0;
        let target = rows.remove(index);
        assert_eq!(
            json!({"name":a.name().unwrap_or(""), "layer":doc.layer(a.layer_id()).unwrap().name(),
            "color_source":if a.color_source()==ObjectColorSource::Object {"ColorFromObject"} else {"ColorFromLayer"},
            "color":[a.object_color().red,a.object_color().green,a.object_color().blue],
            "groups":object.group_ids().iter().map(|id|doc.group(*id).unwrap().name().unwrap()).collect::<Vec<_>>()}),
            target["attributes"]
        );
        assert_eq!(
            doc.is_selected(object.id()),
            !archive && target["selected"] != 0
        );
        assert_eq!(target["native_valid"], true);
        if let Geometry::Point(p) = object.geometry() {
            error = error.max(p.distance_to(native::point(&target["point"])).unwrap());
            continue;
        }
        let actual = object.geometry().curve_ref().unwrap().to_nurbs().unwrap();
        let expected = native::curve(&target["curve"]);
        assert_eq!(actual.degree(), expected.degree());
        assert_eq!(
            actual.control_points().len(),
            expected.control_points().len(),
            "{} actual {actual:?} expected {expected:?}",
            a.name().unwrap_or("")
        );
        for (a, b) in actual
            .control_points()
            .iter()
            .zip(expected.control_points())
        {
            error = error
                .max(a.point().distance_to(b.point()).unwrap())
                .max((a.weight() - b.weight()).abs());
        }
        assert_eq!(actual.knots().len(), expected.knots().len());
        for (a, b) in actual.knots().iter().zip(expected.knots()) {
            error = error.max((a - b).abs());
        }
        for row in target["samples"].as_array().unwrap() {
            // Knots already verify the original native chart. Evaluate its
            // parameter directly: an affine remap can move a knot station by
            // one ULP and select the other tangent at a nonsmooth junction.
            let source = object.geometry().curve_ref().unwrap();
            let t = row["parameter"]
                .as_f64()
                .unwrap()
                .clamp(*source.domain().start(), *source.domain().end());
            let sample = source.evaluate_with_tangent(t).unwrap();
            error = error.max(
                sample
                    .point()
                    .distance_to(native::point(&row["point"]))
                    .unwrap(),
            );
            error = error.max(
                Point3::try_from(sample.tangent().as_vector().to_array())
                    .unwrap()
                    .distance_to(native::point(&row["tangent"]))
                    .unwrap(),
            );
        }
        if error > 1e-6 {
            eprintln!(
                "Curve pair {} actual {actual:?} expected {expected:?}",
                a.name().unwrap_or("")
            );
        }
    }
    assert!(
        error <= 1e-6,
        "Native curve pair coefficients, domains, points and tangents: {error:e}"
    );
    error
}

#[path = "fillet_getter/history.rs"]
mod history;
#[path = "fillet_getter/state.rs"]
mod state;
pub use history::history_recorded;
pub use state::State;

pub fn read(family: &str) -> Value {
    history::read(family)
}

pub fn archive(doc: &mut Document, value: &Value) -> f64 {
    state::archive(doc, value)
}
