//! Independent native command captures, using the same 3dm source definitions.
use serde_json::Value;
use viboceros_command::{CommandRegistry, open_3dm_with_views};
use viboceros_document::{ObjectId, SelectionMode};
#[path = "support/modify_radius.rs"]
mod comparison;
// Selection replay below maps native copy IDs before checking them.
#[allow(dead_code)]
#[path = "support/simplify_curves.rs"]
mod native_curve_comparison;
#[test]
fn radius_modes_representation_metadata_copy_selection_and_history_match_rhino() {
    let capture: Value = serde_json::from_str(include_str!(
        "../../../tools/rhino_oracle/observations/modify_radius.json"
    ))
    .unwrap();
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../tools/rhino_oracle/fixtures/modify_radius");
    for row in capture["results"].as_array().unwrap() {
        let name = row["id"].as_str().unwrap();
        let v = &row["value"];
        let recipe = &v["recipe"];
        assert!(
            matches!(v["command_result"].as_str(), Some("Success" | "Nothing")),
            "{name}"
        );
        let registry = CommandRegistry::with_builtins();
        let mut doc = open_3dm_with_views(root.join(format!("{name}.3dm")).to_str().unwrap())
            .unwrap()
            .0;
        doc.clear_selection();
        let post = recipe["preselect"] == false;
        let ids = v["before"]
            .as_array()
            .unwrap()
            .iter()
            .filter(|o| {
                o["selected"] == true
                    || (post && v["source_ids"].as_array().unwrap().contains(&o["id"]))
            })
            .map(|o| o["id"].as_str().unwrap().parse::<ObjectId>().unwrap())
            .collect::<Vec<_>>();
        doc.select_objects_direct(ids, SelectionMode::Replace)
            .unwrap();
        doc.clear_history().unwrap();
        let before = doc.objects().cloned().collect::<Vec<_>>();
        let mut mapping = comparison::initial_mapping(&before);
        let command = format!(
            "ModifyRadius {} {} Copy={}",
            recipe["mode"].as_str().unwrap(),
            recipe["value"].as_f64().unwrap(),
            if recipe["copy"] == true { "Yes" } else { "No" }
        );
        if post {
            registry.execute_postselected(&mut doc, &command, Default::default())
        } else {
            registry.execute(&mut doc, &command)
        }
        .unwrap_or_else(|e| panic!("{name}: {e}"));
        let mut maximum = comparison::check(&doc, &v["after"], &before, name, &mut mapping);
        let after = doc.objects().cloned().collect::<Vec<_>>();
        if !v["undo"].is_null() {
            assert_eq!(doc.undo_label(), Some("ModifyRadius"), "{name}");
            registry.execute(&mut doc, "Undo").unwrap();
            assert_eq!(
                doc.objects().cloned().collect::<Vec<_>>(),
                before,
                "{name}: undo"
            );
            maximum = maximum.max(comparison::check(
                &doc,
                &v["undo"],
                &before,
                name,
                &mut mapping,
            ));
            assert!(!doc.can_undo(), "{name}: single history step");
            registry.execute(&mut doc, "Redo").unwrap();
            assert_eq!(
                doc.objects().cloned().collect::<Vec<_>>(),
                after,
                "{name}: redo"
            );
            maximum = maximum.max(comparison::check(
                &doc,
                &v["redo"],
                &before,
                name,
                &mut mapping,
            ));
        } else if name == "polycurve" || v["command_result"] == "Nothing" {
            assert!(!doc.can_undo(), "{name}: no-op");
        }
        assert!(maximum <= 1e-8, "{name}: {maximum}");
        eprintln!(
            "MODIFY_RADIUS_AUDIT {}",
            serde_json::json!({"id":name,"maximum_error":maximum})
        );
    }
}
