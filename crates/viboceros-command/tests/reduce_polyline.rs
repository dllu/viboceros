//! Public command replays, with invalid native collapses tested as atomic errors.
use serde_json::Value;
use viboceros_command::{CommandRegistry, open_3dm_with_views};
use viboceros_document::{ObjectId, SelectionMode};
#[path = "support/modify_radius.rs"]
mod comparison;
#[allow(dead_code)]
#[path = "support/simplify_curves.rs"]
mod native_curve_comparison;

#[test]
fn native_reductions_parameters_metadata_copy_selection_and_history_replay() {
    let capture: Value = serde_json::from_str(include_str!(
        "../../../tools/rhino_oracle/observations/reduce_polyline.json"
    ))
    .unwrap();
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../tools/rhino_oracle/fixtures/reduce_polyline");
    let mut guarded = 0;
    let mut matched = 0;
    for row in capture["results"].as_array().unwrap() {
        let name = row["id"].as_str().unwrap();
        let v = &row["value"];
        let recipe = &v["recipe"];
        let registry = CommandRegistry::with_builtins();
        let mut doc = open_3dm_with_views(root.join(format!("{name}.3dm")).to_str().unwrap())
            .unwrap()
            .0;
        doc.clear_selection();
        let post = recipe["preselect"] == false;
        let ids = v["source_ids"]
            .as_array()
            .unwrap()
            .iter()
            .map(|o| o.as_str().unwrap().parse::<ObjectId>().unwrap())
            .collect::<Vec<_>>();
        doc.select_objects_direct(ids, SelectionMode::Replace)
            .unwrap();
        doc.clear_history().unwrap();
        let before = doc.objects().cloned().collect::<Vec<_>>();
        let mut mapping = comparison::initial_mapping(&before);
        let command = format!(
            "ReducePolyline {} Copy={}",
            recipe["reduction_tolerance"].as_f64().unwrap(),
            if recipe["copy"] == true { "Yes" } else { "No" }
        );
        let result = if post {
            registry.execute_postselected(&mut doc, &command, Default::default())
        } else {
            registry.execute(&mut doc, &command)
        };
        if v["after"]
            .as_array()
            .unwrap()
            .iter()
            .any(|o| o["geometry"]["type"] == "invalid_polyline")
        {
            assert!(
                result.unwrap_err().to_string().contains("collapse"),
                "{name}"
            );
            assert_eq!(
                doc.objects().cloned().collect::<Vec<_>>(),
                before,
                "{name}: atomic rejection"
            );
            assert!(!doc.can_undo());
            guarded += 1;
            continue;
        }
        result.unwrap_or_else(|e| panic!("{name}: {e}"));
        let mut maximum = comparison::check(&doc, &v["after"], &before, name, &mut mapping);
        let after = doc.objects().cloned().collect::<Vec<_>>();
        if !v["undo"].is_null() {
            assert_eq!(doc.undo_label(), Some("ReducePolyline"), "{name}");
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
            assert!(!doc.can_undo());
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
        } else {
            assert!(!doc.can_undo(), "{name}: no-op");
        }
        assert!(maximum <= 1e-8, "{name}: {maximum}");
        matched += 1;
        eprintln!(
            "REDUCE_POLYLINE_AUDIT {}",
            serde_json::json!({"id":name,"maximum_error":maximum})
        );
    }
    assert!(matched >= 40);
    assert_eq!(guarded, 2);
}
