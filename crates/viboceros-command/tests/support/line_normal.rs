//! Independently prescribed curve/surface sources and Normal action replay.
use super::native;
use super::reference_inputs as curves;
pub use curves::{guide, token};
use serde_json::Value;
use viboceros_command::CommandRegistry;
use viboceros_document::{Document, Geometry};
use viboceros_geometry::{Brep, NurbsSurface, Tolerance, WeightedPoint3};
pub const FAMILIES: [&str; 4] = [
    "line_normal",
    "line_normal_holdouts",
    "line_normal_final",
    "line_normal_brep",
];
/// Preserve incomplete/misdirected native captures as diagnostics, not passes.
pub fn diagnostic(id: &str) -> bool {
    [
        "circle_length",
        "plane_length",
        "line_length",
        "surface_reversed",
        "trimmed_no",
        "trimmed_yes",
        "trimmed_flipped",
    ]
    .contains(&id)
}
pub fn sources() -> Value {
    serde_json::from_str(include_str!(
        "../../../../tools/rhino_oracle/fixtures/line_normal_sources.json"
    ))
    .unwrap()
}
pub fn geometry(v: &Value) -> Geometry {
    let tolerance = Tolerance::NUMERICAL_VALIDATION;
    let surface = |v: &Value| {
        NurbsSurface::try_new_rational(
            v["degree_u"].as_u64().unwrap() as usize,
            v["degree_v"].as_u64().unwrap() as usize,
            v["control_point_count_u"].as_u64().unwrap() as usize,
            v["control_point_count_v"].as_u64().unwrap() as usize,
            v["control_points"]
                .as_array()
                .unwrap()
                .iter()
                .map(|p| {
                    WeightedPoint3::try_new(
                        native::point(&p["point"]),
                        p["weight"].as_f64().unwrap(),
                    )
                    .unwrap()
                })
                .collect(),
            serde_json::from_value(v["knots_u"].clone()).unwrap(),
            serde_json::from_value(v["knots_v"].clone()).unwrap(),
        )
        .unwrap()
    };
    match v["type"].as_str().unwrap() {
        "box_brep" => {
            let min = native::point(&v["min"]).to_array();
            let max = native::point(&v["max"]).to_array();
            Geometry::Brep(
                Brep::try_command_box(
                    viboceros_command::CommandContext::default().construction_plane,
                    std::array::from_fn(|i| [min[i], max[i]]),
                    tolerance,
                )
                .unwrap(),
            )
        }
        "surface" => Geometry::Brep(Brep::try_surface_face(surface(v), tolerance).unwrap()),
        "normal_surface" => {
            let target = &v["target"];
            // The prescribed four corners define a planar 10-by-10 patch.
            // Rhino CreateFromCorners uses physical side-length domains.
            let p = &target["points"];
            let s = NurbsSurface::try_new(
                1,
                1,
                2,
                2,
                [0, 1, 3, 2].map(|i| native::point(&p[i])).to_vec(),
                vec![0., 0., 10., 10.],
                vec![0., 0., 10., 10.],
            )
            .unwrap();
            let b = if target["kind"] == "trimmed" {
                Brep::try_split_rectangular_surface_face_u(
                    s,
                    0. ..=10.,
                    0. ..=10.,
                    5.,
                    false,
                    tolerance,
                )
                .unwrap()[0]
                    .clone()
            } else {
                Brep::try_surface_face(s, tolerance).unwrap()
            };
            Geometry::Brep(if target["flip"] == true {
                b.reversed()
            } else {
                b
            })
        }
        _ => curves::geometry(v),
    }
}
pub fn setup(v: &Value, source: &Value) -> Document {
    let mut doc = native::scene_with_geometry(v, |row| {
        if row["source"] == true {
            Geometry::Point(native::point(&row["point"]))
        } else {
            let index = row["attributes"]["name"]
                .as_str()
                .unwrap()
                .strip_prefix("guide")
                .unwrap()
                .parse::<usize>()
                .unwrap();
            geometry(&source["curves"][index])
        }
    });
    let t = &v["model_tolerance"];
    doc.set_tolerance(
        Tolerance::try_new(
            t["absolute"].as_f64().unwrap(),
            t["relative"].as_f64().unwrap(),
            t["angular"].as_f64().unwrap(),
        )
        .unwrap(),
    );
    doc.clear_undo_records().unwrap();
    if source["pending_redo"] == true {
        CommandRegistry::with_builtins()
            .execute(&mut doc, "Point 90,91,92")
            .unwrap();
        doc.undo().unwrap();
    }
    doc
}
pub fn command(doc: &Document, source: &Value) -> String {
    let mut tokens = vec!["Line".to_owned()];
    for value in source["actions"].as_array().unwrap() {
        if value.is_object() {
            if value.get("hover").is_some() {
                continue;
            }
            let id = guide(doc, value["curve"].as_u64().unwrap() as usize);
            let key = if doc.object(id).unwrap().geometry().curve_ref().is_some() {
                "Curve"
            } else {
                "Surface"
            };
            tokens.push(format!("{key}={id}"));
            tokens.push(token(&value["pick"]));
        } else {
            tokens.push(token(value));
        }
    }
    tokens.join(" ")
}
