//! Prescribed surface parents and passive input receipts, independent of results.
use super::{native, reference_inputs};
use serde_json::Value;
use viboceros_document::{Document, Geometry};
use viboceros_geometry::{Brep, NurbsSurface, Tolerance, WeightedPoint3};
pub const FAMILIES: [&str; 3] = [
    "blend_curve_edge_pilot",
    "blend_curve_edge_discovery",
    "blend_curve_edge_holdouts",
];
pub fn diagnostic(name: &str) -> bool {
    matches!(
        name,
        "surface_cancel" | "surface_curved_g3" | "surface_curved_g4"
    )
}
pub fn preferences(value: &Value) -> viboceros_command::blend_curve::Preferences {
    let prompts = value["input_trace"][0]["prompts"].as_array().unwrap();
    let adjustment = prompts
        .iter()
        .filter_map(Value::as_str)
        .find(|p| p.starts_with("Select control point"))
        .unwrap_or("");
    viboceros_command::blend_curve::Preferences {
        pick_point: prompts[0]
            .as_str()
            .unwrap()
            .contains("BlendStart=PickPoint"),
        trim: adjustment.contains("Trim=Yes"),
        join: adjustment.contains("Join=Yes"),
    }
}

pub fn geometry(value: &Value) -> Geometry {
    if value["type"] != "surface" {
        return reference_inputs::geometry(value);
    }
    let surface = NurbsSurface::try_new_rational(
        value["degree_u"].as_u64().unwrap() as usize,
        value["degree_v"].as_u64().unwrap() as usize,
        value["control_point_count_u"].as_u64().unwrap() as usize,
        value["control_point_count_v"].as_u64().unwrap() as usize,
        value["control_points"]
            .as_array()
            .unwrap()
            .iter()
            .map(|p| {
                WeightedPoint3::try_new(native::point(&p["point"]), p["weight"].as_f64().unwrap())
                    .unwrap()
            })
            .collect(),
        serde_json::from_value(value["knots_u"].clone()).unwrap(),
        serde_json::from_value(value["knots_v"].clone()).unwrap(),
    )
    .unwrap();
    Geometry::Brep(
        Brep::try_surface_face_with_native_edge_parameters(
            surface,
            Tolerance::NUMERICAL_VALIDATION,
        )
        .unwrap(),
    )
}

pub fn setup(value: &Value) -> Document {
    let recipe = &value["prescribed_case"];
    let mut doc = native::scene_with_geometry(value, |row| {
        if row["source"] == true {
            Geometry::Point(native::point(&serde_json::json!([50., 60., 70.])))
        } else {
            let index: usize = row["attributes"]["name"]
                .as_str()
                .unwrap()
                .strip_prefix("guide")
                .unwrap()
                .parse()
                .unwrap();
            geometry(&recipe["curves"][index])
        }
    });
    let tolerance = &value["model_tolerance"];
    doc.set_tolerance(
        Tolerance::try_new(
            tolerance["absolute"].as_f64().unwrap(),
            tolerance["relative"].as_f64().unwrap(),
            tolerance["angular"].as_f64().unwrap(),
        )
        .unwrap(),
    );
    doc.clear_undo_records().unwrap();
    if recipe["pending_redo"] == true {
        let registry = viboceros_command::CommandRegistry::with_builtins();
        registry.execute(&mut doc, "Point 90,91,92").unwrap();
        doc.undo().unwrap();
    }
    doc
}

/// Check every supporting boundary curve as well as the shared face/vertex
/// checks. Rational weights are equivalent up to one common nonzero scale.
pub fn parent_edges(doc: &Document, target: &Value) -> f64 {
    let mut error = 0_f64;
    for row in target.as_array().unwrap() {
        let Some(expected) = row.get("brep") else {
            continue;
        };
        let name = row["attributes"]["name"].as_str().unwrap();
        let object = doc
            .objects()
            .find(|o| o.attributes().name() == Some(name))
            .unwrap();
        let Geometry::Brep(brep) = object.geometry() else {
            panic!("supporting B-rep parent")
        };
        let edges = expected["edges"].as_array().unwrap();
        assert_eq!(brep.edges().len(), edges.len());
        for (actual, expected) in brep.edges().iter().zip(edges) {
            let actual = actual.curve();
            let expected = native::curve(&expected["curve"]["definition"]);
            assert_eq!(actual.degree(), expected.degree());
            assert_eq!(
                actual.control_points().len(),
                expected.control_points().len()
            );
            let [a0, b0] = [
                actual.control_points()[0].weight(),
                expected.control_points()[0].weight(),
            ];
            for (a, b) in actual
                .control_points()
                .iter()
                .zip(expected.control_points())
            {
                error = error
                    .max(a.point().distance_to(b.point()).unwrap())
                    .max((a.weight() / a0 - b.weight() / b0).abs());
            }
            assert_eq!(actual.knots().len(), expected.knots().len());
            for (a, b) in actual.knots().iter().zip(expected.knots()) {
                error = error.max((a - b).abs());
            }
            for i in 0..=32 {
                let at = |c: &viboceros_geometry::NurbsCurve| {
                    c.evaluate(c.parameter_at(i as f64 / 32.).unwrap()).unwrap()
                };
                error = error.max(at(actual).distance_to(at(&expected)).unwrap());
            }
        }
    }
    error
}
