use serde_json::Value;
use viboceros_command::CommandRegistry;
use viboceros_document::{Document, Geometry, ObjectId, SelectionMode};
use viboceros_geometry::{NurbsCurve, Point3};
pub fn history_snapshot(
    doc: &viboceros_document::Document,
    id: ObjectId,
    marker: ObjectId,
) -> Value {
    let object = doc.object(id).unwrap();
    let c = object.geometry().curve_ref().unwrap().to_nurbs().unwrap();
    let mut knots = c.knots().to_vec();
    let last = knots.len() - 1;
    knots[0] = knots[1];
    knots[last] = knots[last - 1];
    serde_json::json!({"same_id":true,"selected":doc.is_selected(id),"name":object.attributes().name(),"grips_on":doc.grip_mode(id).is_some(),"geometry_text":object.geometry_user_text().get("GeometryCode"),"marker":doc.object(marker).is_some(),
    "curve":{"degree":c.degree(),"control_points":c.control_points().iter().map(|p|serde_json::json!({"point":p.point().to_array(),"weight":p.weight()})).collect::<Vec<_>>(),"knots":knots,"domain":[*c.domain().start(),*c.domain().end()]}})
}
pub fn close_json(actual: &Value, expected: &Value, path: &str) {
    match (actual, expected) {
        (Value::Number(a), Value::Number(b)) => {
            let (a, b) = (a.as_f64().unwrap(), b.as_f64().unwrap());
            assert!(
                (a - b).abs() <= 1e-8_f64.max(1e-12 * a.abs().max(b.abs())),
                "{path}: {a} != {b}"
            );
        }
        (Value::Object(a), Value::Object(b)) => {
            assert!(a.keys().eq(b.keys()), "{path}");
            for (k, a) in a {
                close_json(a, &b[k], &format!("{path}/{k}"));
            }
        }
        (Value::Array(a), Value::Array(b)) => {
            assert_eq!(a.len(), b.len(), "{path}");
            for (i, (a, b)) in a.iter().zip(b).enumerate() {
                close_json(a, b, &format!("{path}/{i}"));
            }
        }
        _ => assert_eq!(actual, expected, "{path}"),
    }
}

pub fn history_document(op: &Value) -> (Document, ObjectId, ObjectId) {
    let mut doc = Document::default();
    let curve = NurbsCurve::try_clamped_uniform(
        3,
        [[0., 0., 0.], [1., 3., 0.], [3., -1., 0.], [5., 0., 0.]]
            .into_iter()
            .map(|p| Point3::try_from(p).unwrap())
            .collect(),
    )
    .unwrap();
    let id = doc.add_geometry(Geometry::NurbsCurve(curve)).unwrap();
    doc.select_object(id, SelectionMode::Replace).unwrap();
    CommandRegistry::with_builtins()
        .execute(&mut doc, "SetObjectName source")
        .unwrap();
    if op["geometry_text"] == true {
        doc.set_object_geometry_user_text([id], "GeometryCode", Some("source"))
            .unwrap();
    }
    doc.clear_selection();
    doc.clear_history().unwrap();
    let marker = doc
        .add_geometry(Geometry::Point(Point3::try_new(30., 30., 30.).unwrap()))
        .unwrap();
    doc.undo().unwrap();
    if op["preselect"] == true {
        doc.select_object(id, SelectionMode::Replace).unwrap();
    }
    (doc, id, marker)
}
