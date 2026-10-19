use super::*;
use crate::viewport::SelectionClick;
use viboceros_document::SelectionMode;
#[path = "../../../crates/viboceros-command/tests/support/mark_foci.rs"]
mod comparison;
fn enter(app: &mut VibocerosApp, text: &str) {
    app.command_input = text.into();
    app.run_command();
}
#[test]
fn mark_foci_native_options_selection_sources_and_undo_redo_through_ui() {
    let capture: serde_json::Value = serde_json::from_str(include_str!(
        "../../../tools/rhino_oracle/observations/mark_foci.json"
    ))
    .unwrap();
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tools/rhino_oracle/fixtures/mark_foci");
    for row in capture["results"].as_array().unwrap() {
        let name = row["id"].as_str().unwrap();
        let v = &row["value"];
        let recipe = &v["recipe"];
        let mut app = test_app();
        app.document = viboceros_command::open_3dm_with_views(
            root.join(format!("{name}.3dm")).to_str().unwrap(),
        )
        .unwrap()
        .0;
        app.document.clear_selection();
        let ids = v["source_ids"]
            .as_array()
            .unwrap()
            .iter()
            .map(|p| p.as_str().unwrap().parse::<ObjectId>().unwrap())
            .collect::<Vec<_>>();
        if recipe["preselect"] == true {
            app.document
                .select_objects_direct(ids.clone(), SelectionMode::Replace)
                .unwrap();
        }
        app.document.clear_history().unwrap();
        let before = app.document.objects().cloned().collect::<Vec<_>>();
        let mut mapping = comparison::initial_mapping(&before);
        if recipe["preselect"] == true {
            enter(&mut app, "MarkFoci DrawAxis=Yes");
        } else {
            enter(&mut app, "MarkFoci");
            assert_eq!(
                app.viewport_object_filter(),
                Some(viboceros_command::ObjectSelectionFilter::Curves)
            );
            enter(
                &mut app,
                if recipe["draw_axis"] == true {
                    "DrawAxis=Yes"
                } else {
                    "DrawAxis=No"
                },
            );
            for id in ids {
                app.apply_selection_click(SelectionClick {
                    object_id: Some(id),
                    mode: SelectionMode::Add,
                });
            }
            assert_eq!(app.document.objects().cloned().collect::<Vec<_>>(), before);
            assert!(!app.document.can_undo());
            enter(&mut app, "");
        }
        assert!(app.command_line_idle(), "{name}: {:?}", app.command_log);
        if name == "definition_near-conic" {
            assert_eq!(app.document.objects().cloned().collect::<Vec<_>>(), before);
            assert!(!app.document.can_undo());
            continue;
        }
        let mut maximum =
            comparison::check(&app.document, &v["after"], &before, name, &mut mapping);
        let after = app.document.objects().cloned().collect::<Vec<_>>();
        if !v["undo"].is_null() {
            assert_eq!(app.document.undo_label(), Some("MarkFoci"));
            enter(&mut app, "Undo");
            assert_eq!(app.document.objects().cloned().collect::<Vec<_>>(), before);
            maximum = maximum.max(comparison::check(
                &app.document,
                &v["undo"],
                &before,
                name,
                &mut mapping,
            ));
            enter(&mut app, "Redo");
            assert_eq!(app.document.objects().cloned().collect::<Vec<_>>(), after);
            maximum = maximum.max(comparison::check(
                &app.document,
                &v["redo"],
                &before,
                name,
                &mut mapping,
            ));
        } else {
            assert!(!app.document.can_undo());
        }
        assert!(maximum <= 1e-8, "{name}: {maximum}");
    }
}
#[test]
fn mark_foci_remembers_options_after_cancel_and_undo_without_model_changes() {
    let mut app = test_app();
    enter(&mut app, "MarkFoci");
    enter(&mut app, "DrawAxis=No");
    enter(&mut app, "DrawAxis=invalid");
    assert!(app.object_prompt.is_some());
    assert!(!app.document.can_undo());
    enter(&mut app, "Cancel");
    assert!(app.command_line_idle());
    app.document = viboceros_command::open_3dm_with_views(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/tools/rhino_oracle/fixtures/mark_foci/ellipse.3dm"
    ))
    .unwrap()
    .0;
    let id = app.document.objects().next().unwrap().id();
    app.document
        .select_objects_direct([id], SelectionMode::Replace)
        .unwrap();
    app.document.clear_history().unwrap();
    enter(&mut app, "MarkFoci");
    assert!(app.command_line_idle(), "{:?}", app.command_log);
    assert_eq!(app.document.objects().len(), 3);
    enter(&mut app, "Undo");
    assert_eq!(app.document.objects().len(), 1);
    enter(&mut app, "MarkFoci");
    assert_eq!(app.document.objects().len(), 3);
}
