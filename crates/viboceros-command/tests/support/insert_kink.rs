use super::native_curve_comparison as curves;
use serde_json::Value;
use viboceros_document::{Document, Geometry, ObjectAttributes, ObjectId};
use viboceros_geometry::{NurbsSurface, Point3};
fn point(v: &Value) -> Point3 {
    Point3::try_from(serde_json::from_value::<[f64; 3]>(v.clone()).unwrap()).unwrap()
}
pub fn surface(s: &NurbsSurface, v: &Value) -> f64 {
    let mut gap = 0_f64;
    assert_eq!(s.degree_u(), v["degree"][0].as_u64().unwrap() as usize);
    assert_eq!(s.degree_v(), v["degree"][1].as_u64().unwrap() as usize);
    for (actual, key) in [(s.knots_u(), "knots_u"), (s.knots_v(), "knots_v")] {
        let expected = v[key].as_array().unwrap();
        assert_eq!(actual.len(), expected.len());
        for (a, b) in actual.iter().zip(expected) {
            gap = gap.max((a - b.as_f64().unwrap()).abs());
        }
    }
    assert_eq!(
        s.control_point_count_u(),
        v["control_count"][0].as_u64().unwrap() as usize
    );
    assert_eq!(
        s.control_point_count_v(),
        v["control_count"][1].as_u64().unwrap() as usize
    );
    for (domain, key) in [(s.domain_u(), "domain_u"), (s.domain_v(), "domain_v")] {
        gap = gap
            .max((*domain.start() - v[key][0].as_f64().unwrap()).abs())
            .max((*domain.end() - v[key][1].as_f64().unwrap()).abs());
    }
    let pts = v["control_points"].as_array().unwrap();
    assert_eq!(s.control_points().len(), pts.len());
    for (a, b) in s.control_points().iter().zip(pts) {
        gap = gap
            .max(a.point().distance_to(point(&b["point"])).unwrap())
            .max((a.weight() - b["weight"].as_f64().unwrap()).abs());
    }
    gap
}
pub fn check(doc: &Document, v: &Value, source: ObjectId, label: &str) -> f64 {
    check_with_weight_tolerance(doc, v, source, label, 0.)
}
/// Keep the absolute geometry checks and optionally admit relative rounding
/// in raw rational-curve weights. No controls or weights are normalized.
pub fn check_with_weight_tolerance(
    doc: &Document,
    v: &Value,
    source: ObjectId,
    label: &str,
    relative_weight_epsilon: f64,
) -> f64 {
    assert_eq!(
        doc.objects().len(),
        v.as_array().unwrap().len(),
        "{label}: count"
    );
    let mut gap = 0_f64;
    for row in v.as_array().unwrap() {
        let original = row["id"].as_str().unwrap() == source.to_string();
        let object = if original {
            doc.object(source).unwrap()
        } else {
            doc.objects().find(|o| o.id() != source).unwrap()
        };
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
        if !original {
            assert_eq!(
                object.attributes(),
                &ObjectAttributes::on_layer(doc.current_layer_id())
            );
            assert!(object.group_ids().is_empty());
        }
        let g = &row["geometry"];
        if let Some(curve) = object.geometry().curve_ref() {
            gap = gap.max(compare_curve(curve, g, label, relative_weight_epsilon));
        } else if let Geometry::Brep(b) = object.geometry() {
            assert_eq!(b.faces().len(), g["faces"].as_u64().unwrap() as usize);
            let topology = &g["topology"];
            let vertices = topology["vertices"].as_array().unwrap();
            assert_eq!(b.vertices().len(), vertices.len(), "{label}: vertices");
            for (a, c) in b.vertices().iter().zip(vertices) {
                gap = gap.max(a.point().distance_to(point(c)).unwrap());
            }
            let edges = topology["edges"].as_array().unwrap();
            assert_eq!(b.edges().len(), edges.len(), "{label}: edges");
            for (a, c) in b.edges().iter().zip(edges) {
                let d = a.curve().domain();
                gap = gap
                    .max(
                        a.curve()
                            .evaluate(*d.start())
                            .unwrap()
                            .distance_to(point(&c[0]))
                            .unwrap(),
                    )
                    .max(
                        a.curve()
                            .evaluate(*d.end())
                            .unwrap()
                            .distance_to(point(&c[1]))
                            .unwrap(),
                    );
            }
            let mut loop_index = 0;
            for (i, face) in b.faces().iter().enumerate() {
                assert_eq!(
                    face.is_reversed(),
                    topology["reversed"][i].as_bool().unwrap(),
                    "{label}: orientation"
                );
                assert_eq!(
                    face.loops().len(),
                    topology["loop_counts"][i].as_u64().unwrap() as usize,
                    "{label}: loops"
                );
                for l in face.loops() {
                    assert_eq!(
                        l.trims().len(),
                        topology["trim_counts"][loop_index].as_u64().unwrap() as usize,
                        "{label}: trims"
                    );
                    loop_index += 1;
                }
                gap = gap.max(surface(face.surface(), &g["surfaces"][i]));
                let samples = g["samples"][i].as_array().unwrap();
                for (j, p) in samples.iter().enumerate() {
                    let s = face.surface();
                    let u = *s.domain_u().start()
                        + (*s.domain_u().end() - *s.domain_u().start()) * (j % 9) as f64 / 8.;
                    let v = *s.domain_v().start()
                        + (*s.domain_v().end() - *s.domain_v().start()) * (j / 9) as f64 / 8.;
                    gap = gap.max(s.evaluate(u, v).unwrap().distance_to(point(p)).unwrap());
                }
            }
        } else {
            panic!("{label}: unexpected geometry");
        }
    }
    gap
}

fn normalize_outer_knots(curve: viboceros_geometry::CurveRef<'_>, g: &Value) -> Value {
    use viboceros_geometry::CurveRef;
    let mut expected = g.clone();
    match curve {
        CurveRef::NurbsCurve(c) => {
            // RhinoCommon omits the two unused outer knots. The capture pads
            // them by repetition; 3dm import restores their extrapolated values.
            // Every active knot, control, weight, domain and sample is checked.
            let knots = expected["definition"]["knots"].as_array_mut().unwrap();
            knots[0] = serde_json::json!(c.knots()[0]);
            let last = knots.len() - 1;
            knots[last] = serde_json::json!(c.knots()[last]);
        }
        CurveRef::PolyCurve(c) => {
            let segments = expected["segments"].as_array_mut().unwrap();
            assert_eq!(segments.len(), c.segments().len());
            for (a, b) in c.segments().iter().zip(segments) {
                *b = normalize_outer_knots(a.as_ref(), b);
            }
        }
        _ => {}
    }
    expected
}
fn compare_curve(
    curve: viboceros_geometry::CurveRef<'_>,
    g: &Value,
    label: &str,
    relative_weight_epsilon: f64,
) -> f64 {
    curves::compare_with_weight_tolerance(
        curve,
        &normalize_outer_knots(curve, g),
        label,
        relative_weight_epsilon,
    )
}
