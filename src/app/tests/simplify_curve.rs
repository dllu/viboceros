use super::native_curve_comparison as comparison;
use super::*;
use crate::viewport::SelectionClick;
use viboceros_document::SelectionMode;
fn enter(app: &mut VibocerosApp, text: &str) {
    app.command_input = text.into();
    app.run_command();
}
#[test]
fn simplify_curve_preselection_and_postselection_replay_native_results_and_history() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    let capture: serde_json::Value = serde_json::from_str(include_str!(
        "../../../tools/rhino_oracle/observations/simplify_curves.json"
    ))
    .unwrap();
    let mut count = 0;
    for row in capture["results"].as_array().unwrap() {
        let name = row["id"].as_str().unwrap();
        let v = &row["value"];
        let mut app = test_app();
        app.document = viboceros_command::open_3dm_with_views(
            root.join(format!(
                "tools/rhino_oracle/fixtures/simplify_curves/{name}.3dm"
            ))
            .to_str()
            .unwrap(),
        )
        .unwrap()
        .0;
        app.document.clear_selection();
        let selected = v["before"]
            .as_array()
            .unwrap()
            .iter()
            .filter(|o| o["selected"] == true)
            .map(|o| o["id"].as_str().unwrap().parse::<ObjectId>().unwrap())
            .collect::<Vec<_>>();
        app.document
            .select_objects_direct(selected, SelectionMode::Replace)
            .unwrap();
        app.document.clear_history().unwrap();
        let before = app.document.objects().cloned().collect::<Vec<_>>();
        enter(&mut app, "SimplifyCrv");
        if v["preselect"] == false {
            assert!(app.object_prompt.is_some(), "{name}");
            assert_eq!(
                app.viewport_object_filter(),
                Some(viboceros_command::ObjectSelectionFilter::Curves)
            );
            for id in v["source_ids"].as_array().unwrap() {
                app.apply_selection_click(SelectionClick {
                    object_id: Some(id.as_str().unwrap().parse().unwrap()),
                    mode: SelectionMode::Add,
                });
            }
            assert_eq!(
                app.document.objects().cloned().collect::<Vec<_>>(),
                before,
                "{name}: staged picks"
            );
            assert!(!app.document.can_undo());
            enter(&mut app, "");
        }
        assert!(app.object_prompt.is_none(), "{name}: {:?}", app.command_log);
        assert!(app.command_line_idle(), "{name}");
        assert_eq!(app.document.objects().len(), before.len());
        for expected in v["after"].as_array().unwrap() {
            let id = expected["id"]
                .as_str()
                .unwrap()
                .parse::<ObjectId>()
                .unwrap();
            let actual = app.document.object(id).unwrap();
            let original = before.iter().find(|o| o.id() == id).unwrap();
            assert_eq!(
                actual.attributes(),
                original.attributes(),
                "{name}: attributes"
            );
            assert_eq!(actual.group_ids(), original.group_ids(), "{name}: groups");
            assert_eq!(
                app.document.is_selected(id),
                expected["selected"].as_bool().unwrap(),
                "{name}: selection"
            );
            assert_eq!(
                serde_json::to_value(actual.geometry_user_text()).unwrap(),
                expected["geometry_text"],
                "{name}: geometry user text"
            );
            if let Some(curve) = actual.geometry().curve_ref() {
                assert!(
                    comparison::compare(curve, &expected["geometry"], name) <= 1e-8,
                    "{name}: geometry"
                );
            } else {
                assert_eq!(actual, original);
            }
        }
        let after = app.document.objects().cloned().collect::<Vec<_>>();
        if before != after {
            enter(&mut app, "Undo");
            assert_eq!(app.document.objects().cloned().collect::<Vec<_>>(), before);
            comparison::compare_selection(&app.document, &v["undo"], name);
            assert!(!app.document.can_undo());
            enter(&mut app, "Redo");
            assert_eq!(app.document.objects().cloned().collect::<Vec<_>>(), after);
            comparison::compare_selection(&app.document, &v["redo"], name);
        } else {
            assert!(!app.document.can_undo());
        }
        count += 1;
    }
    assert_eq!(count, 41);
}
#[test]
fn simplify_curve_filtering_cancellation_invalid_arguments_and_noops_preserve_redo() {
    let mut app = test_app();
    app.execute_command("Line 0,0,0 6,0,0");
    let id = app.document.objects().next().unwrap().id();
    app.document
        .select_object(id, SelectionMode::Replace)
        .unwrap();
    app.document.clear_history().unwrap();
    app.execute_command("SetObjectName Future");
    app.execute_command("Undo");
    let before = app.document.objects().cloned().collect::<Vec<_>>();
    assert!(app.document.can_redo());
    enter(&mut app, "SimplifyCrv");
    assert_eq!(app.document.objects().cloned().collect::<Vec<_>>(), before);
    assert!(app.document.can_redo());
    enter(&mut app, "SimplifyCrv Unsupported=Yes");
    assert_eq!(app.document.objects().cloned().collect::<Vec<_>>(), before);
    assert!(app.document.can_redo());
    app.document.clear_selection();
    enter(&mut app, "SimplifyCrv");
    assert!(app.object_prompt.is_some());
    app.apply_selection_click(SelectionClick {
        object_id: Some(id),
        mode: SelectionMode::Add,
    });
    app.cancel_current_prompt_or_selection();
    assert!(app.object_prompt.is_none());
    assert_eq!(app.document.objects().cloned().collect::<Vec<_>>(), before);
    assert!(app.document.can_redo());
    assert!(!app.document.can_undo());
}
