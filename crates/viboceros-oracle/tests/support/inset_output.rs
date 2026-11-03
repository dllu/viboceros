//! Bijective output-locus and metadata checks independent of native table order.
use super::{common, native_surface};
use serde_json::Value;
use viboceros_document::{Document, Geometry, Object, ObjectId};
use viboceros_geometry::{CurveSegment3, NurbsCurve, PolyCurve3};

pub fn assert_document(
    doc: &Document,
    source: ObjectId,
    expected: &Value,
    epsilon: f64,
    label: &str,
) {
    let expected = expected.as_array().unwrap();
    assert_eq!(doc.objects().len(), expected.len(), "{label}: object count");
    let mut used = std::collections::BTreeSet::new();
    for (i, row) in expected.iter().enumerate() {
        let object = doc
            .objects()
            .find(|o| {
                !used.contains(&o.id())
                    && (row["source"] != 0 || o.id() == source)
                    && matches_geometry(o.geometry(), row, epsilon)
            })
            .unwrap_or_else(|| {
                let actual = doc
                    .objects()
                    .map(|o| match o.geometry() {
                        Geometry::Brep(b) => format!(
                            "Brep: {} faces, {} edges, {} vertices",
                            b.faces().len(),
                            b.edges().len(),
                            b.vertices().len()
                        ),
                        g => {
                            if let Some(c) = g.curve_ref() {
                                let c = c.to_nurbs().unwrap();
                                let n = curve_record(&row["curve"]);
                                format!(
                                    "curve closed={:?}, witness {:?} / {:?}",
                                    c.is_closed(),
                                    native_surface::curve_witnesses(&c, &n, epsilon),
                                    native_surface::curve_witnesses(&n, &c, epsilon)
                                )
                            } else {
                                format!("{g:?}")
                            }
                        }
                    })
                    .collect::<Vec<_>>();
                panic!("{label}: no unused output matches row {i}: {actual:?}")
            });
        used.insert(object.id());
        assert_metadata(doc, object, source, row, label);
        if let Geometry::Brep(b) = object.geometry() {
            native_surface::assert_geometry_with_epsilon(
                b,
                &row["brep"],
                doc.tolerance(),
                epsilon,
                label,
            );
        }
    }
}
fn assert_metadata(doc: &Document, o: &Object, source: ObjectId, row: &Value, label: &str) {
    let a = o.attributes();
    let c = a.object_color();
    let source_layer = doc.layer(a.layer_id()).unwrap().name() == "Inset surface source";
    let groups = o
        .group_ids()
        .iter()
        .map(|&g| {
            assert_eq!(doc.group(g).unwrap().name(), Some("Inset surface group"));
            0
        })
        .collect::<Vec<_>>();
    let actual = serde_json::json!({"source":(o.id()==source).then_some(0),"name":a.name(),
        "selected":doc.is_selected(o.id()),"layer":source_layer.then_some(0),
        "color":[c.red,c.green,c.blue],"groups":groups});
    let expected = serde_json::json!({"source":row["source"],"name":row["name"],
        "selected":row["selected"],"layer":row["layer"],"color":row["color"],"groups":row["groups"]});
    common::compare(&actual, &expected, label);
    if row["source"].is_null() {
        assert_eq!(
            a.layer_id(),
            doc.current_layer_id(),
            "{label}: creation layer"
        );
    }
}
fn matches_geometry(g: &Geometry, row: &Value, epsilon: f64) -> bool {
    if let Geometry::Brep(b) = g {
        let Some(vertices) = row["brep"]["vertices"].as_array() else {
            return false;
        };
        return b.faces().len() == row["brep"]["faces"].as_array().unwrap().len()
            && b.edges().len() == row["brep"]["edges"].as_array().unwrap().len()
            && b.vertices().len() == vertices.len()
            && b.vertices().iter().all(|v| {
                vertices.iter().any(|w| {
                    let p = w["point"].as_array().unwrap();
                    v.point()
                        .to_array()
                        .iter()
                        .zip(p)
                        .all(|(a, b)| (a - b.as_f64().unwrap()).abs() <= epsilon)
                })
            });
    }
    if row["curve"].is_null() {
        return false;
    }
    let Some(curve) = g.curve_ref() else {
        return false;
    };
    if curve.is_closed().unwrap() != row["curve"]["closed"].as_bool().unwrap() {
        return false;
    }
    let actual = curve.to_nurbs().unwrap();
    let expected = curve_record(&row["curve"]);
    native_surface::curve_witnesses(&actual, &expected, epsilon).is_ok()
        && native_surface::curve_witnesses(&expected, &actual, epsilon).is_ok()
}
fn curve_record(v: &Value) -> NurbsCurve {
    if !v["definition"].is_null() {
        return native_surface::nurbs_definition(&v["definition"]);
    }
    match v["type"].as_str().unwrap() {
        "line" => {
            let samples = v["samples"].as_array().unwrap();
            let point = |p: &Value| {
                viboceros_geometry::Point3::try_from([
                    p[0].as_f64().unwrap(),
                    p[1].as_f64().unwrap(),
                    p[2].as_f64().unwrap(),
                ])
                .unwrap()
            };
            NurbsCurve::try_new(
                1,
                vec![point(&samples[0]), point(samples.last().unwrap())],
                vec![0., 0., 1., 1.],
            )
            .unwrap()
        }
        "polycurve" => PolyCurve3::try_new(
            v["segments"]
                .as_array()
                .unwrap()
                .iter()
                .map(|s| CurveSegment3::NurbsCurve(curve_record(s)))
                .collect::<Vec<_>>(),
        )
        .unwrap()
        .to_nurbs()
        .unwrap(),
        kind => panic!("missing native definition for {kind}"),
    }
}
