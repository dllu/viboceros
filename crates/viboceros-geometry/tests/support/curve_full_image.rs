//! Positive-rational curve image bounds after exact degree/knot refinement.
use serde_json::Value;
use viboceros_geometry::{NurbsCurve, WeightedPoint3};

pub fn curve(value: &Value) -> NurbsCurve {
    NurbsCurve::try_new_rational(
        value["degree"].as_u64().unwrap() as usize,
        value["control_points"]
            .as_array()
            .unwrap()
            .iter()
            .map(|c| {
                WeightedPoint3::try_new(
                    crate::native::point(&c["point"]),
                    c["weight"].as_f64().unwrap(),
                )
                .unwrap()
            })
            .collect(),
        serde_json::from_value(value["knots"].clone()).unwrap(),
    )
    .unwrap()
}
pub fn curve_bound(a: &NurbsCurve, b: &NurbsCurve) -> f64 {
    let mut a = a.try_reparameterized(0. ..=1.).unwrap();
    let mut b = b.try_reparameterized(0. ..=1.).unwrap();
    let degree = a.degree().max(b.degree());
    if a.degree() != degree {
        a = a.try_change_degree(degree, false).unwrap();
    }
    if b.degree() != degree {
        b = b.try_change_degree(degree, false).unwrap();
    }
    let mut knots = Vec::new();
    for c in [&a, &b] {
        for g in c.knots().chunk_by(|a, b| a == b) {
            if g[0] > 0. && g[0] < 1. {
                knots.push((g[0], g.len()));
            }
        }
    }
    knots.sort_by(|a, b| a.0.total_cmp(&b.0));
    for g in knots.chunk_by(|a, b| a.0 == b.0) {
        let m = g.iter().map(|v| v.1).max().unwrap();
        a = a.try_insert_knot(g[0].0, m).unwrap();
        b = b.try_insert_knot(g[0].0, m).unwrap();
    }
    assert_eq!(a.knots(), b.knots());
    assert_eq!(a.control_points().len(), b.control_points().len());
    let anchor = b.control_points()[0].point();
    let (mut delta, mut weight_delta, mut radius, mut minimum) =
        (0_f64, 0_f64, 0_f64, f64::INFINITY);
    for (a, b) in a.control_points().iter().zip(b.control_points()) {
        assert!(a.weight() > 0. && b.weight() > 0.);
        delta = delta.max(a.point().distance_to(b.point()).unwrap());
        weight_delta = weight_delta.max((a.weight() - b.weight()).abs());
        radius = radius.max(anchor.distance_to(b.point()).unwrap());
        minimum = minimum.min(a.weight());
    }
    delta + 2. * weight_delta * radius / minimum + 128. * f64::EPSILON * radius.max(1.)
}
