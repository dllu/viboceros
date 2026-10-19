use super::native_curve_comparison;
use super::*;
#[path = "../../../crates/viboceros-command/tests/support/modify_radius.rs"]
mod comparison;
fn enter(app: &mut VibocerosApp, text: &str) {
    app.command_input = text.into();
    app.run_command();
}
#[test]
fn modify_radius_native_modes_copy_metadata_and_selection_history_through_ui() {
    let capture: serde_json::Value = serde_json::from_str(include_str!(
        "../../../tools/rhino_oracle/observations/modify_radius.json"
    ))
    .unwrap();
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tools/rhino_oracle/fixtures/modify_radius");
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
        let ids = v["before"]
            .as_array()
            .unwrap()
            .iter()
            .filter(|o| o["selected"] == true)
            .map(|o| o["id"].as_str().unwrap().parse::<ObjectId>().unwrap())
            .collect::<Vec<_>>();
        app.document
            .select_objects_direct(ids, SelectionMode::Replace)
            .unwrap();
        app.document.clear_history().unwrap();
        let before = app.document.objects().cloned().collect::<Vec<_>>();
        let mut mapping = comparison::initial_mapping(&before);
        enter(&mut app, "ModifyRadius");
        if recipe["preselect"] == false {
            assert_eq!(
                app.viewport_object_filter(),
                Some(viboceros_command::ObjectSelectionFilter::CircularCurves),
                "{name}"
            );
            for id in v["source_ids"].as_array().unwrap() {
                app.apply_selection_click(SelectionClick {
                    object_id: Some(id.as_str().unwrap().parse().unwrap()),
                    mode: SelectionMode::Add,
                });
            }
            enter(&mut app, "");
        }
        assert!(app.object_prompt.is_some(), "{name}: {:?}", app.command_log);
        assert_eq!(
            app.document.objects().cloned().collect::<Vec<_>>(),
            before,
            "{name}: staged"
        );
        assert!(!app.document.can_undo());
        enter(
            &mut app,
            if recipe["copy"] == true {
                "Copy=Yes"
            } else {
                "Copy=No"
            },
        );
        enter(&mut app, recipe["mode"].as_str().unwrap());
        enter(&mut app, &recipe["value"].as_f64().unwrap().to_string());
        assert!(app.command_line_idle(), "{name}: {:?}", app.command_log);
        let mut maximum =
            comparison::check(&app.document, &v["after"], &before, name, &mut mapping);
        let after = app.document.objects().cloned().collect::<Vec<_>>();
        if !v["undo"].is_null() {
            assert_eq!(app.document.undo_label(), Some("ModifyRadius"));
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
        }
        assert!(maximum <= 1e-8, "{name}: {maximum}");
    }
}
#[test]
fn modify_radius_prompts_filter_cancel_bad_sizes_and_full_scripts() {
    let mut app = test_app();
    app.execute_command("Circle 0,0,0 2");
    app.execute_command("Line 0,0,0 5,0,0");
    let circle = app
        .document
        .objects()
        .find(|o| matches!(o.geometry(), Geometry::Circle(_)))
        .unwrap()
        .id();
    let line = app
        .document
        .objects()
        .find(|o| matches!(o.geometry(), Geometry::Line(_)))
        .unwrap()
        .id();
    app.document.clear_history().unwrap();
    app.document
        .select_objects_direct([circle], SelectionMode::Replace)
        .unwrap();
    app.execute_command("SetObjectName Future");
    app.execute_command("Undo");
    let before = app.document.objects().cloned().collect::<Vec<_>>();
    enter(&mut app, "ModifyRadius");
    for bad in ["0", "-2", "NaN", "Area=-1", "2 Copy=Maybe", "2 3"] {
        enter(&mut app, bad);
        assert!(app.object_prompt.is_some());
        assert_eq!(app.document.objects().cloned().collect::<Vec<_>>(), before);
        assert!(app.document.can_redo());
    }
    enter(&mut app, "Copy=Yes");
    app.cancel_current_prompt_or_selection();
    assert!(app.command_line_idle());
    assert!(app.document.can_redo());
    assert_eq!(app.commands.copy_default("ModifyRadius"), Some(false));
    app.document.clear_selection();
    enter(&mut app, "ModifyRadius");
    app.apply_selection_click(SelectionClick {
        object_id: Some(line),
        mode: SelectionMode::Add,
    });
    assert!(!app.document.is_selected(line));
    app.apply_selection_click(SelectionClick {
        object_id: Some(circle),
        mode: SelectionMode::Add,
    });
    enter(&mut app, "");
    enter(&mut app, "Diameter");
    enter(&mut app, "8");
    assert!(app.command_line_idle());
    let Geometry::Circle(c) = app.document.object(circle).unwrap().geometry() else {
        panic!()
    };
    assert_eq!(c.radius(), 4.);
    assert_eq!(app.document.undo_label(), Some("ModifyRadius"));
    app.document
        .select_objects_direct([circle], SelectionMode::Replace)
        .unwrap();
    enter(&mut app, "ModifyRadius Area=12.566370614359172 Copy=Yes");
    assert!(app.command_line_idle());
    assert_eq!(app.document.objects().len(), 3);
    enter(&mut app, "Undo");
    app.document.clear_selection();
    enter(&mut app, "ModifyRadius Diameter=6 Copy=Yes");
    assert!(app.object_prompt.is_some());
    app.apply_selection_click(SelectionClick {
        object_id: Some(circle),
        mode: SelectionMode::Add,
    });
    enter(&mut app, "");
    assert!(app.command_line_idle(), "{:?}", app.command_log);
    assert_eq!(app.document.objects().len(), 3);
    enter(&mut app, "ModifyRadius");
    assert!(app.object_prompt.is_some());
    enter(&mut app, "Point 9,9,9");
    assert!(app.object_prompt.is_none());
    assert!(app.command_line_idle());
    assert_eq!(app.document.objects().len(), 4);
}
