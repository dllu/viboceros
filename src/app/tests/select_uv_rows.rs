use super::*;
use serde_json::Value;
use viboceros_document::ControlPointId;
use viboceros_oracle::select_uv_rows::snapshot;

fn enter(app: &mut VibocerosApp, input: &str) {
    app.command_input = input.into();
    app.run_command();
}
fn check(doc: &Document, id: ObjectId, peer: ObjectId, expected: &Value, name: &str) {
    let actual = snapshot(doc, id, peer);
    let mut expected = expected.clone();
    // Native locations and imported rational controls can differ by an ulp.
    let a = actual["grips"].as_array().unwrap();
    let b = expected["grips"].as_array_mut().unwrap();
    assert_eq!(a.len(), b.len(), "{name}");
    for (a, b) in a.iter().zip(b) {
        for (x, y) in a["point"]
            .as_array()
            .unwrap()
            .iter()
            .zip(b["point"].as_array().unwrap())
        {
            assert!(
                (x.as_f64().unwrap() - y.as_f64().unwrap()).abs() <= 1e-8,
                "{name}"
            );
        }
        b["point"] = a["point"].clone();
    }
    assert_eq!(actual, expected, "{name}");
}
fn run(
    app: &mut VibocerosApp,
    id: ObjectId,
    peer: ObjectId,
    actions: &Value,
    ending: (bool, bool),
    expected: &Value,
    name: &str,
) {
    let (cancel_surface, cancel) = ending;
    enter(app, "SelUV");
    assert!(!app.command_line_idle());
    if cancel_surface {
        app.cancel_current_prompt_or_selection();
    } else {
        if app.select_uv_prompt.as_ref().unwrap().selecting() {
            app.apply_selection_click(SelectionClick {
                object_id: Some(id),
                mode: SelectionMode::Replace,
            });
        }
        for action in actions.as_array().unwrap() {
            if let Some(word) = action.as_str() {
                enter(app, word);
            } else {
                let point = if let Some(i) = action["grip"].as_u64() {
                    app.document.control_point_locations(id).unwrap()[i as usize]
                } else if action.get("parameter").is_some() {
                    let uv =
                        serde_json::from_value::<[f64; 2]>(action["parameter"].clone()).unwrap();
                    viboceros_command::surface_reference::surface(
                        app.document.object(id).unwrap().geometry(),
                        None,
                    )
                    .unwrap()
                    .0
                    .evaluate(uv[0], uv[1])
                    .unwrap()
                } else {
                    Point3::try_from(
                        serde_json::from_value::<[f64; 3]>(action["world"].clone()).unwrap(),
                    )
                    .unwrap()
                };
                enter(app, &format!("w{},{},{}", point.x(), point.y(), point.z()));
            }
            assert!(
                app.select_uv_prompt.is_some(),
                "{name}: {:?}",
                app.command_log
            );
        }
        if cancel {
            app.cancel_current_prompt_or_selection();
        } else {
            enter(app, "");
        }
    }
    assert!(app.command_line_idle(), "{name}: {:?}", app.command_log);
    check(&app.document, id, peer, expected, name);
}

#[test]
fn all_native_seluv_application_states_and_option_memory() {
    let folder = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tools/rhino_oracle");
    let r: Value = serde_json::from_str(
        &std::fs::read_to_string(folder.join("observations/select_uv_rows.json")).unwrap(),
    )
    .unwrap();
    assert_eq!(r["results"].as_array().unwrap().len(), 125);
    for row in r["results"].as_array().unwrap() {
        let name = row["id"].as_str().unwrap();
        let v = &row["value"];
        let recipe = &v["recipe"];
        let mut app = test_app();
        let (doc, _, _, views) = viboceros_command::open_3dm_with_views(
            folder
                .join(format!("fixtures/select_uv_rows/{name}.3dm"))
                .to_str()
                .unwrap(),
        )
        .unwrap();
        app.document = doc;
        app.restore_file_viewports(views);
        let id = v["source_id"].as_str().unwrap().parse().unwrap();
        let peer = v["peer_id"].as_str().unwrap().parse().unwrap();
        app.document.clear_selection();
        app.document.clear_history().unwrap();
        if recipe["grips_on"] == true {
            app.document.enable_control_points([id]).unwrap();
            app.document
                .select_control_points(
                    recipe["selected"]
                        .as_array()
                        .into_iter()
                        .flatten()
                        .map(|i| ControlPointId {
                            object: id,
                            index: i.as_u64().unwrap() as usize,
                        }),
                    SelectionMode::Add,
                )
                .unwrap();
        }
        let marker = app
            .document
            .add_geometry(Geometry::Point(point(30., 30., 30.)))
            .unwrap();
        app.document.undo().unwrap();
        if recipe["preselected"] == true {
            app.document
                .select_objects_direct([id], SelectionMode::Add)
                .unwrap();
        }
        app.document
            .select_objects_direct([peer], SelectionMode::Add)
            .unwrap();
        let original = app.document.object(id).unwrap().clone();
        check(&app.document, id, peer, &v["before"], name);
        run(
            &mut app,
            id,
            peer,
            &recipe["actions"],
            (recipe["cancel_surface"] == true, recipe["cancel"] == true),
            &v["after_script"],
            name,
        );
        if !recipe["next_actions"].is_null() {
            run(
                &mut app,
                id,
                peer,
                &recipe["next_actions"],
                (false, false),
                &v["again"]["after_script"],
                name,
            );
        }
        assert_eq!(
            app.document.object(id),
            Some(&original),
            "{name}: model unchanged"
        );
        assert!(app.document.can_redo(), "{name}");
        app.document.redo().unwrap();
        assert!(app.document.object(marker).is_some());
    }
}

#[test]
fn seluv_mouse_grips_stale_sources_invalid_options_and_new_commands() {
    let folder = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tools/rhino_oracle/fixtures/select_uv_rows/surface_U.3dm");
    let (doc, _, _, views) =
        viboceros_command::open_3dm_with_views(folder.to_str().unwrap()).unwrap();
    let mut app = test_app();
    app.document = doc;
    app.restore_file_viewports(views);
    let id = app
        .document
        .objects()
        .find(|o| viboceros_command::select_uv_rows::accepts(o.geometry()))
        .unwrap()
        .id();
    enter(&mut app, "SelUV");
    app.apply_selection_click(SelectionClick {
        object_id: Some(id),
        mode: SelectionMode::Replace,
    });
    let output = ViewportOutput {
        control_point_selection: Some(crate::viewport::ControlPointSelection {
            picks: vec![ControlPointId {
                object: id,
                index: 4,
            }],
            mode: SelectionMode::Replace,
        }),
        ..Default::default()
    };
    assert!(app.handle_viewport_action(output));
    assert_eq!(
        app.document
            .selected_control_points()
            .map(|(g, _)| g.index)
            .collect::<Vec<_>>(),
        [1, 4, 7]
    );
    enter(&mut app, "Direction=Both");
    enter(&mut app, "Toggle");
    assert_eq!(
        app.commands.select_uv_options().direction,
        viboceros_geometry::SurfaceKnotDirection::Both
    );
    enter(&mut app, "HideOthers=Maybe");
    assert!(!app.commands.select_uv_options().hide_others);
    assert!(app.select_uv_prompt.is_some());
    enter(&mut app, "SelNone");
    assert!(app.command_line_idle());
    enter(&mut app, "SelUV");
    app.apply_selection_click(SelectionClick {
        object_id: Some(id),
        mode: SelectionMode::Replace,
    });
    app.document.set_objects_visibility([id], false).unwrap();
    app.validate_select_uv();
    assert!(app.command_line_idle());
}
