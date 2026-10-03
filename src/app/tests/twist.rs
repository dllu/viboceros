use super::*;
use viboceros_geometry::Vector3;

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

fn assert_twist_preference_snapshot(app: &VibocerosApp, expected: &serde_json::Value, label: &str) {
    let expected = expected.as_array().unwrap();
    assert_eq!(app.document.objects().count(), expected.len(), "{label}");
    let point = |v: &serde_json::Value| {
        Point3::try_from(serde_json::from_value::<[f64; 3]>(v.clone()).unwrap()).unwrap()
    };
    let near = |a: Point3, b: Point3| {
        assert!(a.distance_to(b).unwrap() <= 1e-7, "{label}: {a:?} != {b:?}")
    };
    for (object, row) in app.document.objects().zip(expected) {
        assert_eq!(
            app.document.is_selected(object.id()),
            row["selected"].as_bool().unwrap(),
            "{label}"
        );
        let geometry = &row["geometry"];
        match object.geometry() {
            Geometry::Point(p) => near(*p, point(&geometry["points"][0])),
            Geometry::NurbsCurve(curve) => {
                let domain = curve.domain();
                for (i, value) in geometry["samples"].as_array().unwrap().iter().enumerate() {
                    let t = domain.start() + (domain.end() - domain.start()) * i as f64 / 64.;
                    near(curve.evaluate(t).unwrap(), point(value));
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
                assert_eq!(
                    brep.faces().len(),
                    geometry["surfaces"].as_array().unwrap().len(),
                    "{label}"
                );
                let bounds = object
                    .geometry()
                    .tight_bounds(app.document.tolerance())
                    .unwrap();
                near(bounds.min(), point(&geometry["bounds"][0]));
                near(bounds.max(), point(&geometry["bounds"][1]));
            }
            _ => panic!("unexpected Twist preference geometry"),
        }
    }
}

#[test]
fn twist_preferences_match_native_completion_cancellation_history_and_new_document() {
    let captured: serde_json::Value = serde_json::from_str(include_str!(
        "../../../tools/rhino_oracle/observations/twist_options_command.json"
    ))
    .unwrap();
    let mut app = test_app();
    let mut sources = preference_sources(&mut app);
    let yes_no = |v| if v { "Yes" } else { "No" };
    for (i, record) in captured["results"][0]["value"]["records"]
        .as_array()
        .unwrap()
        .iter()
        .enumerate()
    {
        let label = format!("native Twist preference step {i}");
        for before in [true, false] {
            let query = &record[if before {
                "query_before"
            } else {
                "query_after"
            }];
            app.document
                .select_objects_direct(sources["Curve"].iter().copied(), SelectionMode::Replace)
                .unwrap();
            assert!(app.try_start_interactive_command("Twist"), "{label}");
            let Some(InteractiveCommand::Twist { options, .. }) = app.active_command else {
                panic!("missing Twist prompt");
            };
            let expected = &query["defaults"];
            assert_eq!(
                options,
                viboceros_command::twist::TwistOptions {
                    copy: expected["Copy"].as_bool().unwrap(),
                    rigid: expected["Rigid"].as_bool().unwrap(),
                    infinite: expected["Infinite"].as_bool().unwrap(),
                    preserve_structure: expected["PreserveStructure"].as_bool().unwrap(),
                },
                "{label}"
            );
            assert!(app.try_continue_twist("Cancel"));
            assert_twist_preference_snapshot(&app, &query["after"], &label);
            if !before {
                continue;
            }
            let step = &record["step"];
            match step["kind"].as_str().unwrap() {
                "Twist" => {
                    app.document
                        .select_objects_direct(
                            sources[step["shape"].as_str().unwrap()].iter().copied(),
                            SelectionMode::Replace,
                        )
                        .unwrap();
                    assert!(app.try_start_interactive_command("Twist"));
                    for p in [[0., 0., 0.], [0., 0., 10.]] {
                        assert!(app.accept_drafting_point(Point3::try_from(p).unwrap()));
                    }
                    for (name, value) in step["options"].as_object().unwrap() {
                        assert!(app.try_continue_twist(&format!(
                            "{name}={}",
                            yes_no(value.as_bool().unwrap())
                        )));
                    }
                    match step["finish"].as_str().unwrap() {
                        "Complete" | "CopyThenCancel" => {
                            assert!(app.try_continue_twist(
                                &step["degrees"].as_f64().unwrap().to_string()
                            ));
                            if step["finish"] == "CopyThenCancel" {
                                for (name, value) in step["pending_options"].as_object().unwrap() {
                                    assert!(app.try_continue_twist(&format!(
                                        "{name}={}",
                                        yes_no(value.as_bool().unwrap())
                                    )));
                                }
                                assert!(app.try_continue_twist("Cancel"));
                            } else if app.active_command.is_some() {
                                assert!(app.try_continue_twist("Enter"));
                            }
                        }
                        "ReferenceCancel" => {
                            assert!(
                                app.accept_drafting_point(Point3::try_new(1., 0., 0.).unwrap())
                            );
                            assert!(app.try_continue_twist("Cancel"));
                        }
                        _ => {
                            assert!(app.try_continue_twist("Cancel"));
                        }
                    }
                }
                "New" => {
                    sources = preference_sources(&mut app);
                }
                "RememberCopyOptions" => {
                    app.execute_command(&format!(
                        "RememberCopyOptions {}",
                        yes_no(step["enabled"].as_bool().unwrap())
                    ));
                }
                name => {
                    app.execute_command(name);
                }
            }
            if step["kind"] != "New" {
                assert_twist_preference_snapshot(&app, &record["result"]["after"], &label);
            }
        }
    }
    assert_eq!(
        app.document.objects().count(),
        captured["results"][0]["value"]["records"]
            .as_array()
            .unwrap()
            .last()
            .unwrap()["query_after"]["after"]
            .as_array()
            .unwrap()
            .len()
    );
}
#[test]
fn twist_axis_angle_options_cancel_copy_and_external_undo() {
    let mut app = test_app();
    app.execute_command("Point 2,1,5");
    app.execute_command("SelAll");
    assert!(app.try_start_interactive_command("Twist"));
    assert!(app.accept_drafting_point(Point3::try_new(0., 0., 0.).unwrap()));
    assert!(!app.accept_drafting_point(Point3::try_new(0., 0., 0.).unwrap()));
    assert!(app.accept_drafting_point(Point3::try_new(0., 0., 10.).unwrap()));
    assert!(app.try_continue_twist("Copy=Yes"));
    assert!(app.try_continue_twist("Rigid"));
    assert!(app.try_continue_twist("No"));
    assert!(app.try_continue_twist("90"));
    assert!(app.active_command.is_some());
    assert_eq!(app.document.objects().count(), 2);
    assert!(app.try_continue_twist("180"));
    assert_eq!(app.document.objects().count(), 3);
    assert!(app.try_continue_twist("Enter"));
    assert!(app.active_command.is_none());
    app.execute_command("Undo");
    assert_eq!(app.document.objects().count(), 1);
    app.execute_command("Redo");
    assert_eq!(app.document.objects().count(), 3);
}
#[test]
fn command_first_twist_uses_picked_sources_and_reference_points() {
    let mut app = test_app();
    app.execute_command("Point 2,1,5");
    let id = app.document.objects().next().unwrap().id();
    assert!(app.try_start_interactive_command("Twist"));
    assert!(app.object_prompt.is_some());
    app.document
        .select_object(id, SelectionMode::Replace)
        .unwrap();
    assert!(app.try_continue_transform_source_prompt("Enter"));
    for p in [[0., 0., 0.], [0., 0., 10.], [1., 0., 0.], [0., 1., 0.]] {
        assert!(app.accept_drafting_point(Point3::try_from(p).unwrap()));
    }
    assert!(app.active_command.is_none());
    assert_eq!(app.document.selected_object_count(), 0);
    let Geometry::Point(point) = app.document.object(id).unwrap().geometry() else {
        panic!();
    };
    assert!(
        point
            .distance_to(Point3::try_new(2_f64.sqrt() / 2., 3. * 2_f64.sqrt() / 2., 5.).unwrap())
            .unwrap()
            < 1e-12
    );
}

#[test]
fn tilted_twist_reference_plane_cancel_and_invalid_angle_retain_prompt_state() {
    let mut app = test_app();
    app.execute_command("Point 5,2,1");
    app.execute_command("SelAll");
    assert!(app.try_start_interactive_command("Twist"));
    for p in [[0., 0., 0.], [10., 0., 0.]] {
        assert!(app.accept_drafting_point(Point3::try_from(p).unwrap()));
    }
    let normal = app.drafting_plane.unwrap().z_axis().as_vector();
    assert!(normal.dot(Vector3::try_new(1., 0., 0.).unwrap()).unwrap() > 1. - 1e-12);
    let state = app.active_command;
    assert!(app.try_continue_twist("NaN"));
    assert_eq!(app.active_command, state);
    assert!(app.try_continue_twist("Cancel"));
    assert!(app.drafting_plane.is_none());
    assert!(app.twist_session.is_none());
    assert_eq!(app.document.selected_object_count(), 1);
}

#[test]
fn twist_mouse_turns_numeric_override_and_copy_cancel_use_one_history_entry() {
    let mut app = test_app();
    app.execute_command("Point 2,1,5");
    app.execute_command("SelAll");
    let id = app.document.objects().next().unwrap().id();
    assert!(app.try_start_interactive_command("Twist"));
    for p in [[0., 0., 0.], [0., 0., 10.], [5., 0., 0.]] {
        assert!(app.accept_drafting_point(Point3::try_from(p).unwrap()));
    }
    let original = app.document.object(id).unwrap().geometry().clone();
    assert_eq!(app.twist_preview().unwrap().last_angle, Some(0.));
    for angle in [90., 180., 270., 360., 450.] {
        assert!(app.update_twist_preview(Some(angle)));
    }
    assert!(!app.accept_drafting_point(Point3::try_new(0., 0., 2.).unwrap()));
    assert_eq!(app.twist_preview().unwrap().last_angle, Some(450.));
    assert_eq!(app.document.object(id).unwrap().geometry(), &original);
    assert!(app.try_continue_twist("Copy=Yes"));
    assert!(app.accept_drafting_point(Point3::try_new(0., 5., 0.).unwrap()));
    let first = app
        .document
        .objects()
        .find(|o| o.id() != id)
        .unwrap()
        .geometry()
        .clone();
    let Geometry::Point(first) = first else {
        panic!()
    };
    assert!(
        first
            .distance_to(Point3::try_new(-2_f64.sqrt() / 2., -3. * 2_f64.sqrt() / 2., 5.).unwrap())
            .unwrap()
            < 1e-12
    );
    assert!(app.twist_preview().is_none());
    assert!(app.accept_drafting_point(Point3::try_new(5., 0., 0.).unwrap()));
    assert!(app.update_twist_preview(Some(810.)));
    // A scalar remains explicit even after accepting the first reference direction.
    assert!(app.try_continue_twist("90"));
    let second = app
        .document
        .objects()
        .filter(|o| o.id() != id)
        .find(|o| o.geometry() != &Geometry::Point(first))
        .unwrap()
        .geometry();
    let Geometry::Point(second) = second else {
        panic!()
    };
    assert!(
        second
            .distance_to(Point3::try_new(2_f64.sqrt() / 2., 3. * 2_f64.sqrt() / 2., 5.).unwrap())
            .unwrap()
            < 1e-12
    );
    assert!(app.try_continue_twist("Cancel"));
    assert!(app.twist_preview().is_none());
    assert_eq!(app.document.objects().count(), 3);
    app.execute_command("Undo");
    assert_eq!(app.document.objects().count(), 1);
    assert_eq!(app.document.object(id).unwrap().geometry(), &original);
    app.execute_command("Redo");
    assert_eq!(app.document.objects().count(), 3);
}
