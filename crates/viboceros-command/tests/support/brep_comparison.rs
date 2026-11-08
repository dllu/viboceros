//! Finite public native B-rep expectations shared across command suites.
use serde_json::Value;
use viboceros_geometry::{Brep, NurbsCurve, NurbsSurface, Point3, Tolerance, WeightedPoint3};
#[path = "brep_captured.rs"]
pub mod captured;
#[path = "brep_geometry.rs"]
pub mod geometry;
fn point(v: &Value) -> Point3 {
    Point3::try_from(serde_json::from_value::<[f64; 3]>(v.clone()).unwrap()).unwrap()
}
fn controls(v: &Value) -> Vec<WeightedPoint3> {
    v["control_points"]
        .as_array()
        .unwrap()
        .iter()
        .map(|c| {
            WeightedPoint3::try_new(point(&c["point"]), c["weight"].as_f64().unwrap()).unwrap()
        })
        .collect()
}
fn curve(v: &Value) -> NurbsCurve {
    NurbsCurve::try_new_rational(
        v["degree"].as_u64().unwrap() as usize,
        controls(v),
        serde_json::from_value(v["knots"].clone()).unwrap(),
    )
    .unwrap()
}
fn surface(v: &Value) -> NurbsSurface {
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
