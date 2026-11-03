use super::*;
use std::collections::BTreeMap;
use viboceros_geometry::{NurbsCurve, Point3, SubD, Tolerance, WeightedPoint3};

fn curve(value: &Value) -> NurbsCurve {
    NurbsCurve::try_new_rational(
        value["degree"].as_u64().unwrap() as usize,
        value["control_points"]
            .as_array()
            .unwrap()
            .iter()
            .map(|c| {
                WeightedPoint3::try_new(native::point(&c["point"]), c["weight"].as_f64().unwrap())
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
    assert_eq!(a.degree(), b.degree());
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
    a.control_points()
        .iter()
        .zip(b.control_points())
        .map(|(a, b)| {
            assert_eq!(a.weight(), 1.);
            assert_eq!(b.weight(), 1.);
            a.point().distance_to(b.point()).unwrap()
        })
        .fold(0_f64, f64::max)
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

pub(super) fn check(source: &SubD, value: &Value) {
    let actual = source.to_nurbs_brep_g0(Tolerance::DEFAULT).unwrap();
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
                    v.point().distance_to(native::point(&p["point"])).unwrap(),
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

#[test]
fn coincident_shells_keep_independent_topology() {
    let source = native::source(&native::case("box")["before"]);
    let mut points = source.control_points().to_vec();
    points.extend_from_within(..);
    let mut faces = source.faces().to_vec();
    faces.extend(
        source
            .faces()
            .iter()
            .map(|f| f.iter().map(|i| i + 8).collect()),
    );
    let doubled = SubD::try_from_polygons(points, faces).unwrap();
    let brep = doubled.to_nurbs_brep_g0(Tolerance::DEFAULT).unwrap();
    assert_eq!(
        (
            brep.vertices().len(),
            brep.edges().len(),
            brep.faces().len()
        ),
        (16, 24, 12)
    );
    assert!(brep.is_solid());
    // Source identities distinguish two coincident shells throughout assembly.
    assert_eq!(brep.edge_connected_face_components().len(), 2);
}

#[test]
fn ngon_midpoints_split_adjacent_quad_trims_without_splitting_their_surfaces() {
    let points = (0..2)
        .flat_map(|z| {
            (0..5).map(move |i| {
                let a = std::f64::consts::TAU * i as f64 / 5.;
                Point3::try_new(3. * a.cos(), 3. * a.sin(), 2. * z as f64).unwrap()
            })
        })
        .collect();
    let mut faces = (0..5)
        .map(|i| vec![i, (i + 1) % 5, (i + 1) % 5 + 5, i + 5])
        .collect::<Vec<_>>();
    faces.push(vec![4, 3, 2, 1, 0]);
    faces.push(vec![5, 6, 7, 8, 9]);
    let source = SubD::try_from_polygons(points, faces).unwrap();
    let archive = source.archive().to_vec();
    let brep = source.to_nurbs_brep_g0(Tolerance::DEFAULT).unwrap();
    assert_eq!(
        (
            brep.vertices().len(),
            brep.edges().len(),
            brep.faces().len()
        ),
        (22, 35, 15)
    );
    assert!(brep.is_solid());
    for face in &brep.faces()[..5] {
        assert_eq!(face.loops()[0].trims().len(), 6);
    }
    for face in &brep.faces()[5..] {
        assert_eq!(face.loops()[0].trims().len(), 4);
    }
    assert_eq!(source.archive(), archive);
}
