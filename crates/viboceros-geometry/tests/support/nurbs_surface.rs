//! Decode complete oracle surface definitions without sampling or rounding.
use serde_json::Value;
use viboceros_geometry::{NurbsSurface, Point3, WeightedPoint3};

pub fn from_definition(v: &Value) -> NurbsSurface {
    let n = |key: &str, i: usize| v[key][i].as_u64().unwrap() as usize;
    NurbsSurface::try_new_rational(
        n("degree", 0),
        n("degree", 1),
        n("control_count", 0),
        n("control_count", 1),
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
        serde_json::from_value(v["knots_u"].clone()).unwrap(),
        serde_json::from_value(v["knots_v"].clone()).unwrap(),
    )
    .unwrap()
}
