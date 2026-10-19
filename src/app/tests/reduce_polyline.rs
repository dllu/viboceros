use super::native_object_comparison as comparison;
use super::*;
fn enter(app: &mut VibocerosApp, text: &str) {
    app.command_input = text.into();
    app.run_command();
}

#[test]
fn reduce_polyline_native_selection_numeric_input_copy_and_history_through_ui() {
    let capture: serde_json::Value = serde_json::from_str(include_str!(
        "../../../tools/rhino_oracle/observations/reduce_polyline.json"
    ))
    .unwrap();
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tools/rhino_oracle/fixtures/reduce_polyline");
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
            .map(|o| o.as_str().unwrap().parse::<ObjectId>().unwrap())
            .collect::<Vec<_>>();
        if recipe["preselect"] != false {
            app.document
                .select_objects_direct(ids.clone(), SelectionMode::Replace)
                .unwrap();
        }
        app.document.clear_history().unwrap();
        let before = app.document.objects().cloned().collect::<Vec<_>>();
        let mut mapping = comparison::initial_mapping(&before);
        enter(&mut app, "ReducePolyline");
        if recipe["preselect"] == false {
            assert_eq!(
                app.viewport_object_filter(),
                Some(viboceros_command::ObjectSelectionFilter::Polylines)
            );
            for id in ids {
                app.apply_selection_click(SelectionClick {
                    object_id: Some(id),
                    mode: SelectionMode::Add,
                });
            }
            enter(&mut app, "");
        }
        assert!(app.object_prompt.is_some(), "{name}: {:?}", app.command_log);
        assert_eq!(app.document.objects().cloned().collect::<Vec<_>>(), before);
        assert!(!app.document.can_undo());
        enter(
            &mut app,
            if recipe["copy"] == true {
                "Copy=Yes"
            } else {
                "Copy=No"
            },
        );
        enter(
            &mut app,
            &recipe["reduction_tolerance"].as_f64().unwrap().to_string(),
        );
        if v["after"]
            .as_array()
            .unwrap()
            .iter()
            .any(|o| o["geometry"]["type"] == "invalid_polyline")
        {
            assert!(app.object_prompt.is_some());
            assert!(app.command_log.back().unwrap().contains("collapse"));
            assert_eq!(app.document.objects().cloned().collect::<Vec<_>>(), before);
            assert!(!app.document.can_undo());
            enter(&mut app, "Cancel");
            assert!(app.command_line_idle());
            continue;
        }
        assert!(app.command_line_idle(), "{name}: {:?}", app.command_log);
        let mut maximum =
            comparison::check(&app.document, &v["after"], &before, name, &mut mapping);
        let after = app.document.objects().cloned().collect::<Vec<_>>();
        if !v["undo"].is_null() {
            assert_eq!(app.document.undo_label(), Some("ReducePolyline"));
            enter(&mut app, "Undo");
            assert_eq!(app.document.objects().cloned().collect::<Vec<_>>(), before);
            maximum = maximum.max(comparison::check(
                &app.document,
                &v["undo"],
                &before,
                name,
                &mut mapping,
            ));
            assert!(!app.document.can_undo());
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
fn reduce_polyline_defaults_persist_across_undo_and_cancellation_does_not_edit() {
    let mut app = test_app();
    app.document = viboceros_command::open_3dm_with_views(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/tools/rhino_oracle/fixtures/reduce_polyline/wave.3dm"
    ))
    .unwrap()
    .0;
    let id = app.document.objects().next().unwrap().id();
    app.document
        .select_objects_direct([id], SelectionMode::Replace)
        .unwrap();
    app.document.clear_history().unwrap();
    let before = app.document.objects().cloned().collect::<Vec<_>>();
    enter(&mut app, "ReducePolyline");
    enter(&mut app, "Copy=Yes");
    enter(&mut app, "0");
    assert!(app.object_prompt.is_some());
    assert!(!app.document.can_undo());
    enter(&mut app, "Cancel");
    assert!(app.command_line_idle());
    assert_eq!(app.document.objects().cloned().collect::<Vec<_>>(), before);
    assert_eq!(
        app.commands.transform_scalar_default("ReducePolyline"),
        None
    );
    enter(&mut app, "ReducePolyline 0.15 Copy=No");
    assert!(app.command_line_idle());
    let after = app.document.objects().cloned().collect::<Vec<_>>();
    assert_ne!(before, after);
    enter(&mut app, "Undo");
    assert_eq!(app.document.objects().cloned().collect::<Vec<_>>(), before);
    assert_eq!(
        app.commands.transform_scalar_default("ReducePolyline"),
        Some(0.15)
    );
    enter(&mut app, "ReducePolyline");
    enter(&mut app, "");
    assert!(app.command_line_idle());
    assert_eq!(app.document.objects().cloned().collect::<Vec<_>>(), after);
    enter(&mut app, "Undo");
    app.document.clear_selection();
    enter(&mut app, "ReducePolyline 0.15 Copy=Yes");
    assert!(app.object_prompt.is_some());
    app.apply_selection_click(SelectionClick {
        object_id: Some(id),
        mode: SelectionMode::Add,
    });
    enter(&mut app, "");
    assert!(app.command_line_idle());
    assert_eq!(app.document.objects().len(), 2);
}
