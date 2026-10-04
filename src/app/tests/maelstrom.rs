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
