use super::*;
use viboceros_command::bend::BendOptions;

fn preference_sources(
    app: &mut VibocerosApp,
) -> std::collections::BTreeMap<&'static str, Vec<ObjectId>> {
    let tolerance = Tolerance::try_new(1e-5, 1e-12, 1e-9).unwrap();
    app.document = Document::new(tolerance);
    let mut sources = std::collections::BTreeMap::new();
    let points = [-2., 0., 2.5, 5., 7.5, 10., 12.]
        .map(|z| {
            app.document
                .add_geometry(Geometry::Point(Point3::try_new(2., 1., z).unwrap()))
                .unwrap()
        })
        .to_vec();
    sources.insert("Points", points);
    let curve = viboceros_geometry::NurbsCurve::try_new(
        3,
        [[1., 0., -2.], [3., 2., 2.], [2., -1., 8.], [3., 1., 12.]]
            .map(|v| Point3::try_from(v).unwrap())
            .to_vec(),
        vec![0., 0., 0., 0., 1., 1., 1., 1.],
    )
    .unwrap();
    sources.insert(
        "Curve",
        vec![
            app.document
                .add_geometry(Geometry::NurbsCurve(curve))
                .unwrap(),
        ],
    );
    let brep = viboceros_geometry::Brep::try_box(
        viboceros_command::CommandContext::default().construction_plane,
        [[1., 3.], [-1., 1.], [0., 10.]],
        tolerance,
    )
    .unwrap();
    sources.insert(
        "Box",
        vec![app.document.add_geometry(Geometry::Brep(brep)).unwrap()],
    );
    sources
}

fn assert_snapshot(app: &VibocerosApp, expected: &serde_json::Value, label: &str) {
    let rows = expected.as_array().unwrap();
    assert_eq!(app.document.objects().count(), rows.len(), "{label}");
    let p = |v: &serde_json::Value| {
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
            Geometry::Point(point) => near(*point, p(&geometry["points"][0]), 1e-7),
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
                assert_eq!(
                    curve.degree(),
                    geometry["definition"]["degree"].as_u64().unwrap() as usize,
                    "{label}"
                );
                assert_eq!(
                    curve.control_points().len(),
                    geometry["definition"]["control_points"]
                        .as_array()
                        .unwrap()
                        .len(),
                    "{label}"
                );
            }
            Geometry::Brep(brep) => {
                assert!(brep.is_solid());
                assert_eq!(
                    brep.faces().len(),
                    geometry["surfaces"].as_array().unwrap().len(),
                    "{label}"
                );
                let bounds = object
                    .geometry()
                    .tight_bounds(app.document.tolerance())
                    .unwrap();
                near(bounds.min(), p(&geometry["bounds"][0]), 2e-5);
                near(bounds.max(), p(&geometry["bounds"][1]), 2e-5);
            }
            _ => panic!("unexpected Bend preference geometry"),
        }
    }
}

#[test]
fn bend_preferences_match_forty_one_native_steps_and_all_terminal_snapshots() {
    for captured in [
        include_str!("../../../tools/rhino_oracle/observations/bend_options_command.json"),
        include_str!("../../../tools/rhino_oracle/observations/bend_options_followup_command.json"),
    ] {
        let captured: serde_json::Value = serde_json::from_str(captured).unwrap();
        let mut app = test_app();
        let mut sources = preference_sources(&mut app);
        let yes_no = |v| if v { "Yes" } else { "No" };
        for (i, record) in captured["results"][0]["value"]["records"]
            .as_array()
            .unwrap()
            .iter()
            .enumerate()
        {
            let label = format!("native Bend preference step {i}");
            for before in [true, false] {
                let query = &record[if before {
                    "query_before"
                } else {
                    "query_after"
                }];
                app.document
                    .select_objects_direct(sources["Curve"].iter().copied(), SelectionMode::Replace)
                    .unwrap();
                assert!(app.try_start_interactive_command("Bend"), "{label}");
                let Some(InteractiveCommand::Bend { options, .. }) = app.active_command else {
                    panic!("missing Bend prompt");
                };
                let expected = &query["defaults"];
                assert_eq!(
                    options,
                    BendOptions {
                        copy: expected["Copy"].as_bool().unwrap(),
                        rigid: expected["Rigid"].as_bool().unwrap(),
                        limit_to_spine: expected["LimitToSpine"].as_bool().unwrap(),
                        symmetric: expected["Symmetric"].as_bool().unwrap(),
                        preserve_structure: expected["PreserveStructure"].as_bool().unwrap(),
                        non_attenuated: expected["NonAttenuated"].as_bool().unwrap(),
                        angle: None,
                    },
                    "{label}"
                );
                assert_eq!(
                    app.commands.transform_scalar_default("Bend"),
                    query["angle_default"].as_f64(),
                    "{label}"
                );
                assert!(app.try_continue_bend("Cancel"));
                assert_snapshot(&app, &query["after"], &label);
                if !before {
                    continue;
                }
                let step = &record["step"];
                match step["kind"].as_str().unwrap() {
                    "Bend" => {
                        app.document
                            .select_objects_direct(
                                sources[step["shape"].as_str().unwrap()].iter().copied(),
                                SelectionMode::Replace,
                            )
                            .unwrap();
                        assert!(app.try_start_interactive_command("Bend"));
                        for p in [[0., 0., 0.], [0., 0., 10.]] {
                            assert!(app.accept_drafting_point(Point3::try_from(p).unwrap()));
                        }
                        for (name, value) in step["options"].as_object().unwrap() {
                            assert!(app.try_continue_bend(&format!(
                                "{name}={}",
                                yes_no(value.as_bool().unwrap())
                            )));
                        }
                        let finish = step["finish"].as_str().unwrap();
                        if finish != "Cancel" && finish != "CompleteThrough" {
                            assert!(app.try_continue_bend("Angle"));
                            assert!(
                                app.try_continue_bend(
                                    &step["degrees"].as_f64().unwrap().to_string()
                                )
                            );
                            if finish != "AngleCancel" {
                                assert!(
                                    app.accept_drafting_point(
                                        Point3::try_new(10., 0., 10.).unwrap()
                                    )
                                );
                            }
                        }
                        if matches!(finish, "CopyThenCancel" | "CopyThenEnter") {
                            for angle in [90., 0.] {
                                assert!(app.try_continue_bend("Angle"));
                                assert!(app.try_continue_bend(&angle.to_string()));
                            }
                            for (name, value) in step["pending_options"].as_object().unwrap() {
                                assert!(app.try_continue_bend(&format!(
                                    "{name}={}",
                                    yes_no(value.as_bool().unwrap())
                                )));
                            }
                        }
                        if finish == "CompleteThrough" {
                            assert!(
                                app.accept_drafting_point(Point3::try_new(10., 0., 10.).unwrap())
                            );
                        }
                        if app.active_command.is_some() {
                            assert!(app.try_continue_bend(
                                if matches!(
                                    finish,
                                    "Complete" | "CompleteThrough" | "CopyThenEnter"
                                ) {
                                    "Enter"
                                } else {
                                    "Cancel"
                                }
                            ));
                        }
                    }
                    "New" => sources = preference_sources(&mut app),
                    "RememberCopyOptions" => app.execute_command(&format!(
                        "RememberCopyOptions {}",
                        yes_no(step["enabled"].as_bool().unwrap())
                    )),
                    name => app.execute_command(name),
                }
                if step["kind"] != "New" {
                    assert_snapshot(&app, &record["result"]["after"], &label);
                }
            }
        }
    }
}

#[test]
fn bend_command_first_source_selection_invalid_points_and_copy_history() {
    let mut app = test_app();
    app.execute_command("Point 2,1,5");
    let id = app.document.objects().next().unwrap().id();
    assert!(app.try_start_interactive_command("Bend"));
    assert!(app.object_prompt.is_some());
    app.document
        .select_object(id, SelectionMode::Replace)
        .unwrap();
    assert!(app.try_continue_transform_source_prompt("Enter"));
    assert!(app.accept_drafting_point(Point3::try_new(0., 0., 0.).unwrap()));
    assert!(!app.accept_drafting_point(Point3::try_new(0., 0., 0.).unwrap()));
    assert!(app.accept_drafting_point(Point3::try_new(0., 0., 10.).unwrap()));
    assert!(!app.accept_drafting_point(Point3::try_new(0., 0., 5.).unwrap()));
    assert!(app.try_continue_bend("Copy=Yes"));
    for through in [[10., 0., 10.], [5., 0., 5.]] {
        assert!(app.accept_drafting_point(Point3::try_from(through).unwrap()));
    }
    assert_eq!(app.document.objects().count(), 3);
    assert!(app.try_continue_bend("Cancel"));
    assert!(app.active_command.is_none());
    app.execute_command("Undo");
    assert_eq!(app.document.objects().count(), 1);
    let Geometry::Point(source) = app.document.object(id).unwrap().geometry() else {
        panic!();
    };
    assert_eq!(*source, Point3::try_new(2., 1., 5.).unwrap());
    app.execute_command("Redo");
    assert_eq!(app.document.objects().count(), 3);
    assert_eq!(app.document.selected_object_count(), 0);
}

#[test]
fn bend_angle_prompt_recalls_values_and_zero_restores_through_point_mode() {
    let mut app = test_app();
    app.execute_command("Point 2,1,5");
    app.execute_command("SelAll");
    assert!(app.try_start_interactive_command("Bend"));
    for p in [[0., 0., 0.], [0., 0., 10.]] {
        assert!(app.accept_drafting_point(Point3::try_from(p).unwrap()));
    }
    assert!(app.try_continue_bend("Angle"));
    let state = app.active_command;
    assert!(app.try_continue_bend("NaN"));
    assert_eq!(app.active_command, state);
    assert!(app.try_continue_bend("90"));
    assert!(app.try_continue_bend("Cancel"));
    assert_eq!(app.commands.transform_scalar_default("Bend"), Some(90.));
    assert!(app.try_start_interactive_command("Bend"));
    for p in [[0., 0., 0.], [0., 0., 10.]] {
        assert!(app.accept_drafting_point(Point3::try_from(p).unwrap()));
    }
    assert!(app.try_continue_bend("Angle"));
    assert!(app.try_continue_bend("Enter"));
    assert!(matches!(
        app.active_command,
        Some(InteractiveCommand::Bend {
            options: BendOptions {
                angle: Some(90.),
                ..
            },
            ..
        })
    ));
    assert!(app.try_continue_bend("Angle=0"));
    assert_eq!(app.commands.transform_scalar_default("Bend"), None);
    assert!(app.try_continue_bend("Cancel"));
    assert_eq!(app.document.objects().count(), 1);
}

#[test]
fn bend_preview_uses_saved_angle_and_resets_after_copies_and_cancellation() {
    let mut app = test_app();
    app.execute_command("Point 2,1,5");
    app.execute_command("SelAll");
    app.commands.remember_bend_prompt_option(
        BendOptions {
            angle: Some(60.),
            ..Default::default()
        },
        "Angle",
    );
    assert!(app.try_start_interactive_command("Bend"));
    assert!(app.bend_preview().is_none());
    assert!(!app.update_bend_preview(Some(Point3::try_new(10., 0., 10.).unwrap())));
    for p in [[0., 0., 0.], [0., 0., 10.]] {
        assert!(app.accept_drafting_point(Point3::try_from(p).unwrap()));
    }
    assert_eq!(app.bend_preview().unwrap().options.angle, Some(60.));
    let before = app.document.objects().cloned().collect::<Vec<_>>();
    let target = Point3::try_new(10., 0., 10.).unwrap();
    assert!(app.update_bend_preview(Some(target)));
    assert!(!app.update_bend_preview(Some(target)));
    assert_eq!(app.document.objects().cloned().collect::<Vec<_>>(), before);
    assert!(app.try_continue_bend("Angle=0"));
    assert_eq!(app.bend_preview().unwrap().options.angle, Some(0.));
    assert!(app.try_continue_bend("Copy=Yes"));
    assert!(app.accept_drafting_point(target));
    assert!(app.bend_preview().unwrap().last_point.is_none());
    assert_eq!(app.document.objects().count(), 2);
    assert!(app.update_bend_preview(Some(target)));
    assert!(app.update_bend_preview(None));
    assert!(app.try_continue_bend("Cancel"));
    assert!(app.bend_preview().is_none());
    assert!(!app.update_bend_preview(Some(target)));
    assert_eq!(app.document.objects().count(), 2);
    app.execute_command("Undo");
    assert_eq!(app.document.objects().count(), 1);
}
