use super::*;
use viboceros_command::taper::{TaperDistance, TaperOptions};

#[test]
fn taper_preview_clears_between_radius_phases_and_copy_placements() {
    let mut app = test_app();
    let source = app
        .document
        .add_geometry(Geometry::Point(Point3::try_new(2., 1., 5.).unwrap()))
        .unwrap();
    app.document
        .select_object(source, SelectionMode::Replace)
        .unwrap();
    app.document.clear_history().unwrap();
    let before = app.document.object(source).unwrap().geometry().clone();
    assert!(app.try_start_interactive_command("Taper"));
    for p in [[0., 0., 0.], [0., 0., 10.]] {
        assert!(app.accept_drafting_point(Point3::try_from(p).unwrap()));
    }
    let cursor = Point3::try_new(1., 0., 10.).unwrap();
    assert!(app.taper_preview().unwrap().initial.is_none());
    assert!(app.update_taper_preview(Some(cursor)));
    assert!(app.try_continue_taper("2"));
    assert_eq!(app.taper_preview().unwrap().last_point, None);
    assert!(app.update_taper_preview(Some(cursor)));
    assert_eq!(app.document.object(source).unwrap().geometry(), &before);
    assert!(!app.document.can_undo());
    assert!(app.try_continue_taper("Copy=Yes"));
    assert!(app.accept_drafting_point(cursor));
    assert_eq!(app.document.objects().count(), 2);
    assert_eq!(app.taper_preview().unwrap().last_point, None);
    assert!(app.update_taper_preview(Some(cursor)));
    assert!(app.try_continue_taper("Cancel"));
    assert!(app.taper_preview().is_none());
    assert_eq!(app.document.objects().count(), 2);
    app.execute_command("Undo");
    assert_eq!(app.document.objects().count(), 1);
    assert_eq!(app.document.object(source).unwrap().geometry(), &before);
}

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
            _ => panic!("unexpected Taper preference geometry"),
        }
    }
}

#[test]
fn taper_preferences_and_terminal_geometry_match_thirty_three_native_steps() {
    let captured: serde_json::Value = serde_json::from_str(include_str!(
        "../../../tools/rhino_oracle/observations/taper_options_command.json"
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
        let label = format!("native Taper preference step {i}");
        for before in [true, false] {
            let query = &record[if before {
                "query_before"
            } else {
                "query_after"
            }];
            app.document
                .select_objects_direct(sources["Curve"].iter().copied(), SelectionMode::Replace)
                .unwrap();
            assert!(app.try_start_interactive_command("Taper"), "{label}");
            let Some(InteractiveCommand::Taper { options, .. }) = app.active_command else {
                panic!("missing Taper prompt");
            };
            let expected = &query["defaults"];
            assert_eq!(
                options,
                TaperOptions {
                    copy: expected["Copy"].as_bool().unwrap(),
                    rigid: expected["Rigid"].as_bool().unwrap(),
                    flat: expected["Flat"].as_bool().unwrap(),
                    infinite: expected["Infinite"].as_bool().unwrap(),
                    preserve_structure: expected["PreserveStructure"].as_bool().unwrap(),
                },
                "{label}"
            );
            for p in [[0., 0., 0.], [0., 0., 10.]] {
                assert!(app.accept_drafting_point(Point3::try_from(p).unwrap()));
            }
            assert!(app.try_continue_taper("Cancel"));
            assert_snapshot(&app, &query["after"], &label);
            if !before {
                continue;
            }
            let step = &record["step"];
            match step["kind"].as_str().unwrap() {
                "Taper" => {
                    app.document
                        .select_objects_direct(
                            sources[step["shape"].as_str().unwrap()].iter().copied(),
                            SelectionMode::Replace,
                        )
                        .unwrap();
                    assert!(app.try_start_interactive_command("Taper"));
                    for p in [[0., 0., 0.], [0., 0., 10.]] {
                        assert!(app.accept_drafting_point(Point3::try_from(p).unwrap()));
                    }
                    for (name, v) in step["options"].as_object().unwrap() {
                        assert!(app.try_continue_taper(&format!(
                            "{name}={}",
                            yes_no(v.as_bool().unwrap())
                        )));
                    }
                    let finish = step["finish"].as_str().unwrap();
                    if finish != "CancelStart" {
                        assert!(app.try_continue_taper("2"));
                    }
                    if !matches!(finish, "CancelStart" | "CancelEnd") {
                        assert!(app.try_continue_taper("1"));
                    }
                    if finish.starts_with("CopyThen") {
                        for (name, v) in step["pending_options"].as_object().unwrap() {
                            assert!(app.try_continue_taper(&format!(
                                "{name}={}",
                                yes_no(v.as_bool().unwrap())
                            )));
                        }
                    }
                    if app.active_command.is_some() {
                        assert!(app.try_continue_taper(
                            if matches!(finish, "Complete" | "CopyThenEnter") {
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

#[test]
fn command_first_selection_zero_reprompt_point_distances_and_repeated_copy_history() {
    let mut app = test_app();
    app.execute_command("Point 2,1,5");
    let id = app.document.objects().next().unwrap().id();
    assert!(app.try_start_interactive_command("Taper"));
    assert!(app.object_prompt.is_some());
    app.document
        .select_object(id, SelectionMode::Replace)
        .unwrap();
    assert!(app.try_continue_transform_source_prompt("Enter"));
    assert!(app.accept_drafting_point(point(0., 0., 0.)));
    assert!(!app.accept_drafting_point(point(0., 0., 0.)));
    assert!(app.accept_drafting_point(point(0., 0., 10.)));
    let state = app.active_command;
    for invalid in ["0", "NaN", "1e-12"] {
        assert!(app.try_continue_taper(invalid));
        assert_eq!(app.active_command, state);
    }
    assert!(!app.accept_drafting_point(point(0., 0., 5.)));
    assert!(app.try_continue_taper("Copy"));
    assert!(app.try_continue_taper("Yes"));
    assert!(app.try_continue_taper("Flat=Yes"));
    assert!(app.accept_drafting_point(point(0., 2., 5.)));
    for target in [point(0., 1., 10.), point(0., 3., -5.)] {
        assert!(app.accept_drafting_point(target));
    }
    let positions = app
        .document
        .objects()
        .map(|o| {
            if let Geometry::Point(p) = o.geometry() {
                *p
            } else {
                panic!()
            }
        })
        .collect::<Vec<_>>();
    assert_eq!(
        positions,
        vec![point(2., 1., 5.), point(2., 0.75, 5.), point(2., 1.25, 5.)]
    );
    assert!(app.try_continue_taper("Cancel"));
    assert!(app.active_command.is_none());
    app.execute_command("Undo");
    assert_eq!(app.document.objects().count(), 1);
    app.execute_command("Redo");
    assert_eq!(app.document.objects().count(), 3);
    assert_eq!(app.document.selected_object_count(), 0);
    assert_eq!(
        app.commands.taper_options_default(),
        TaperOptions::default()
    );
}

#[test]
fn typed_signed_distances_use_the_scalar_prompt_and_flat_uses_the_active_cplane() {
    let mut app = test_app();
    app.execute_command("Point 2,1,5");
    app.execute_command("SelAll");
    app.active_viewport = 2;
    let cplane = Frame3::try_from_directions(
        point(0., 0., 0.),
        viboceros_geometry::Vector3::try_from([0., 1., 0.]).unwrap(),
        viboceros_geometry::Vector3::try_from([0., 0., 1.]).unwrap(),
        Tolerance::DEFAULT,
    )
    .unwrap();
    app.viewports[2].set_construction_plane(cplane);
    assert!(app.try_start_interactive_command("Taper"));
    for input in ["w0,0,0", "w0,0,10", "Flat=Yes", "2", "-1"] {
        app.command_input = input.to_owned();
        app.run_command();
    }
    assert!(app.active_command.is_none());
    assert!(app.point_constraint.is_none());
    let Geometry::Point(p) = app.document.objects().next().unwrap().geometry() else {
        panic!();
    };
    assert_eq!(*p, point(2., 0.25, 5.));
    assert!(app.commands.taper_options_default().flat);
    app.execute_command("Undo");
    app.execute_command("Redo");
    assert_eq!(app.document.selected_object_count(), 1);
    assert!(app.try_start_interactive_command("Taper"));
    assert!(app.try_continue_taper("Cancel"));
    assert!(app.taper_session.is_none());
    assert!(app.try_start_interactive_command("Taper"));
    for p in [point(0., 0., 0.), point(0., 0., 10.)] {
        assert!(app.accept_drafting_point(p));
    }
    assert!(app.try_continue_taper("2cm"));
    assert!(matches!(
        app.active_command,
        Some(InteractiveCommand::Taper {
            initial: Some(TaperDistance::Number(20.)),
            ..
        })
    ));
    assert!(app.try_continue_taper("Cancel"));
}
