use super::grip_transform::{compare, geometry, snapshot};
use super::*;
use serde_json::Value;
use viboceros_document::{ControlPointId, SelectionMode};
use viboceros_geometry::Vector3;

fn enter(app: &mut VibocerosApp, text: &str) {
    app.command_input = text.into();
    app.run_command();
}

fn coords(value: &Value) -> [f64; 3] {
    serde_json::from_value(value.clone()).unwrap()
}

#[test]
fn scale_nu_options_rigid_groups_grip_exclusion_and_cancel_memory_match_native() {
    replay_scale_nu_options(
        include_str!("../../../tools/rhino_oracle/fixtures/scale_nu_options.json"),
        include_str!("../../../tools/rhino_oracle/observations/scale_nu_options.json"),
        36,
        "ScaleNU",
    );
}

#[test]
fn scale_nu_temporary_grip_positions_are_discarded_by_move_and_history_like_native() {
    replay_scale_nu_options(
        include_str!("../../../tools/rhino_oracle/fixtures/scale_nu_pending_grips.json"),
        include_str!("../../../tools/rhino_oracle/observations/scale_nu_pending_grips.json"),
        3,
        "ScaleNU",
    );
}

pub(super) fn replay_scale_nu_options(fixture: &str, observed: &str, count: usize, command: &str) {
    use serde_json::json;
    let fixture: Value = serde_json::from_str(fixture).unwrap();
    let observed: Value = serde_json::from_str(observed).unwrap();
    let operations = fixture["operations"].as_array().unwrap();
    let rows = observed["results"].as_array().unwrap();
    assert_eq!(operations.len(), count);
    assert_eq!(rows.len(), operations.len());
    let mut replays = 0;
    for (op, row) in operations.iter().zip(rows) {
        assert_eq!(op["id"], row["id"]);
        let native = &row["value"];
        for incremental in [false, true] {
            if command == "ScalePositions"
                && !incremental
                && (native["macro"].as_str().unwrap().contains("_Cancel")
                    || matches!(
                        op["input"].as_str(),
                        Some("repeat_direction" | "repeat_factor" | "remember_scalar_cancel")
                    ))
            {
                continue;
            }
            let mut app = test_app();
            app.active_viewport = 1;
            app.viewports[1].set_construction_plane(
                Frame3::try_from_directions(
                    Point3::try_from(coords(&native["plane"]["origin"])).unwrap(),
                    Vector3::try_from(coords(&native["plane"]["x_axis"])).unwrap(),
                    Vector3::try_from(coords(&native["plane"]["y_axis"])).unwrap(),
                    Tolerance::DEFAULT,
                )
                .unwrap(),
            );
            enter(&mut app, "Point 1,1,1");
            enter(&mut app, "SelAll");
            for script in native["seed_macros"].as_array().unwrap() {
                for token in script.as_str().unwrap().split_whitespace() {
                    enter(&mut app, token);
                }
            }
            enter(&mut app, "Delete");
            app.document.clear_history().unwrap();
            app.document
                .begin_transaction(format!("{command} option sources"))
                .unwrap();
            let mut sources = Vec::new();
            for (i, source) in native["before"]["objects"]
                .as_array()
                .unwrap()
                .iter()
                .enumerate()
            {
                let kind = native["kinds"][i].as_str().unwrap();
                let geom = if kind == "point" {
                    Geometry::Point(point(8., 0., 0.))
                } else {
                    geometry(&json!({"source":kind}), source)
                };
                let id = app.document.add_geometry(geom).unwrap();
                sources.push(id);
                app.document
                    .set_object_names([(id, Some(format!("options source {i}")))])
                    .unwrap();
                if command == "ScalePositions" {
                    for (key, value) in source["attribute_user_text"].as_object().unwrap() {
                        app.document
                            .set_object_user_text([id], key, Some(value.as_str().unwrap()))
                            .unwrap();
                    }
                    for (key, value) in source["geometry_user_text"].as_object().unwrap() {
                        app.document
                            .set_object_geometry_user_text([id], key, Some(value.as_str().unwrap()))
                            .unwrap();
                    }
                }
            }
            for members in native["before"]["groups"].as_array().unwrap() {
                app.document
                    .add_group(
                        None,
                        members
                            .as_array()
                            .unwrap()
                            .iter()
                            .map(|i| sources[i.as_u64().unwrap() as usize]),
                    )
                    .unwrap();
            }
            app.document.commit_transaction().unwrap();
            if op["selection"] != "objects" {
                app.document.enable_control_points([sources[0]]).unwrap();
                app.document
                    .select_control_points(
                        [0, 2].map(|index| ControlPointId {
                            object: sources[0],
                            index,
                        }),
                        SelectionMode::Add,
                    )
                    .unwrap();
                if op["selection"] == "parent" {
                    app.document
                        .select_objects_direct([sources[0]], SelectionMode::Add)
                        .unwrap();
                }
                app.document
                    .select_objects_direct(sources[1..].iter().copied(), SelectionMode::Add)
                    .unwrap();
            } else {
                app.document
                    .select_objects_direct(sources.iter().copied(), SelectionMode::Replace)
                    .unwrap();
            }
            let state = |app: &VibocerosApp| {
                let mut objects = snapshot(app, sources[0], None);
                for (value, object) in objects
                    .as_array_mut()
                    .unwrap()
                    .iter_mut()
                    .zip(app.document.objects())
                {
                    value["role"] = json!(if sources.contains(&object.id()) {
                        "source"
                    } else {
                        "output"
                    });
                    if command == "ScalePositions" {
                        value["attribute_user_text"] = json!(object.attributes().user_text());
                        value["geometry_user_text"] = json!(object.geometry_user_text());
                    }
                }
                let order = app.document.objects().map(|o| o.id()).collect::<Vec<_>>();
                let groups = app
                    .document
                    .groups()
                    .map(|g| {
                        order
                            .iter()
                            .enumerate()
                            .filter_map(|(i, id)| {
                                g.members().any(|member| member == *id).then_some(i)
                            })
                            .collect::<Vec<_>>()
                    })
                    .collect::<Vec<_>>();
                json!({"objects":objects,"groups":groups})
            };
            let label = format!(
                "{} incremental={incremental}: {:?}",
                op["id"], app.command_log
            );
            compare(&state(&app), &native["before"], &label);
            let tokens = native["macro"]
                .as_str()
                .unwrap()
                .split_whitespace()
                .collect::<Vec<_>>();
            if incremental
                || op["mode"] == "rigid_repeat"
                || op["mode"] == "remember_factors_cancel"
            {
                for (i, token) in tokens.iter().enumerate() {
                    if command == "ScalePositions"
                        && *token == "_Cancel"
                        && i + 1 == tokens.len()
                        && app.active_command.is_none()
                        && app.object_prompt.is_none()
                    {
                        compare(&state(&app), &native["after"], &label);
                    }
                    enter(&mut app, token);
                }
            } else {
                let args = tokens[1..]
                    .iter()
                    .filter(|t| **t != "_Enter")
                    .map(|t| t.strip_prefix('w').unwrap_or(t))
                    .collect::<Vec<_>>();
                enter(&mut app, &format!("{command} {}", args.join(" ")));
            }
            assert!(
                app.active_command.is_none(),
                "{label}: {:?}",
                app.command_log
            );
            compare(&state(&app), &native["after_script"], &label);
            if let Some(followup) = native.get("followup") {
                assert_eq!(followup["macro"], "_Move w0,0,0 w1,2,3");
                if incremental {
                    for token in followup["macro"].as_str().unwrap().split_whitespace() {
                        enter(&mut app, token);
                    }
                } else {
                    enter(&mut app, "Move 0,0,0 1,2,3");
                }
                compare(&state(&app), &followup["after"], &format!("Move {label}"));
            }
            enter(&mut app, "Undo");
            compare(&state(&app), &native["undo"], &format!("Undo {label}"));
            enter(&mut app, "Redo");
            compare(&state(&app), &native["redo"], &format!("Redo {label}"));
            replays += 1;
        }
    }
    if command == "ScalePositions" {
        assert_eq!(replays, 99);
    }
}

#[test]
fn scale_nu_cursor_and_keyboard_reference_inputs_match_20_native_captures() {
    use crate::viewport::{DraftingInput, ViewportInput};
    use serde_json::json;
    let fixture: Value = serde_json::from_str(include_str!(
        "../../../tools/rhino_oracle/fixtures/scale_nu_reference.json"
    ))
    .unwrap();
    let observed: Value = serde_json::from_str(include_str!(
        "../../../tools/rhino_oracle/observations/scale_nu_reference.json"
    ))
    .unwrap();
    let operations = fixture["operations"].as_array().unwrap();
    let results = observed["results"].as_array().unwrap();
    assert_eq!(operations.len(), 32);
    assert_eq!(results.len(), operations.len());
    let mut matched = 0;
    for (op, row) in operations.iter().zip(results) {
        let label = op["id"].as_str().unwrap();
        let native = &row["value"];
        assert_eq!(row["id"], label);
        // Scripted native coordinates depend on preceding cursor input. Keep
        // those twelve raw diagnostics separate from these real UI replays.
        if op["finish"] == "Scripted" {
            continue;
        }
        matched += 1;
        let axis = op["axis"].as_u64().unwrap() as usize;
        let mut app = test_app();
        app.active_viewport = 1;
        app.viewports[1] = crate::viewport::clip_tests::captured_view(&native["pending"]["camera"]);
        let source = app
            .document
            .add_geometry(Geometry::Point(point(2., 3., 4.)))
            .unwrap();
        app.document
            .select_objects_direct([source], SelectionMode::Replace)
            .unwrap();
        app.document.clear_history().unwrap();
        let objects = |app: &VibocerosApp| {
            json!(app.document.objects().map(|object| {
            let Geometry::Point(p) = object.geometry() else { panic!("point source") };
            json!({"point": p.to_array(), "selected": app.document.is_selected(object.id())})
        }).collect::<Vec<_>>())
        };
        compare(&objects(&app), &native["before"], label);
        enter(&mut app, "ScaleNU Copy=No");
        enter(&mut app, "w0,0,0");
        for _ in 0..axis {
            enter(&mut app, "1");
        }
        let mut reference = [0.; 3];
        reference[axis] = 2.;
        if op["reference"] == "OffAxis" {
            reference = [1.; 3];
        }
        enter(
            &mut app,
            &format!("w{},{},{}", reference[0], reference[1], reference[2]),
        );
        let before = app.document.objects().cloned().collect::<Vec<_>>();
        let defaults = app.commands.axis_scale_defaults("ScaleNU");
        let (destination, map) = crate::viewport::affine_preview::tests::captured_destination(
            &app.document,
            &native["pending"],
            ViewportInput {
                drafting: DraftingInput {
                    active: true,
                    osnap: viboceros_drafting::ObjectSnapModes::NONE,
                    ..Default::default()
                },
                affine_preview: Some(app.affine_preview().unwrap()),
                ..Default::default()
            },
        );
        app.update_affine_preview(Some(map));
        assert_eq!(
            app.document.objects().cloned().collect::<Vec<_>>(),
            before,
            "{label}"
        );
        assert_eq!(
            app.commands.axis_scale_defaults("ScaleNU"),
            defaults,
            "{label}"
        );
        compare(&objects(&app), &native["pending"]["objects"], label);
        if op["finish"] == "Click" {
            assert!(
                app.accept_filtered_drafting_point(destination, false),
                "{label}: {:?}",
                app.command_log
            );
        } else {
            let mut target = [0.; 3];
            target[axis] = 6.;
            enter(
                &mut app,
                &format!("w{},{},{}", target[0], target[1], target[2]),
            );
        }
        for _ in axis + 1..3 {
            enter(&mut app, "1");
        }
        assert!(
            app.active_command.is_none(),
            "{label}: {:?}",
            app.command_log
        );
        compare(&objects(&app), &native["after"], label);
        if op["finish"] == "Click" {
            compare(
                &json!(map.transform_point(point(2., 3., 4.)).unwrap().to_array()),
                &native["after"][0]["point"],
                label,
            );
        }
        assert_eq!(app.document.undo_label(), Some("ScaleNU"));
        enter(&mut app, "Undo");
        compare(&objects(&app), &native["undo"], label);
        enter(&mut app, "Redo");
        compare(&objects(&app), &native["redo"], label);
    }
    assert_eq!(matched, 20);
}

#[test]
fn scale_nu_native_replays_numeric_reference_distance_grips_copy_and_history() {
    let q: Value = serde_json::from_str(include_str!(
        "../../../tools/rhino_oracle/fixtures/scale_nu.json"
    ))
    .unwrap();
    let r: Value = serde_json::from_str(include_str!(
        "../../../tools/rhino_oracle/observations/scale_nu.json"
    ))
    .unwrap();
    assert_eq!(q["operations"].as_array().unwrap().len(), 26);
    assert_eq!(r["results"].as_array().unwrap().len(), 26);
    let diagnostic = [5, 10, 11, 12, 20, 22];
    let mut compatible = 0;
    for (i, (op, row)) in q["operations"]
        .as_array()
        .unwrap()
        .iter()
        .zip(r["results"].as_array().unwrap())
        .enumerate()
    {
        for incremental in [false, true] {
            let mut app = test_app();
            let native = &row["value"];
            assert_eq!(row["id"], op["id"]);
            app.active_viewport = 1;
            app.viewports[1].set_construction_plane(
                Frame3::try_from_directions(
                    Point3::try_from(coords(&native["plane"]["origin"])).unwrap(),
                    Vector3::try_from(coords(&native["plane"]["x_axis"])).unwrap(),
                    Vector3::try_from(coords(&native["plane"]["y_axis"])).unwrap(),
                    Tolerance::DEFAULT,
                )
                .unwrap(),
            );
            for axis in 0..3 {
                assert!(app.commands.remember_axis_scale(
                    "ScaleNU",
                    axis,
                    native["seed"][axis].as_f64().unwrap()
                ));
            }
            app.document.begin_transaction("ScaleNU sources").unwrap();
            let is_point = op["source"] == "point";
            let source = app
                .document
                .add_geometry(if is_point {
                    Geometry::Point(point(2., 3., 4.))
                } else {
                    geometry(op, &native["before"][0])
                })
                .unwrap();
            app.document
                .set_object_names([(source, Some("grip source".into()))])
                .unwrap();
            let peer = (op["point"] == true).then(|| {
                app.document
                    .add_geometry(Geometry::Point(point(8., 0., 0.)))
                    .unwrap()
            });
            app.document.commit_transaction().unwrap();
            let all = op["selection"] == "all";
            if is_point || op["selection"] == "object" {
                if !all {
                    app.document
                        .select_objects_direct([source], SelectionMode::Add)
                        .unwrap();
                }
            } else {
                app.document.enable_control_points([source]).unwrap();
                if !all {
                    app.document
                        .select_control_points(
                            [0, 2].map(|index| ControlPointId {
                                object: source,
                                index,
                            }),
                            SelectionMode::Add,
                        )
                        .unwrap();
                }
            }
            if op["parent"] == true {
                app.document
                    .select_objects_direct([source], SelectionMode::Add)
                    .unwrap();
            }
            if let Some(peer) = peer {
                app.document
                    .select_objects_direct([peer], SelectionMode::Add)
                    .unwrap();
            }
            compare(
                &snapshot(&app, source, peer),
                &native["before"],
                "ScaleNU baseline",
            );
            let tokens = native["macro"]
                .as_str()
                .unwrap()
                .split_whitespace()
                .collect::<Vec<_>>();
            let copy = op["copy"].as_bool().unwrap();
            if incremental || all || op["input"] == "repeat" {
                enter(&mut app, "ScaleNU");
                for token in &tokens[1..] {
                    enter(&mut app, token);
                }
            } else {
                let arguments = tokens[1..]
                    .iter()
                    .filter(|token| **token != "_Enter")
                    .map(|token| token.strip_prefix('w').unwrap_or(token))
                    .collect::<Vec<_>>();
                let mut script = format!("ScaleNU {}", arguments.join(" "));
                if op["input"] == "defaults" {
                    script.push_str(" 2 3 .5");
                }
                enter(&mut app, &script);
            }
            assert!(
                app.active_command.is_none(),
                "{}: {:?}",
                op["id"],
                app.command_log
            );
            let actual = snapshot(&app, source, peer);
            let context = format!(
                "{} incremental={incremental} copy={copy}: {:?}",
                op["id"], app.command_log
            );
            if diagnostic.contains(&i) {
                // Preserve the native script discrepancy, with an independent
                // axis-distance witness for our coordinate input behavior.
                let expected = match i {
                    11 => [2., 9., 4.],
                    12 => [2., 3., 12.],
                    _ => [6., 3., 4.],
                };
                compare(&actual[0]["point"], &serde_json::json!(expected), &context);
                assert!(
                    coords(&native["after_script"][0]["point"])
                        .into_iter()
                        .zip(expected)
                        .any(|(a, b)| (a - b).abs() > 1e-6)
                );
            } else {
                compare(&actual, &native["after_script"], &context);
                compatible += 1;
            }
            enter(&mut app, "Undo");
            compare(
                &snapshot(&app, source, peer),
                &native["undo"],
                &format!("Undo {context}"),
            );
            enter(&mut app, "Redo");
            if !diagnostic.contains(&i) {
                compare(
                    &snapshot(&app, source, peer),
                    &native["redo"],
                    &format!("Redo {context}"),
                );
            }
        }
    }
    assert_eq!(compatible, 40);
}

#[test]
fn scale_nu_partial_input_preview_defaults_retry_and_cancel() {
    let mut app = test_app();
    enter(&mut app, "Point 2,3,4");
    enter(&mut app, "SelAll");
    let source = app.document.selected_object_ids().next().unwrap();
    for input in ["ScaleNU", "w0,0,0", "2", "NaN"] {
        enter(&mut app, input);
    }
    assert_eq!(
        app.commands.axis_scale_defaults("ScaleNU"),
        Some([1., 1., 1.])
    );
    assert_eq!(
        app.document.object(source).unwrap().geometry(),
        &Geometry::Point(point(2., 3., 4.))
    );
    let preview = app.affine_preview().unwrap();
    assert_eq!(
        preview
            .last_transform
            .unwrap()
            .transform_point(point(2., 3., 4.))
            .unwrap(),
        point(4., 3., 4.)
    );
    let map = preview
        .definition
        .transform_at(
            preview.frame.unwrap(),
            point(0., 6., 0.),
            Tolerance::DEFAULT,
        )
        .unwrap();
    assert_eq!(
        map.transform_point(point(2., 3., 4.)).unwrap(),
        point(4., 3., 4.)
    );
    enter(&mut app, "Cancel");
    assert!(app.active_command.is_none());
    assert_eq!(
        app.commands.axis_scale_defaults("ScaleNU"),
        Some([1., 1., 1.])
    );
    for input in [
        "ScaleNU Copy=Yes WorldCoordinates",
        "w0,0,0",
        "",
        "0",
        "1",
        "4",
        "",
        "",
    ] {
        enter(&mut app, input);
    }
    // Enter at Y and Z accepts defaults during the second copy; Enter at the
    // next X prompt finishes the repeated session.
    assert!(app.active_command.is_some());
    enter(&mut app, "");
    assert!(app.active_command.is_none());
    assert_eq!(app.document.objects().count(), 3);
    enter(&mut app, "Undo");
    assert_eq!(app.document.objects().count(), 1);
}
