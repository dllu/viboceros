use serde_json::{Value, json};
use viboceros_geometry::{NurbsCurve, NurbsSurface, Point3, WeightedPoint3};

pub(super) fn curve_value(curve: &NurbsCurve) -> Value {
    json!({"degree":curve.degree(),"domain":[*curve.domain().start(),*curve.domain().end()],"knots":curve.knots(),"control_points":control_values(curve.control_points()),"closed":curve.is_closed().unwrap(),"periodic":curve.is_periodic()})
}

pub(super) fn surface_value(surface: &NurbsSurface) -> Value {
    json!({"degree":[surface.degree_u(),surface.degree_v()],"domain_u":[*surface.domain_u().start(),*surface.domain_u().end()],"domain_v":[*surface.domain_v().start(),*surface.domain_v().end()],"knots_u":surface.knots_u(),"knots_v":surface.knots_v(),"control_count":[surface.control_point_count_u(),surface.control_point_count_v()],"control_points":control_values(surface.control_points()),"periodic_u":surface.is_periodic_u(),"periodic_v":surface.is_periodic_v()})
}
pub(super) fn points(value: &Value) -> Vec<WeightedPoint3> {
    value
        .as_array()
        .unwrap()
        .iter()
        .map(|c| {
            let p = &c["point"];
            WeightedPoint3::try_new(
                Point3::try_new(
                    p[0].as_f64().unwrap(),
                    p[1].as_f64().unwrap(),
                    p[2].as_f64().unwrap(),
                )
                .unwrap(),
                c.get("weight").map_or(1., |w| w.as_f64().unwrap()),
            )
            .unwrap()
        })
        .collect()
}
pub(super) fn knots(value: &Value) -> Vec<f64> {
    value
        .as_array()
        .unwrap()
        .iter()
        .map(|x| x.as_f64().unwrap())
        .collect()
}
pub(super) fn control_values(points: &[WeightedPoint3]) -> Value {
    json!(
        points
            .iter()
            .map(|p| json!({"point":p.point().to_array(),"weight":p.weight()}))
            .collect::<Vec<_>>()
    )
}
pub(super) fn compare(actual: &Value, expected: &Value, path: &str) {
    match (actual, expected) {
        (Value::Number(a), Value::Number(b)) => {
            let (a, b) = (a.as_f64().unwrap(), b.as_f64().unwrap());
            assert!(
                (a - b).abs() <= 1e-10 + 1e-12 * a.abs().max(b.abs()),
                "{path}: {a} != {b}"
            );
        }
        (Value::Array(a), Value::Array(b)) => {
            assert_eq!(a.len(), b.len(), "{path}");
            for (i, (a, b)) in a.iter().zip(b).enumerate() {
                compare(a, b, &format!("{path}/{i}"));
            }
        }
        (Value::Object(a), Value::Object(b)) => {
            assert_eq!(
                a.keys().collect::<Vec<_>>(),
                b.keys().collect::<Vec<_>>(),
                "{path}"
            );
            for (k, v) in a {
                compare(v, &b[k], &format!("{path}/{k}"));
            }
        }
        _ => assert_eq!(actual, expected, "{path}"),
    }
}
