//! Replay actual SimplifyCrv commands from the same native 3dm sources.
use serde_json::Value;
use viboceros_command::{CommandRegistry, open_3dm_with_views};
use viboceros_document::{Geometry, ObjectId, SelectionMode};
#[path = "support/simplify_curves.rs"]
mod comparison;
use comparison::compare;
#[test]
fn native_curve_classes_parameterization_metadata_and_history_replay() {
    let capture: Value = serde_json::from_str(include_str!(
        "../../../tools/rhino_oracle/observations/simplify_curves.json"
    ))
    .unwrap();
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tools/rhino_oracle");
    let registry = CommandRegistry::with_builtins();
    for row in capture["results"].as_array().unwrap() {
        let name = row["id"].as_str().unwrap();
        let v = &row["value"];
        assert_eq!(v["success"], true, "{name}");
        let (mut doc, _, _, _) = open_3dm_with_views(
            root.join(format!("fixtures/simplify_curves/{name}.3dm"))
                .to_str()
                .unwrap(),
        )
        .unwrap();
        doc.clear_selection();
        let ids = v["before"]
            .as_array()
            .unwrap()
            .iter()
            .filter(|o| {
                o["selected"] == true
                    || (v["preselect"] == false
                        && v["source_ids"].as_array().unwrap().contains(&o["id"]))
            })
            .map(|o| o["id"].as_str().unwrap().parse::<ObjectId>().unwrap())
            .collect::<Vec<_>>();
        doc.select_objects_direct(ids, SelectionMode::Replace)
            .unwrap();
        doc.clear_history().unwrap();
        let before = doc.objects().cloned().collect::<Vec<_>>();
        let result = if v["preselect"] == false {
            registry.execute_postselected(&mut doc, "SimplifyCrv", Default::default())
        } else {
            registry.execute(&mut doc, "SimplifyCrv")
        };
        result.unwrap_or_else(|e| panic!("{name}: {e}"));
        assert_eq!(doc.objects().len(), before.len());
        let mut maximum = 0_f64;
        for expected in v["after"].as_array().unwrap() {
            let native_before = v["before"]
                .as_array()
                .unwrap()
                .iter()
                .find(|o| o["id"] == expected["id"])
                .unwrap();
            for key in [
                "name",
                "layer",
                "color_source",
                "color",
                "groups",
                "user_text",
            ] {
                assert_eq!(
                    native_before[key], expected[key],
                    "{name}: native {key} retention"
                );
            }
            let id = expected["id"]
                .as_str()
                .unwrap()
                .parse::<ObjectId>()
                .unwrap();
            let object = doc.object(id).unwrap();
            let original = before.iter().find(|o| o.id() == id).unwrap();
            assert_eq!(
                object.attributes(),
                original.attributes(),
                "{name}: attributes"
            );
            assert_eq!(object.group_ids(), original.group_ids(), "{name}: groups");
            assert_eq!(
                doc.is_selected(id),
                expected["selected"].as_bool().unwrap(),
                "{name}: selection"
            );
            assert_eq!(
                serde_json::to_value(object.geometry_user_text()).unwrap(),
                expected["geometry_text"],
                "{name}: geometry user text"
            );
            if let Some(curve) = object.geometry().curve_ref() {
                maximum = maximum.max(compare(curve, &expected["geometry"], name));
            } else {
                assert!(matches!(object.geometry(), Geometry::Point(_)));
                assert_eq!(object, original);
            }
        }
        assert!(maximum <= 1e-8, "{name}: error {maximum}");
        let after = doc.objects().cloned().collect::<Vec<_>>();
        if before != after {
            assert_eq!(doc.undo_label(), Some("SimplifyCrv"));
            registry.execute(&mut doc, "Undo").unwrap();
            assert_eq!(
                doc.objects().cloned().collect::<Vec<_>>(),
                before,
                "{name}: undo"
            );
            comparison::compare_selection(&doc, &v["undo"], name);
            assert!(!doc.can_undo());
            registry.execute(&mut doc, "Redo").unwrap();
            assert_eq!(
                doc.objects().cloned().collect::<Vec<_>>(),
                after,
                "{name}: redo"
            );
            comparison::compare_selection(&doc, &v["redo"], name);
        } else {
            assert!(!doc.can_undo(), "{name}: no-op");
        }
        eprintln!(
            "SIMPLIFY_CURVE_AUDIT {}",
            serde_json::json!({"id":name,"maximum_error":maximum,"changed":before!=after})
        );
    }
}
