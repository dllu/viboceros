use serde_json::Value;
use viboceros_command::{CommandRegistry, open_3dm_with_views};
use viboceros_document::{ObjectId, SelectionMode};
#[path = "support/mark_foci.rs"]
mod comparison;
#[allow(dead_code)]
#[path = "support/simplify_curves.rs"]
mod native_curve_comparison;
#[test]
fn native_foci_axes_sources_attributes_selection_and_history() {
    let capture: Value = serde_json::from_str(include_str!(
        "../../../tools/rhino_oracle/observations/mark_foci.json"
    ))
    .unwrap();
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../tools/rhino_oracle/fixtures/mark_foci");
    for row in capture["results"].as_array().unwrap() {
        let name = row["id"].as_str().unwrap();
        let v = &row["value"];
        let recipe = &v["recipe"];
        let registry = CommandRegistry::with_builtins();
        let mut doc = open_3dm_with_views(root.join(format!("{name}.3dm")).to_str().unwrap())
            .unwrap()
            .0;
        doc.clear_selection();
        let ids = v["source_ids"]
            .as_array()
            .unwrap()
            .iter()
            .map(|s| s.as_str().unwrap().parse::<ObjectId>().unwrap())
            .collect::<Vec<_>>();
        doc.select_objects_direct(ids, SelectionMode::Replace)
            .unwrap();
        doc.clear_history().unwrap();
        let before = doc.objects().cloned().collect::<Vec<_>>();
        let mut mapping = comparison::initial_mapping(&before);
        let command = format!(
            "MarkFoci DrawAxis={}",
            if recipe["draw_axis"] == true {
                "Yes"
            } else {
                "No"
            }
        );
        if recipe["preselect"] == true {
            registry.execute(&mut doc, &command)
        } else {
            registry.execute_postselected(&mut doc, &command, Default::default())
        }
        .unwrap_or_else(|e| panic!("{name}: {e}"));
        // Native fits this perturbed multi-span ellipse approximately. The
        // kernel requires a whole-curve conic certificate at model tolerance.
        if name == "definition_near-conic" {
            assert_eq!(doc.objects().cloned().collect::<Vec<_>>(), before);
            assert!(!doc.can_undo());
            eprintln!("MARK_FOCI_GUARD {name}");
            continue;
        }
        let mut maximum = comparison::check(&doc, &v["after"], &before, name, &mut mapping);
        let after = doc.objects().cloned().collect::<Vec<_>>();
        if !v["undo"].is_null() {
            assert_eq!(doc.undo_label(), Some("MarkFoci"));
            registry.execute(&mut doc, "Undo").unwrap();
            assert_eq!(doc.objects().cloned().collect::<Vec<_>>(), before);
            maximum = maximum.max(comparison::check(
                &doc,
                &v["undo"],
                &before,
                name,
                &mut mapping,
            ));
            assert!(!doc.can_undo());
            registry.execute(&mut doc, "Redo").unwrap();
            assert_eq!(doc.objects().cloned().collect::<Vec<_>>(), after);
            maximum = maximum.max(comparison::check(
                &doc,
                &v["redo"],
                &before,
                name,
                &mut mapping,
            ));
        } else {
            assert!(!doc.can_undo());
        }
        assert!(maximum <= 1e-8, "{name}: {maximum}");
        eprintln!(
            "MARK_FOCI_AUDIT {}",
            serde_json::json!({"id":name,"maximum_error":maximum})
        );
    }
}

#[test]
fn unsupported_sources_and_invalid_options_preserve_redo_and_preferences() {
    let registry = CommandRegistry::with_builtins();
    let mut doc = viboceros_document::Document::default();
    registry.execute(&mut doc, "Line 0,0,0 6,0,0").unwrap();
    let id = doc.objects().next().unwrap().id();
    doc.select_object(id, SelectionMode::Replace).unwrap();
    doc.clear_history().unwrap();
    registry.execute(&mut doc, "SetObjectName Future").unwrap();
    registry.execute(&mut doc, "Undo").unwrap();
    let before = doc.objects().cloned().collect::<Vec<_>>();
    registry.execute(&mut doc, "_MarkFoci DrawAxis=No").unwrap();
    for text in [
        "MarkFoci DrawAxis=invalid",
        "MarkFoci Copy=Yes",
        "MarkFoci DrawAxis=Yes DrawAxis=No",
        "MarkFoci 5",
    ] {
        assert!(registry.execute(&mut doc, text).is_err());
    }
    assert_eq!(doc.objects().cloned().collect::<Vec<_>>(), before);
    assert!(doc.can_redo());
    assert!(!doc.can_undo());
    let prompt = registry
        .object_selection_prompt("MarkFoci")
        .unwrap()
        .unwrap();
    assert!(!prompt.options[0].value);
    assert!(
        CommandRegistry::with_builtins()
            .object_selection_prompt("MarkFoci")
            .unwrap()
            .unwrap()
            .options[0]
            .value
    );
}
