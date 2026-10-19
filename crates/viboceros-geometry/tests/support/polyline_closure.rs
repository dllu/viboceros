#![allow(dead_code)]
use serde_json::Value;
use viboceros_geometry::{
    Curve3, LineSegment, NurbsCurve, Point3, PolyCurve3, Polyline3, Real, Tolerance, WeightedPoint3,
};
pub fn capture() -> Value {
    serde_json::from_str(include_str!(
        "../../../../tools/rhino_oracle/observations/polyline_closure.json"
    ))
    .unwrap()
}
pub fn point(v: &Value) -> Point3 {
    Point3::try_from(serde_json::from_value::<[Real; 3]>(v.clone()).unwrap()).unwrap()
}
pub fn curve(v: &Value) -> Curve3 {
    match v["type"].as_str().unwrap() {
        "polyline" => Curve3::Polyline(
            Polyline3::try_with_parameters(
                v["vertices"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .map(point)
                    .collect(),
                serde_json::from_value(v["parameters"].clone()).unwrap(),
                Tolerance::NUMERICAL_VALIDATION,
            )
            .unwrap(),
        ),
        "nurbs" => {
            let d = &v["definition"];
            let curve = NurbsCurve::try_new_rational(
                d["degree"].as_u64().unwrap() as usize,
                d["control_points"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .map(|p| {
                        WeightedPoint3::try_new(point(&p["point"]), p["weight"].as_f64().unwrap())
                            .unwrap()
                    })
                    .collect(),
                serde_json::from_value(d["knots"].clone()).unwrap(),
            )
            .unwrap();
            Curve3::NurbsCurve(if v["rational"].as_bool().unwrap() {
                curve.with_rational_representation()
            } else {
                curve
            })
        }

        "line" => Curve3::Line(
            LineSegment::try_new(
                point(&v["start"]),
                point(&v["end"]),
                Tolerance::NUMERICAL_VALIDATION,
            )
            .unwrap()
            .try_reparameterized(
                v["domain"][0].as_f64().unwrap()..=v["domain"][1].as_f64().unwrap(),
            )
            .unwrap(),
        ),
        "polycurve" => {
            let parts = v["segments"]
                .as_array()
                .unwrap()
                .iter()
                .enumerate()
                .map(|(i, s)| {
                    curve(s)
                        .to_polycurve()
                        .unwrap()
                        .try_reparameterized(
                            v["parameters"][i].as_f64().unwrap()
                                ..=v["parameters"][i + 1].as_f64().unwrap(),
                        )
                        .unwrap()
                })
                .collect::<Vec<_>>();
            Curve3::PolyCurve(PolyCurve3::concatenate(&parts).unwrap())
        }
        _ => panic!("unknown native closure record"),
    }
}
pub fn compare_points(actual: &[Point3], expected: &Value, name: &str, ordered: bool) -> Real {
    let mut expected = expected
        .as_array()
        .unwrap()
        .iter()
        .map(point)
        .collect::<Vec<_>>();
    assert_eq!(actual.len(), expected.len(), "{name}: point count");
    let mut maximum = 0_f64;
    for (i, &p) in actual.iter().enumerate() {
        let at = if ordered {
            i
        } else {
            expected
                .iter()
                .enumerate()
                .min_by(|(_, a), (_, b)| {
                    p.distance_to(**a)
                        .unwrap()
                        .total_cmp(&p.distance_to(**b).unwrap())
                })
                .unwrap()
                .0
        };
        maximum = maximum.max(p.distance_to(expected[at]).unwrap());
        if !ordered {
            expected.remove(at);
        }
    }
    maximum
}
