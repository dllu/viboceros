#[path = "../../viboceros-geometry/tests/support/polyline_closure.rs"]
mod reference;
use serde_json::Value;
use std::collections::BTreeSet;
use viboceros_command::{CommandRegistry, open_3dm_with_views};
use viboceros_document::{Geometry, GripMode, ObjectId, SelectionMode};
use viboceros_geometry::{AffineTransform3, Tolerance, Vector3};
#[test]
fn native_closure_selection_grips_extraction_and_noop_history_survive_3dm() {
    let data = reference::capture();
    let v = &data["results"][0]["value"];
    let registry = CommandRegistry::with_builtins();
    let mut doc = open_3dm_with_views(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../tools/rhino_oracle/fixtures/polyline_closure.3dm"
    ))
    .unwrap()
    .0;
    doc.clear_selection();
    doc.clear_history().unwrap();
    let before = doc.objects().cloned().collect::<Vec<_>>();
    for row in v["rows"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|r| r["valid"] == true)
    {
        let id = row["id"].as_str().unwrap().parse::<ObjectId>().unwrap();
        let object = doc.object(id).unwrap();
        let name = row["recipe"]["id"].as_str().unwrap();
        assert_eq!(
            object.geometry().curve_ref().unwrap().is_closed().unwrap(),
            row["closed"].as_bool().unwrap(),
            "{name}"
        );
        let points = object.geometry().grip_locations(GripMode::Control).unwrap();
        assert!(
            reference::compare_points(&points, &row["grips"], name, true) <= 1e-8,
            "{name}: grips"
        );
        if let Some(moved) = row.get("move_grip") {
            let result = object
                .geometry()
                .with_transformed_grips(
                    &[row["recipe"]["grip_index"].as_u64().unwrap_or(0) as usize]
                        .into_iter()
                        .collect(),
                    AffineTransform3::from_translation(
                        Vector3::try_from(
                            serde_json::from_value::<[f64; 3]>(
                                row["recipe"]
                                    .get("move_vector")
                                    .cloned()
                                    .unwrap_or(serde_json::json!([1., 2., 3.])),
                            )
                            .unwrap(),
                        )
                        .unwrap(),
                    ),
                    Tolerance::NUMERICAL_VALIDATION,
                )
                .unwrap();
            let expected = Geometry::from(reference::curve(&moved["value"]));
            assert_eq!(
                result, expected,
                "{name}: seam edit geometry and representation"
            );
        }
    }
    for name in ["SelClosedCrv", "SelOpenCrv"] {
        doc.clear_selection();
        registry.execute(&mut doc, name).unwrap();
        let expected = v["queries"][name]["value"]
            .as_array()
            .unwrap()
            .iter()
            .map(|s| s.as_str().unwrap().to_string())
            .collect::<BTreeSet<_>>();
        assert_eq!(
            doc.selected_object_ids()
                .map(|id| id.to_string())
                .collect::<BTreeSet<_>>(),
            expected,
            "{name}"
        );
        assert!(!doc.can_undo());
    }
    doc.clear_selection();
    registry.execute(&mut doc, "SelClosedCrv").unwrap();
    registry.execute(&mut doc, "SetObjectName Future").unwrap();
    registry.execute(&mut doc, "Undo").unwrap();
    registry.execute(&mut doc, "CloseCrv").unwrap();
    assert_eq!(doc.objects().cloned().collect::<Vec<_>>(), before);
    assert!(doc.can_redo());
    assert!(!doc.can_undo());
    for row in v["rows"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|r| r.get("extract").is_some())
    {
        let name = row["recipe"]["id"].as_str().unwrap();
        let id = row["id"].as_str().unwrap().parse::<ObjectId>().unwrap();
        doc.select_objects_direct([id], SelectionMode::Replace)
            .unwrap();
        doc.clear_history().unwrap();
        registry.execute(&mut doc, "ExtractPt").unwrap();
        let actual = doc
            .objects()
            .filter_map(|o| {
                if let Geometry::Point(p) = o.geometry() {
                    Some(*p)
                } else {
                    None
                }
            })
            .collect::<Vec<_>>();
        let expected = Value::Array(
            row["extract"]["value"]
                .as_array()
                .unwrap()
                .iter()
                .map(|o| o["point"].clone())
                .collect(),
        );
        assert!(
            reference::compare_points(&actual, &expected, name, false) <= 1e-8,
            "{name}: points"
        );
        registry.execute(&mut doc, "Undo").unwrap();
        assert_eq!(doc.objects().cloned().collect::<Vec<_>>(), before);
    }
    let path = std::env::temp_dir().join(format!(
        "viboceros-polyline-closure-{}.3dm",
        std::process::id()
    ));
    registry
        .execute(&mut doc, &format!("SaveAs {}", path.display()))
        .unwrap();
    let read = open_3dm_with_views(path.to_str().unwrap()).unwrap().0;
    std::fs::remove_file(path).unwrap();
    assert_eq!(read.objects().len(), before.len());
    for source in &before {
        let object = read.object(source.id()).unwrap();
        assert_eq!(
            object.geometry(),
            source.geometry(),
            "{}: exact stored geometry",
            source.attributes().name().unwrap()
        );
        assert_eq!(object.geometry_user_text(), source.geometry_user_text());
        assert_eq!(object.attributes().name(), source.attributes().name());
        assert_eq!(
            object.attributes().user_text(),
            source.attributes().user_text()
        );
        assert_eq!(
            read.layer(object.attributes().layer_id()).unwrap().name(),
            doc.layer(source.attributes().layer_id()).unwrap().name()
        );
    }
}
