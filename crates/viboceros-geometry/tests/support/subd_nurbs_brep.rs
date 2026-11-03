//! Complete edge and oriented topology comparison against native records.
use serde_json::Value;
use std::collections::BTreeMap;
use viboceros_geometry::{Brep, NurbsCurve, WeightedPoint3};

fn curve(value: &Value) -> NurbsCurve {
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
fn curve_bound(a: &NurbsCurve, b: &NurbsCurve) -> f64 {
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
fn cycle(values: &[usize]) -> Vec<usize> {
    (0..values.len())
        .map(|i| {
            values[i..]
                .iter()
                .chain(&values[..i])
                .copied()
                .collect::<Vec<_>>()
        })
        .min()
        .unwrap()
}

pub fn check(actual: &Brep, value: &Value) {
    let target = &value["output"]["geometry"];
    let topology = &target["topology"];
    assert_eq!(
        actual.faces().len(),
        target["faces"].as_array().unwrap().len()
    );
    assert_eq!(
        actual.edges().len(),
        target["edges"].as_array().unwrap().len()
    );
    assert_eq!(
        actual.vertices().len(),
        target["vertices"].as_array().unwrap().len()
    );
    assert_eq!(
        actual.is_solid(),
        value["output"]["solid"].as_bool().unwrap()
    );
    assert!(actual.is_manifold());
    let mut vertex_map = Vec::new();
    let mut used = vec![false; actual.vertices().len()];
    for v in actual.vertices() {
        let (i, d) = target["vertices"]
            .as_array()
            .unwrap()
            .iter()
            .enumerate()
            .map(|(i, p)| {
                (
                    i,
                    v.point()
                        .distance_to(crate::native::point(&p["point"]))
                        .unwrap(),
                )
            })
            .min_by(|a, b| a.1.total_cmp(&b.1))
            .unwrap();
        assert!(d < 1e-11 && !used[i]);
        used[i] = true;
        vertex_map.push(i);
    }
    let mut expected_edges = BTreeMap::new();
    for (i, edge) in topology["edges"].as_array().unwrap().iter().enumerate() {
        let (a, b) = (
            edge[0].as_u64().unwrap() as usize,
            edge[1].as_u64().unwrap() as usize,
        );
        assert!(expected_edges.insert([a.min(b), a.max(b)], i).is_none());
    }
    let mut maximum = 0_f64;
    for edge in actual.edges() {
        let [a, b] = edge.vertices().map(|i| vertex_map[i]);
        let i = expected_edges[&[a.min(b), a.max(b)]];
        let mut expected = curve(&target["edges"][i]["curve"]["definition"]);
        if topology["edges"][i][0].as_u64().unwrap() as usize != a {
            expected = expected.reversed().unwrap();
        }
        maximum = maximum.max(curve_bound(edge.curve(), &expected));
    }
    assert!(maximum < 1e-11, "edge full-image bound {maximum}");
    for (face_index, face) in actual.faces().iter().enumerate() {
        assert_eq!(face.loops().len(), 1);
        let boundary = &face.loops()[0];
        let expected = &topology["faces"][face_index]["loops"][0]["trims"];
        let expected = expected.as_array().unwrap();
        assert_eq!(boundary.trims().len(), expected.len());
        let a = boundary
            .trims()
            .iter()
            .map(|t| vertex_map[t.vertices()[0]])
            .collect::<Vec<_>>();
        let b = expected
            .iter()
            .map(|t| t["vertices"][0].as_u64().unwrap() as usize)
            .collect::<Vec<_>>();
        assert_eq!(cycle(&a), cycle(&b));
        for trim in boundary.trims() {
            let first = vertex_map[trim.vertices()[0]];
            let native = expected
                .iter()
                .find(|t| t["vertices"][0].as_u64().unwrap() as usize == first)
                .unwrap();
            assert_eq!(
                format!("{:?}", trim.trim_type()),
                native["type"].as_str().unwrap()
            );
        }
    }
}
