use super::native_curve_comparison;
use super::*;
use viboceros_document::SelectionMode;
#[path = "../../../crates/viboceros-command/tests/support/insert_kink.rs"]
mod comparison;
fn enter(app: &mut VibocerosApp, text: &str) {
    app.command_input = text.into();
    app.run_command();
}
fn point(v: &serde_json::Value) -> Point3 {
    Point3::try_from(serde_json::from_value::<[f64; 3]>(v.clone()).unwrap()).unwrap()
}
#[test]
fn insert_kink_native_point_getters_cancel_subcurves_and_history() {
    let capture: serde_json::Value = serde_json::from_str(include_str!(
        "../../../tools/rhino_oracle/observations/insert_kink.json"
    ))
    .unwrap();
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tools/rhino_oracle/fixtures/insert_kink");
    for row in capture["results"].as_array().unwrap() {
        let name = row["id"].as_str().unwrap();
        let v = &row["value"];
        let r = &v["recipe"];
        let mut app = test_app();
        app.document = viboceros_command::open_3dm_with_views(
            root.join(format!("{name}.3dm")).to_str().unwrap(),
        )
        .unwrap()
        .0;
        let id = v["source_id"]
            .as_str()
            .unwrap()
            .parse::<ObjectId>()
            .unwrap();
        app.document.clear_selection();
        if v["before"][0]["selected"] == true {
            app.document
                .select_object(id, SelectionMode::Replace)
                .unwrap();
        }
        app.document.clear_history().unwrap();
        let before = app.document.objects().cloned().collect::<Vec<_>>();
        let temporary = r["subcurve"].as_array();
        enter(
            &mut app,
            if temporary.is_some() {
                "InsertKink SubCrv"
            } else {
                "InsertKink"
            },
        );
        assert!(app.insert_kink_prompt.is_some(), "{name}: prompt");
        if r["preselect"] == false || temporary.is_some() {
            app.apply_selection_click(SelectionClick {
                object_id: Some(id),
                mode: SelectionMode::Replace,
            });
        }
        if name == "polysurface_rejected" {
            app.apply_selection_click(SelectionClick {
                object_id: Some(id),
                mode: SelectionMode::Replace,
            });
            assert!(app.insert_kink_prompt.as_ref().unwrap().selecting());
            enter(&mut app, "");
            assert!(app.insert_kink_prompt.is_none());
            let maximum = comparison::check(&app.document, &v["after"], id, name);
            assert!(maximum <= 1e-8);
            eprintln!(
                "INSERT_KINK_UI_AUDIT {}",
                serde_json::json!({"id":name,"maximum_error":maximum})
            );
            continue;
        }
        assert!(
            app.insert_kink_prompt.as_ref().unwrap().wants_point(),
            "{name}: source"
        );
        if let Some(range) = temporary {
            let c = app
                .document
                .object(id)
                .unwrap()
                .geometry()
                .curve_ref()
                .unwrap();
            let ends = range
                .iter()
                .map(|t| {
                    c.evaluate(c.parameter_at(t.as_f64().unwrap()).unwrap())
                        .unwrap()
                })
                .collect::<Vec<_>>();
            for p in ends {
                enter(&mut app, &format!("w{}", format_model_point(p)));
            }
        }
        if let Some(direction) = r["direction"].as_str() {
            enter(&mut app, &format!("Direction={direction}"));
        }
        if r["symmetrical"] == true {
            enter(&mut app, "Symmetrical=Yes");
        }
        if r["midpoints"] == true {
            enter(&mut app, "Midpoints=Yes");
        }
        if name == "numeric_invalid" {
            enter(&mut app, "0.35");
            app.cancel_current_prompt_or_selection();
        } else {
            for p in v["picks"].as_array().unwrap() {
                enter(&mut app, &format!("w{}", format_model_point(point(p))));
            }
            assert_eq!(
                app.document.objects().cloned().collect::<Vec<_>>(),
                before,
                "{name}: edits staged"
            );
            assert!(!app.document.can_undo());
            if r["cancel"] == true {
                app.cancel_current_prompt_or_selection();
            } else {
                enter(&mut app, "");
            }
        }
        assert!(app.insert_kink_prompt.is_none(), "{name}: ended");
        let mut maximum = comparison::check(&app.document, &v["after"], id, name);
        if !v["undo"].is_null() {
            assert_eq!(app.document.undo_label(), Some("InsertKink"), "{name}");
            enter(&mut app, "Undo");
            maximum = maximum.max(comparison::check(&app.document, &v["undo"], id, name));
            enter(&mut app, "Redo");
            maximum = maximum.max(comparison::check(&app.document, &v["redo"], id, name));
        }
        assert!(maximum <= 1e-8, "{name}: {maximum}");
        eprintln!(
            "INSERT_KINK_UI_AUDIT {}",
            serde_json::json!({"id":name,"maximum_error":maximum})
        );
    }
}
#[test]
fn insert_kink_source_changes_options_cancel_and_redo() {
    let mut app = test_app();
    app.execute_command("Line 0,0,0 6,0,0");
    let id = app.document.objects().next().unwrap().id();
    app.document
        .select_object(id, SelectionMode::Replace)
        .unwrap();
    app.document.clear_history().unwrap();
    app.execute_command("SetObjectName Future");
    enter(&mut app, "Undo");
    enter(&mut app, "InsertKink");
    enter(&mut app, "2,0,0");
    enter(&mut app, "Direction=Both");
    enter(&mut app, "Symmetrical=Yes");
    app.cancel_current_prompt_or_selection();
    assert!(app.document.can_redo());
    assert!(!app.document.can_undo());
    assert!(app.commands.insert_kink_options().symmetrical);
    enter(&mut app, "InsertKink");
    enter(&mut app, "2,0,0");
    app.document
        .replace_object_geometries([(id, Geometry::Point(Point3::try_new(0., 0., 0.).unwrap()))])
        .unwrap();
    app.validate_insert_kink();
    assert!(app.insert_kink_prompt.is_none());
}

#[test]
fn insert_kink_viewport_points_and_shared_relative_filtered_input() {
    let mut app = test_app();
    app.execute_command("Line 0,0,0 6,0,0");
    let id = app.document.objects().next().unwrap().id();
    app.document.clear_history().unwrap();
    enter(&mut app, "InsertKink");
    let output = ViewportOutput {
        parametric_click: Some(vec![crate::viewport::ParametricPick {
            object: id,
            member: None,
            face: None,
        }]),
        ..Default::default()
    };
    assert!(app.handle_viewport_action(output));
    assert!(app.handle_viewport_action(ViewportOutput {
        edge_parameter: Some(2.),
        ..Default::default()
    }));
    enter(&mut app, "End");
    assert_eq!(app.one_shot_snap_label(), Some("End"));
    enter(&mut app, "r1,0,0");
    assert!(app.one_shot_snap_label().is_none());
    enter(&mut app, ".x");
    enter(&mut app, "4,20,0");
    enter(&mut app, "0,0,0");
    assert!(app.handle_viewport_action(ViewportOutput {
        enter_pressed: true,
        ..Default::default()
    }));
    let Geometry::PolyCurve(c) = app.document.object(id).unwrap().geometry() else {
        panic!("polycurve");
    };
    assert_eq!(c.parameters(), &[0., 2., 3., 4., 6.]);
    assert_eq!(app.document.undo_label(), Some("InsertKink"));
}
