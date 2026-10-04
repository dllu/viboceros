use super::*;
use serde_json::Value;
use viboceros_command::maelstrom::{MaelstromOptions, MaelstromRadius};

fn type_value(app: &mut VibocerosApp, value: &Value) {
    app.command_input = if let Some(p) = value.as_array() {
        format!("w{},{},{}", p[0], p[1], p[2])
    } else if value.is_null() {
        String::new()
    } else {
        value.to_string()
    };
    app.run_command();
}

fn assert_points(app: &VibocerosApp, rows: &Value, selection: bool, label: &str) {
    let rows = rows.as_array().unwrap();
    assert_eq!(app.document.objects().count(), rows.len(), "{label}");
    for (object, row) in app.document.objects().zip(rows) {
        let Geometry::Point(point) = object.geometry() else {
            panic!("{label}: expected point")
        };
        let expected =
            Point3::try_from(serde_json::from_value::<[f64; 3]>(row["point"].clone()).unwrap())
                .unwrap();
        assert!(
            point.distance_to(expected).unwrap() <= 1e-11,
            "{label}: {point:?} != {expected:?}"
        );
        if selection {
            assert_eq!(
                app.document.is_selected(object.id()),
                row["selected"].as_bool().unwrap(),
                "{label}"
            );
        }
    }
}

fn assert_preference_snapshot(app: &VibocerosApp, rows: &Value, label: &str) {
    let rows = rows.as_array().unwrap();
    assert_eq!(app.document.objects().count(), rows.len(), "{label}");
    let p = |v: &Value| {
        Point3::try_from(serde_json::from_value::<[f64; 3]>(v.clone()).unwrap()).unwrap()
    };
    let near = |a: Point3, b: Point3, epsilon: f64| {
        assert!(
            a.distance_to(b).unwrap() <= epsilon,
            "{label}: {a:?} != {b:?}"
        )
    };
    for (object, row) in app.document.objects().zip(rows) {
        assert_eq!(
            app.document.is_selected(object.id()),
            row["selected"].as_bool().unwrap(),
            "{label}"
        );
        let geometry = &row["geometry"];
        match object.geometry() {
            Geometry::Point(point) => near(*point, p(&geometry["points"][0]), 1e-11),
            Geometry::Line(line) => {
                for (i, value) in geometry["samples"].as_array().unwrap().iter().enumerate() {
                    near(line.point_at(i as f64 / 64.).unwrap(), p(value), 1e-11);
                }
            }
            Geometry::NurbsCurve(curve) => {
                let domain = curve.domain();
                for (i, value) in geometry["samples"].as_array().unwrap().iter().enumerate() {
                    near(
                        curve
                            .evaluate(
                                domain.start() + (domain.end() - domain.start()) * i as f64 / 64.,
                            )
                            .unwrap(),
                        p(value),
                        2e-5,
                    );
                }
            }
            Geometry::Brep(brep) => {
                assert!(brep.is_solid());
                assert_eq!(
                    brep.faces().len(),
                    geometry["surfaces"].as_array().unwrap().len()
                );
                let bounds = object
                    .geometry()
                    .tight_bounds(app.document.tolerance())
                    .unwrap();
                near(bounds.min(), p(&geometry["bounds"][0]), 2e-5);
                near(bounds.max(), p(&geometry["bounds"][1]), 2e-5);
            }
            _ => panic!("{label}: unexpected preference geometry"),
        }
    }
}

#[test]
fn interactive_preferences_and_terminal_geometry_match_29_native_steps() {
    let captured: Value = serde_json::from_str(include_str!(
        "../../../tools/rhino_oracle/observations/maelstrom_options_command.json"
    ))
    .unwrap();
    let mut app = test_app();
    let mut sources = super::taper::preference_sources(&mut app);
    let yes_no = |v| if v { "Yes" } else { "No" };
    for (i, record) in captured["results"][0]["value"]["records"]
        .as_array()
        .unwrap()
        .iter()
        .enumerate()
    {
        let label = format!("native Maelstrom preference step {i}");
        for before in [true, false] {
            let query = &record[if before {
                "query_before"
            } else {
                "query_after"
            }];
            app.document
                .select_objects_direct(sources["Curve"].iter().copied(), SelectionMode::Replace)
                .unwrap();
            assert!(app.try_start_interactive_command("Maelstrom"), "{label}");
            let Some(InteractiveCommand::Maelstrom { options, .. }) = app.active_command else {
                panic!("{label}: missing prompt")
            };
            assert_eq!(
                options,
                MaelstromOptions {
                    copy: query["defaults"]["Copy"].as_bool().unwrap(),
                    rigid: query["defaults"]["Rigid"].as_bool().unwrap()
                },
                "{label}"
            );
            assert_eq!(
                app.commands.maelstrom_radius_default(),
                query["first_radius"].as_f64().unwrap(),
                "{label}"
            );
            assert!(app.accept_drafting_point(point(0., 0., 0.)));
            assert!(app.try_continue_maelstrom("Enter"));
            assert!(app.try_continue_maelstrom("Cancel"));
            assert_preference_snapshot(&app, &query["after"], &label);
            if !before {
                continue;
            }
            let step = &record["step"];
            match step["kind"].as_str().unwrap() {
                "Maelstrom" => {
                    app.document
                        .select_objects_direct(
                            sources[step["shape"].as_str().unwrap()].iter().copied(),
                            SelectionMode::Replace,
                        )
                        .unwrap();
                    assert!(app.try_start_interactive_command("Maelstrom"));
                    assert!(app.accept_drafting_point(point(0., 0., 0.)));
                    type_value(
                        &mut app,
                        &step.get("radius0").cloned().unwrap_or(Value::from(2.)),
                    );
                    for (name, v) in step["options"].as_object().unwrap() {
                        assert!(app.try_continue_maelstrom(&format!(
                            "{name}={}",
                            yes_no(v.as_bool().unwrap())
                        )));
                    }
                    let finish = step["finish"].as_str().unwrap();
                    if finish != "CancelStart" {
                        assert!(app.try_continue_maelstrom("5"));
                    }
                    if !matches!(finish, "CancelStart" | "CancelEnd") {
                        assert!(app.try_continue_maelstrom("90"));
                    }
                    if finish.starts_with("CopyThen") {
                        for (name, v) in step["pending_options"].as_object().unwrap() {
                            assert!(app.try_continue_maelstrom(&format!(
                                "{name}={}",
                                yes_no(v.as_bool().unwrap())
                            )));
                        }
                    }
                    if app.active_command.is_some() {
                        assert!(app.try_continue_maelstrom(
                            if matches!(finish, "Complete" | "CopyThenEnter") {
                                "Enter"
                            } else {
                                "Cancel"
                            }
                        ));
                    }
                }
                "New" => sources = super::taper::preference_sources(&mut app),
                "RememberCopyOptions" => app.execute_command(&format!(
                    "RememberCopyOptions {}",
                    yes_no(step["enabled"].as_bool().unwrap())
                )),
                name => app.execute_command(name),
            }
            if step["kind"] != "New" {
                assert_preference_snapshot(&app, &record["result"]["after"], &label);
            }
        }
    }
}

#[test]
fn picked_coil_angles_empty_getters_and_copy_batches_match_native_commands() {
    let mut app = test_app();
    for (recipe, captured) in [
        (
            include_str!("../../../tools/rhino_oracle/fixtures/maelstrom_input_command.json"),
            include_str!("../../../tools/rhino_oracle/observations/maelstrom_input_command.json"),
        ),
        (
            include_str!("../../../tools/rhino_oracle/fixtures/maelstrom_small_angle_command.json"),
            include_str!(
                "../../../tools/rhino_oracle/observations/maelstrom_small_angle_command.json"
            ),
        ),
        (
            include_str!(
                "../../../tools/rhino_oracle/fixtures/maelstrom_angle_boundary_command.json"
            ),
            include_str!(
                "../../../tools/rhino_oracle/observations/maelstrom_angle_boundary_command.json"
            ),
        ),
        (
            include_str!(
                "../../../tools/rhino_oracle/fixtures/maelstrom_angle_quadrant_command.json"
            ),
            include_str!(
                "../../../tools/rhino_oracle/observations/maelstrom_angle_quadrant_command.json"
            ),
        ),
    ] {
        let request: Value = serde_json::from_str(recipe).unwrap();
        let captured: Value = serde_json::from_str(captured).unwrap();
        for (op, capture) in request["operations"]
            .as_array()
            .unwrap()
            .iter()
            .zip(captured["results"].as_array().unwrap())
        {
            let label = op["id"].as_str().unwrap();
            let v = &capture["value"];
            app.document = Document::default();
            let ids = v["before"]
                .as_array()
                .unwrap()
                .iter()
                .map(|row| {
                    let p = Point3::try_from(
                        serde_json::from_value::<[f64; 3]>(row["point"].clone()).unwrap(),
                    )
                    .unwrap();
                    let id = app.document.add_geometry(Geometry::Point(p)).unwrap();
                    if row["selected"].as_bool().unwrap() {
                        app.document.select_object(id, SelectionMode::Add).unwrap();
                    }
                    id
                })
                .collect::<Vec<_>>();
            app.document.clear_history().unwrap();
            let center =
                Point3::try_from(serde_json::from_value::<[f64; 3]>(op["center"].clone()).unwrap())
                    .unwrap();
            let normal = viboceros_geometry::Vector3::try_from(
                serde_json::from_value::<[f64; 3]>(op["normal"].clone()).unwrap(),
            )
            .unwrap();
            app.viewports[app.active_viewport].set_construction_plane(
                Frame3::try_from_normal(center, normal, Tolerance::NUMERICAL_VALIDATION).unwrap(),
            );
            assert!(app.try_start_interactive_command("Maelstrom"), "{label}");
            if op["postselect"].as_bool().unwrap() {
                assert!(app.object_prompt.is_some(), "{label}");
                for id in &ids {
                    app.command_input = format!("SelID {id}");
                    app.run_command();
                }
                app.command_input.clear();
                app.run_command();
            }
            type_value(&mut app, &op["center"]);
            type_value(&mut app, &op["radius0"]);
            assert!(
                matches!(
                    app.active_command,
                    Some(InteractiveCommand::Maelstrom {
                        initial: Some(_),
                        ..
                    })
                ),
                "{label}"
            );
            for (name, field) in [("Copy", "copy"), ("Rigid", "rigid")] {
                app.command_input = format!(
                    "{name}={}",
                    if op[field].as_bool().unwrap() {
                        "Yes"
                    } else {
                        "No"
                    }
                );
                app.run_command();
            }
            type_value(&mut app, &op["radius1"]);
            if !op["radius1"].is_null() {
                for angle in op["angles"].as_array().unwrap() {
                    type_value(&mut app, angle);
                }
            }
            if app.active_command.is_some() {
                assert!(app.try_continue_maelstrom("Cancel"));
            }
            assert!(app.active_command.is_none(), "{label}");
            assert!(app.maelstrom_session.is_none(), "{label}");
            assert_points(&app, &v["after"], true, label);
            // The probe appends a separate Cancel after successful commands.
            // Apply its transient selection cleanup before testing history.
            if v["events"]
                .as_array()
                .unwrap()
                .iter()
                .skip_while(|event| event["name"] != "Maelstrom")
                .skip(1)
                .any(|event| event["name"] == "Cancel")
            {
                app.document.clear_selection();
            }
            let placed = !v["undo"].as_array().unwrap().is_empty();
            assert_eq!(app.document.can_undo(), placed, "{label}");
            if placed {
                app.execute_command("Undo");
                assert_points(&app, &v["undo"], true, label);
                app.execute_command("Redo");
                assert_points(&app, &v["redo"], true, label);
            }
        }
    }
}

#[test]
fn accepted_circle_plane_survives_a_viewport_change() {
    let mut app = test_app();
    app.execute_command("Point 2,1,5");
    app.execute_command("SelAll");
    app.active_viewport = 2;
    app.viewports[2]
        .set_construction_plane(viboceros_command::construction_plane::WorldPlane::Top.frame());
    assert!(app.try_start_interactive_command("Maelstrom"));
    assert!(app.accept_drafting_point(point(0., 0., 0.)));
    assert!(app.accept_drafting_point(point(0., 0., 2.)));
    let frame = app.drafting_plane.unwrap();
    assert!(frame.z_axis().as_vector().to_array()[0] < -0.99);
    app.active_viewport = 3;
    app.viewports[3]
        .set_construction_plane(viboceros_command::construction_plane::WorldPlane::Front.frame());
    assert!(app.try_continue_maelstrom("5"));
    assert!(app.accept_drafting_point(point(0., 5., 0.)));
    let Geometry::Point(p) = app.document.objects().next().unwrap().geometry() else {
        panic!()
    };
    assert!(p.distance_to(point(2., 5., -1.)).unwrap() <= 1e-11);
    assert!(app.active_command.is_none());
}

#[test]
fn invalid_input_reprompts_units_preferences_and_context_survive_cancel() {
    let mut app = test_app();
    app.execute_command("Point 2,1,5");
    app.execute_command("SelAll");
    app.document.clear_history().unwrap();
    let before = app.document.objects().next().unwrap().geometry().clone();
    assert!(app.try_start_interactive_command("Maelstrom"));
    assert!(app.try_continue_maelstrom("Copy=Yes"));
    assert_eq!(
        app.commands.maelstrom_options_default(),
        MaelstromOptions::default()
    );
    assert!(app.accept_drafting_point(point(0., 0., 0.)));
    let state = app.active_command;
    for invalid in ["0", "-2", "NaN", "inf", "1e-12"] {
        assert!(app.try_continue_maelstrom(invalid));
        assert_eq!(app.active_command, state);
        assert_eq!(app.commands.maelstrom_radius_default(), 1.);
        assert!(!app.document.can_undo());
    }
    assert!(!app.accept_drafting_point(point(0., 0., 0.)));
    assert!(app.try_continue_maelstrom("2cm"));
    assert!(matches!(
        app.active_command,
        Some(InteractiveCommand::Maelstrom {
            initial: Some(MaelstromRadius::Number(20.)),
            ..
        })
    ));
    assert_eq!(app.commands.maelstrom_radius_default(), 20.);
    assert!(app.try_continue_maelstrom("Copy"));
    assert!(app.try_continue_maelstrom("Maybe"));
    assert!(!app.accept_drafting_point(point(0., 2., 0.)));
    assert!(app.try_continue_maelstrom("Yes"));
    assert!(app.commands.maelstrom_options_default().copy);
    assert!(!app.accept_drafting_point(point(0., 0., 5.)));
    assert!(app.try_continue_maelstrom("Rigid=Yes"));
    assert!(app.try_continue_maelstrom("Cancel"));
    assert_eq!(
        app.commands.maelstrom_options_default(),
        MaelstromOptions {
            copy: true,
            rigid: false
        }
    );
    assert_eq!(app.commands.maelstrom_radius_default(), 20.);
    assert_eq!(app.document.objects().next().unwrap().geometry(), &before);
    assert!(!app.document.can_undo());
    assert!(app.drafting_plane.is_none());
    assert!(app.try_start_interactive_command("Maelstrom"));
    assert!(app.accept_drafting_point(point(0., 0., 0.)));
    assert!(app.try_continue_maelstrom("Enter"));
    assert!(app.try_continue_maelstrom("0"));
    assert!(app.try_continue_maelstrom("NaN"));
    assert!(app.active_command.is_some());
    assert_eq!(app.document.objects().next().unwrap().geometry(), &before);
    assert!(app.try_continue_maelstrom("0"));
    assert_eq!(app.document.objects().count(), 2);
    assert!(app.try_continue_maelstrom("Enter"));
    assert!(app.document.can_undo());
    app.execute_command("Undo");
    assert_eq!(app.document.objects().count(), 1);
}

#[test]
fn native_mouse_turns_copy_and_cancel_use_pending_angle_without_document_edits() {
    let fixture: Value = serde_json::from_str(include_str!(
        "../../../tools/rhino_oracle/fixtures/maelstrom_preview.json"
    ))
    .unwrap();
    let captures: Value = serde_json::from_str(include_str!(
        "../../../tools/rhino_oracle/observations/maelstrom_preview.json"
    ))
    .unwrap();
    let p = |v: &Value| {
        Point3::try_from(serde_json::from_value::<[f64; 3]>(v.clone()).unwrap()).unwrap()
    };
    for index in [1, 12, 13, 14, 23, 24, 25, 26, 27, 28] {
        let op = &fixture["operations"][index];
        let v = &captures["results"][index]["value"];
        let mut app = test_app();
        let geometry = &v["before"][0]["geometry"];
        let id = app
            .document
            .add_geometry(Geometry::Line(
                viboceros_geometry::LineSegment::try_new(
                    p(&geometry["samples"][0]),
                    p(&geometry["samples"][64]),
                    app.document.tolerance(),
                )
                .unwrap(),
            ))
            .unwrap();
        app.document
            .select_object(id, SelectionMode::Replace)
            .unwrap();
        app.document.clear_history().unwrap();
        let before = app.document.object(id).unwrap().clone();
        assert!(app.try_start_interactive_command("Maelstrom"));
        assert!(app.accept_drafting_point(point(0., 0., 0.)));
        type_value(&mut app, &op["radius0"]);
        assert!(app.try_continue_maelstrom(&format!(
            "Copy={}",
            if op["copy"] == true { "Yes" } else { "No" }
        )));
        type_value(&mut app, &op["radius1"]);
        if op["phase"] == "Repeat" {
            assert!(app.try_continue_maelstrom("45"));
            assert!(app.maelstrom_preview().unwrap().last.is_none());
        }
        let angle = v["calibration"]["degrees"].as_f64().unwrap();
        let cursor = p(&v["calibration"]["valid_point"]);
        let preview_cursor = crate::viewport::MaelstromCursor {
            point: cursor,
            angle: Some(angle),
        };
        assert!(app.update_maelstrom_preview(preview_cursor));
        assert_eq!(app.document.object(id).unwrap(), &before);
        if op["phase"] != "Repeat" {
            assert!(!app.document.can_undo());
        }
        if op["finish"] == "Cancel" {
            app.cancel_interactive_command(true);
            assert!(app.maelstrom_preview().is_none());
            assert_eq!(app.document.object(id).unwrap(), &before);
            assert!(!app.document.can_undo());
            continue;
        }
        assert!(app.accept_filtered_drafting_point(cursor, false));
        if op["copy"] == true {
            assert!(app.maelstrom_preview().unwrap().last.is_none());
            assert!(app.try_continue_maelstrom("Enter"));
        }
        assert_preference_snapshot(&app, &v["after"], op["id"].as_str().unwrap());
        assert!(app.maelstrom_preview().is_none());
        assert!(app.document.can_undo());
        app.execute_command("Undo");
        assert_eq!(app.document.objects().count(), 1);
        assert_eq!(app.document.object(id).unwrap(), &before);
    }
}

#[test]
fn typed_coordinates_keep_native_mouse_turns_and_scalar_angles_override_them() {
    let fixture: Value = serde_json::from_str(include_str!(
        "../../../tools/rhino_oracle/fixtures/maelstrom_typed_hover.json"
    ))
    .unwrap();
    let captures: Value = serde_json::from_str(include_str!(
        "../../../tools/rhino_oracle/observations/maelstrom_typed_hover.json"
    ))
    .unwrap();
    let p = |v: &Value| {
        Point3::try_from(serde_json::from_value::<[f64; 3]>(v.clone()).unwrap()).unwrap()
    };
    let axis_fixture: Value = serde_json::from_str(include_str!(
        "../../../tools/rhino_oracle/fixtures/maelstrom_axis_hover.json"
    ))
    .unwrap();
    let axis_captures: Value = serde_json::from_str(include_str!(
        "../../../tools/rhino_oracle/observations/maelstrom_axis_hover.json"
    ))
    .unwrap();
    for (op, row) in fixture["operations"]
        .as_array()
        .unwrap()
        .iter()
        .zip(captures["results"].as_array().unwrap())
        .chain(
            axis_fixture["operations"]
                .as_array()
                .unwrap()
                .iter()
                .zip(axis_captures["results"].as_array().unwrap()),
        )
    {
        let v = &row["value"];
        let mut app = test_app();
        let ids = v["before"]
            .as_array()
            .unwrap()
            .iter()
            .filter(|o| o["witness"] != true)
            .map(|o| {
                app.document
                    .add_geometry(Geometry::Point(p(&o["geometry"]["points"][0])))
                    .unwrap()
            })
            .collect::<Vec<_>>();
        app.document
            .select_objects_direct(ids, SelectionMode::Replace)
            .unwrap();
        app.document.clear_history().unwrap();
        assert!(app.try_start_interactive_command("Maelstrom"));
        assert!(app.accept_drafting_point(point(0., 0., 0.)));
        type_value(&mut app, &op["radius0"]);
        type_value(&mut app, &op["radius1"]);
        assert!(
            app.update_maelstrom_preview(crate::viewport::MaelstromCursor {
                point: p(&v["calibration"]["valid_point"]),
                angle: Some(v["calibration"]["degrees"].as_f64().unwrap())
            })
        );
        assert!(!app.document.can_undo());
        match op["finish"].as_str().unwrap() {
            "Coordinate" => type_value(&mut app, &v["calibration"]["valid_point"]),
            "Number" => type_value(&mut app, &Value::from(-90.)),
            "Axis" => type_value(&mut app, &serde_json::json!([0., 0., 0.])),
            "Click" => {
                app.update_maelstrom_preview(crate::viewport::MaelstromCursor {
                    point: point(0., 0., 0.),
                    angle: Some(0.),
                });
                assert!(app.accept_filtered_drafting_point(point(0., 0., 0.), false));
            }
            _ => unreachable!(),
        }
        let filtered = |rows: &Value| {
            Value::Array(
                rows.as_array()
                    .unwrap()
                    .iter()
                    .filter(|o| o["witness"] != true)
                    .cloned()
                    .collect(),
            )
        };
        assert_preference_snapshot(&app, &filtered(&v["after"]), op["id"].as_str().unwrap());
        assert!(app.active_command.is_none());
        app.execute_command("Undo");
        assert_preference_snapshot(&app, &filtered(&v["before"]), op["id"].as_str().unwrap());
    }
}

#[test]
fn first_circle_modes_size_memory_and_second_diameter_match_native() {
    for (fixture, observed) in [
        (
            include_str!("../../../tools/rhino_oracle/fixtures/maelstrom_circle.json"),
            include_str!("../../../tools/rhino_oracle/observations/maelstrom_circle.json"),
        ),
        (
            include_str!("../../../tools/rhino_oracle/fixtures/maelstrom_circle_memory.json"),
            include_str!("../../../tools/rhino_oracle/observations/maelstrom_circle_memory.json"),
        ),
        (
            include_str!("../../../tools/rhino_oracle/fixtures/maelstrom_circle_point.json"),
            include_str!("../../../tools/rhino_oracle/observations/maelstrom_circle_point.json"),
        ),
        (
            include_str!("../../../tools/rhino_oracle/fixtures/maelstrom_circle_angle.json"),
            include_str!("../../../tools/rhino_oracle/observations/maelstrom_circle_angle.json"),
        ),
    ] {
        let fixture: Value = serde_json::from_str(fixture).unwrap();
        let observed: Value = serde_json::from_str(observed).unwrap();
        let mut app = test_app();
        for (op, row) in fixture["operations"]
            .as_array()
            .unwrap()
            .iter()
            .zip(observed["results"].as_array().unwrap())
        {
            let v = &row["value"];
            let label = op["id"].as_str().unwrap();
            app.document = Document::default();
            let p = |value: &Value| {
                Point3::try_from(serde_json::from_value::<[f64; 3]>(value.clone()).unwrap())
                    .unwrap()
            };
            let plane = Frame3::try_from_normal(
                p(&op["origin"]),
                viboceros_geometry::Vector3::try_from(p(&op["normal"]).to_array()).unwrap(),
                Tolerance::NUMERICAL_VALIDATION,
            )
            .unwrap();
            app.viewports[app.active_viewport].set_construction_plane(plane);
            app.document
                .begin_transaction("Maelstrom Circle sources")
                .unwrap();
            let ids = v["before"]
                .as_array()
                .unwrap()
                .iter()
                .map(|value| {
                    app.document
                        .add_geometry(Geometry::Point(p(&value["point"])))
                        .unwrap()
                })
                .collect::<Vec<_>>();
            app.document
                .select_objects_direct(ids, SelectionMode::Replace)
                .unwrap();
            app.document.commit_transaction().unwrap();
            assert!(app.try_start_interactive_command("Maelstrom"));
            let inputs = v["resolved_inputs"].as_array().unwrap();
            for (i, value) in inputs.iter().enumerate() {
                if value
                    .as_str()
                    .is_some_and(|name| name.starts_with("ProjectOsnap="))
                {
                    continue;
                }
                if let Some(name) = value.as_str() {
                    assert!(app.try_continue_maelstrom(name), "{label}: {name}");
                } else {
                    type_value(&mut app, value);
                }
                if app.active_command.is_none() {
                    break;
                }
                if v["circle"].is_null() || i + 1 < inputs.len().saturating_sub(4) {
                    assert_eq!(
                        app.document.undo_label(),
                        Some("Maelstrom Circle sources"),
                        "{label}: circle getter created history"
                    );
                    assert_eq!(app.document.objects().count(), 8);
                }
            }
            if !v["circle"].is_null() {
                assert!(
                    (app.commands.maelstrom_radius_default()
                        - v["circle"]["radius"].as_f64().unwrap())
                    .abs()
                        < 1e-11,
                    "{label}"
                );
            }
            assert_eq!(
                app.commands.maelstrom_uses_diameter(),
                v["diameter"].as_bool().unwrap(),
                "{label}"
            );
            if app.active_command.is_some() {
                app.cancel_interactive_command(true);
            }
            assert_points(&app, &v["after"], true, label);
            // The owned probe's terminal Cancel runs after successful Maelstrom.
            if v["events"]
                .as_array()
                .unwrap()
                .iter()
                .skip_while(|event| event["name"] != "Maelstrom")
                .skip(1)
                .any(|event| event["name"] == "Cancel")
            {
                app.document.clear_selection();
            }
            app.execute_command("Undo");
            assert_points(&app, &v["undo"], true, label);
            app.execute_command("Redo");
            assert_points(&app, &v["redo"], true, label);
        }
    }
}

#[test]
fn point_getter_scalars_use_drafting_constraints_while_circle_sizes_use_numbers() {
    let mut app = test_app();
    app.execute_command("Point 2,1,0");
    app.execute_command("SelAll");
    app.document.clear_history().unwrap();
    assert!(app.try_start_interactive_command("Maelstrom"));
    assert!(app.try_continue_maelstrom("2Point"));
    assert!(app.accept_drafting_point(point(0., 0., 0.)));
    app.command_input = "4".into();
    app.run_command();
    assert!(app.point_constraint.is_some());
    assert!(app.maelstrom_preview().unwrap().circle_getter.is_some());
    assert!(!app.document.can_undo());
    // A viewport pick follows the locked diameter distance.
    assert!(app.accept_filtered_drafting_point(point(0., 6., 0.), false));
    assert!((app.commands.maelstrom_radius_default() - 2.).abs() < 1e-11);
    assert!(app.point_constraint.is_none());
    assert!(!app.document.can_undo());
    app.cancel_interactive_command(true);

    assert!(app.try_start_interactive_command("Maelstrom"));
    assert!(app.accept_drafting_point(point(0., 0., 0.)));
    assert!(app.try_continue_maelstrom("_Diameter=8"));
    assert!(app.commands.maelstrom_uses_diameter());
    assert_eq!(app.commands.maelstrom_radius_default(), 4.);
    assert!(app.point_constraint.is_none());
    assert!(!app.document.can_undo());
    app.cancel_interactive_command(true);
}
