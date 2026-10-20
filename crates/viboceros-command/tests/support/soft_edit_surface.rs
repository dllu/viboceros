use serde_json::Value;
use viboceros_document::{Document, Geometry, ObjectAttributes, ObjectId};
use viboceros_geometry::Point3;
fn point(v: &Value) -> Point3 {
    Point3::try_from(serde_json::from_value::<[f64; 3]>(v.clone()).unwrap()).unwrap()
}
fn multiset_gap<T>(actual: &[T], expected: &[Value], distance: impl Fn(&T, &Value) -> f64) -> f64 {
    assert_eq!(actual.len(), expected.len());
    let mut unused = (0..expected.len()).collect::<Vec<_>>();
    let mut gap = 0_f64;
    for a in actual {
        let (index, d) = unused
            .iter()
            .enumerate()
            .map(|(i, j)| (i, distance(a, &expected[*j])))
            .min_by(|a, b| a.1.total_cmp(&b.1))
            .unwrap();
        unused.remove(index);
        gap = gap.max(d);
    }
    gap
}
/// Geometry is compared independently of B-rep vertex/edge enumeration. The
/// complete tensor, topology counts, endpoint multisets and 81 samples remain
/// absolute checks. Only inactive outer knots and raw-weight rounding follow
/// the established 3dm import conventions.
pub fn check(doc: &Document, expected: &Value, source: ObjectId, label: &str) -> f64 {
    let rows = expected.as_array().unwrap();
    assert_eq!(doc.objects().len(), rows.len(), "{label}: object count");
    let original = rows
        .iter()
        .find(|o| o["id"].as_str().unwrap() == source.to_string())
        .unwrap();
    let mut pairs = vec![(doc.object(source).unwrap(), original)];
    pairs.extend(
        doc.objects().filter(|o| o.id() != source).zip(
            rows.iter()
                .filter(|o| o["id"].as_str().unwrap() != source.to_string())
                .rev(),
        ),
    );
    let mut gap = 0_f64;
    for (object, row) in pairs {
        assert_eq!(
            doc.is_selected(object.id()),
            row["selected"].as_bool().unwrap(),
            "{label}: selection"
        );
        assert_eq!(
            serde_json::to_value(object.geometry_user_text()).unwrap(),
            row["geometry_text"],
            "{label}: geometry text"
        );
        if object.id() != source {
            assert_eq!(
                object.attributes(),
                &ObjectAttributes::on_layer(doc.current_layer_id())
            );
            assert!(object.group_ids().is_empty());
        }
        let Geometry::Brep(b) = object.geometry() else {
            panic!("{label}: expected B-rep")
        };
        let g = &row["geometry"];
        assert_eq!(b.faces().len(), g["faces"].as_u64().unwrap() as usize);
        let topology = &g["topology"];
        gap = gap.max(multiset_gap(
            b.vertices(),
            topology["vertices"].as_array().unwrap(),
            |a, c| a.point().distance_to(point(c)).unwrap(),
        ));
        gap = gap.max(multiset_gap(
            b.edges(),
            topology["edges"].as_array().unwrap(),
            |a, c| {
                let d = a.curve().domain();
                let p = a.curve().evaluate(*d.start()).unwrap();
                let q = a.curve().evaluate(*d.end()).unwrap();
                p.distance_to(point(&c[0]))
                    .unwrap()
                    .max(q.distance_to(point(&c[1])).unwrap())
                    .min(
                        p.distance_to(point(&c[1]))
                            .unwrap()
                            .max(q.distance_to(point(&c[0])).unwrap()),
                    )
            },
        ));
        for (i, face) in b.faces().iter().enumerate() {
            assert_eq!(
                face.is_reversed(),
                topology["reversed"][i].as_bool().unwrap()
            );
            assert_eq!(
                face.loops().len(),
                topology["loop_counts"][i].as_u64().unwrap() as usize
            );
            let mut counts = face
                .loops()
                .iter()
                .map(|l| l.trims().len())
                .collect::<Vec<_>>();
            counts.sort();
            let mut native = topology["trim_counts"]
                .as_array()
                .unwrap()
                .iter()
                .map(|v| v.as_u64().unwrap() as usize)
                .collect::<Vec<_>>();
            native.sort();
            assert_eq!(counts, native);
            let mut s = g["surfaces"][i].clone();
            let surface = face.surface();
            for (knots, key) in [
                (surface.knots_u(), "knots_u"),
                (surface.knots_v(), "knots_v"),
            ] {
                let values = s[key].as_array_mut().unwrap();
                assert_eq!(values.len(), knots.len());
                let last = values.len() - 1;
                values[0] = knots[0].into();
                values[last] = knots[last].into();
            }
            for (p, e) in surface
                .control_points()
                .iter()
                .zip(s["control_points"].as_array_mut().unwrap())
            {
                let weight = e["weight"].as_f64().unwrap();
                assert!(
                    (p.weight() - weight).abs() <= 1e-12 * p.weight().abs().max(weight.abs()),
                    "{label}: weight"
                );
                e["weight"] = p.weight().into();
            }
            gap = gap.max(super::comparison::surface(surface, &s));
            for (j, p) in g["samples"][i].as_array().unwrap().iter().enumerate() {
                gap = gap.max(
                    surface
                        .evaluate(
                            surface.parameter_at_u((j % 9) as f64 / 8.).unwrap(),
                            surface.parameter_at_v((j / 9) as f64 / 8.).unwrap(),
                        )
                        .unwrap()
                        .distance_to(point(p))
                        .unwrap(),
                );
            }
        }
    }
    gap
}

/// Retained UV definitions and complete spatial edge samples are checked
/// independently of component numbering. Tolerances are measured bounds in our
/// representation, rather than numerical copies of native tolerance estimates.
pub fn check_boundaries(doc: &Document, value: &Value, source: ObjectId, label: &str) -> f64 {
    let Some(rows) = value["accepted_validity"].as_array() else {
        return 0.;
    };
    let original = rows
        .iter()
        .find(|r| r["id"].as_str().unwrap() == source.to_string())
        .unwrap();
    let mut pairs = vec![(doc.object(source).unwrap(), original)];
    pairs.extend(
        doc.objects().filter(|o| o.id() != source).zip(
            rows.iter()
                .filter(|r| r["id"].as_str().unwrap() != source.to_string())
                .rev(),
        ),
    );
    let mut gap = 0_f64;
    for (object, row) in pairs {
        let Geometry::Brep(b) = object.geometry() else {
            panic!("{label}: B-rep");
        };
        let face = &b.faces()[0];
        let trims = face
            .loops()
            .iter()
            .flat_map(|l| l.trims())
            .collect::<Vec<_>>();
        let expected = row["trims"].as_array().unwrap();
        assert_eq!(trims.len(), expected.len(), "{label}: trims");
        for (trim, native) in trims.into_iter().zip(expected) {
            assert_eq!(
                format!("{:?}", trim.trim_type()),
                native["type"].as_str().unwrap(),
                "{label}: trim type"
            );
            let curve = trim.curve();
            let definition = &native["uv_curve"]["definition"];
            assert_eq!(
                curve.degree(),
                definition["degree"].as_u64().unwrap() as usize
            );
            assert_eq!(
                curve.control_points().len(),
                definition["control_points"].as_array().unwrap().len()
            );
            for (a, e) in curve
                .control_points()
                .iter()
                .zip(definition["control_points"].as_array().unwrap())
            {
                let p = point(&e["point"]);
                assert!(
                    (a.point().x() - p.x()).abs() <= 1e-12 * p.x().abs().max(1.),
                    "{label}: U control"
                );
                assert!(
                    (a.point().y() - p.y()).abs() <= 1e-12 * p.y().abs().max(1.),
                    "{label}: V control"
                );
                assert_eq!(a.weight(), e["weight"].as_f64().unwrap());
            }
            let knots = definition["knots"].as_array().unwrap();
            assert_eq!(curve.knots().len(), knots.len());
            for (a, e) in curve.knots().iter().zip(knots) {
                assert!(
                    (a - e.as_f64().unwrap()).abs() <= 1e-12 * a.abs().max(1.),
                    "{label}: UV knot"
                );
            }
            let mut edge_gap = [0_f64; 2];
            for i in 0..=16 {
                let f = i as f64 / 16.;
                let uv = curve.evaluate(curve.parameter_at(f).unwrap()).unwrap();
                let native_uv = point(&native["uv"][i]);
                assert!(
                    (uv.x() - native_uv.x()).abs() <= 1e-12 * uv.x().abs().max(1.),
                    "{label}: U sample"
                );
                assert!(
                    (uv.y() - native_uv.y()).abs() <= 1e-12 * uv.y().abs().max(1.),
                    "{label}: V sample"
                );
                let p = face.surface().evaluate(uv.x(), uv.y()).unwrap();
                gap = gap.max(p.distance_to(point(&native["surface_points"][i])).unwrap());
                if let Some(edge) = trim.edge() {
                    let edge = &b.edges()[edge];
                    let t = if trim.is_reversed_3d() { 1. - f } else { f };
                    let q = edge
                        .curve()
                        .evaluate(edge.curve().parameter_at(t).unwrap())
                        .unwrap();
                    assert!(
                        p.distance_to(q).unwrap()
                            <= doc.tolerance().absolute().max(edge.tolerance()),
                        "{label}: measured edge bound"
                    );
                    for (direction, error) in edge_gap.iter_mut().enumerate() {
                        let q = edge
                            .curve()
                            .evaluate(edge.curve().parameter_at(f).unwrap())
                            .unwrap();
                        *error = error.max(
                            q.distance_to(point(
                                &native["edge_points"][if direction == 0 { i } else { 16 - i }],
                            ))
                            .unwrap(),
                        );
                    }
                } else {
                    assert!(native["edge_points"].is_null());
                    let vertex = &b.vertices()[trim.vertices()[0]];
                    assert!(
                        p.distance_to(vertex.point()).unwrap()
                            <= doc.tolerance().absolute().max(vertex.tolerance()),
                        "{label}: measured singular bound"
                    );
                }
            }
            if trim.edge().is_some() {
                gap = gap.max(edge_gap[0].min(edge_gap[1]));
            }
        }
    }
    gap
}
