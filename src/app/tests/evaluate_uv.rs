use super::*;

#[test]
fn evaluate_uv_replays_rhino_repeated_picks_esc_and_persisted_options() {
    use serde_json::Value;
    use viboceros_geometry::{NurbsSurface, WeightedPoint3};
    let request: Value = serde_json::from_str(include_str!(
        "../../../tools/rhino_oracle/fixtures/evaluate-uv-session.json"
    ))
    .unwrap();
    let response: Value = serde_json::from_str(include_str!(
        "../../../docs/evaluate-uv-session-rhino-reference.json"
    ))
    .unwrap();
    let operations = request["operations"].as_array().unwrap();
    let results = response["results"].as_array().unwrap();
    assert_eq!(operations.len(), 4);
    assert_eq!(operations.len(), results.len());
    // Keep the registry across documents so each inherited invocation really
    // consumes the choices made in the previous captured operation.
    let mut app = test_app();
    for (operation, result) in operations.iter().zip(results) {
        assert_eq!(operation["id"], result["id"]);
        app.document = Document::default();
        let definition = &operation["surface"];
        let size = |key| definition[key].as_u64().unwrap() as usize;
        let surface = NurbsSurface::try_new_rational(
            size("degree_u"),
            size("degree_v"),
            size("control_point_count_u"),
            size("control_point_count_v"),
            definition["control_points"]
                .as_array()
                .unwrap()
                .iter()
                .map(|control| {
                    let [x, y, z] =
                        serde_json::from_value::<[f64; 3]>(control["point"].clone()).unwrap();
                    WeightedPoint3::try_new(point(x, y, z), control["weight"].as_f64().unwrap())
                        .unwrap()
                })
                .collect(),
            serde_json::from_value(definition["knots_u"].clone()).unwrap(),
            serde_json::from_value(definition["knots_v"].clone()).unwrap(),
        )
        .unwrap();
        let id = app
            .document
            .add_geometry(Geometry::NurbsSurface(surface))
            .unwrap();
        app.document
            .select_object(id, viboceros_document::SelectionMode::Replace)
            .unwrap();
        let source = format!("{:?}", app.document.object(id).unwrap());
        app.command_input = if operation["inherit_options"].as_bool().unwrap_or(false) {
            "EvaluateUVPt".into()
        } else {
            viboceros_command::EvaluateUvOptions {
                normalized: operation["normalized"].as_bool().unwrap_or(false),
                create_point: operation["create_point"].as_bool().unwrap_or(false),
            }
            .command_line()
        };
        app.run_command();
        let events = operation
            .get("events")
            .cloned()
            .unwrap_or_else(|| serde_json::json!([{"point": operation["point"]}]));
        let expected = result["value"]["history"]
            .as_str()
            .unwrap()
            .lines()
            .filter_map(|line| line.strip_prefix("UV coordinates of point = "))
            .map(|line| {
                line.split(',')
                    .map(|v| v.trim().parse::<f64>().unwrap())
                    .collect::<Vec<_>>()
            })
            .collect::<Vec<_>>();
        let mut reports = Vec::new();
        for event in events.as_array().unwrap() {
            for (key, name) in [
                ("normalized", "Normalized"),
                ("create_point", "CreatePoint"),
            ] {
                if let Some(value) = event[key].as_bool() {
                    app.command_input = format!("{name}={}", if value { "Yes" } else { "No" });
                    app.run_command();
                }
            }
            if let Some(value) = event.get("point") {
                let [x, y, z] = serde_json::from_value::<[f64; 3]>(value.clone()).unwrap();
                app.command_input = format!("w{x},{y},{z}");
                app.run_command();
                reports.push(
                    app.command_log
                        .back()
                        .unwrap()
                        .split_whitespace()
                        .nth(4)
                        .unwrap()
                        .split(',')
                        .map(|v| v.parse::<f64>().unwrap())
                        .collect::<Vec<_>>(),
                );
                assert!(app.active_command.is_some());
            }
        }
        assert_eq!(reports.len(), expected.len());
        for (a, b) in reports.iter().zip(expected) {
            assert_eq!(a.len(), 2);
            assert_eq!(b.len(), 2);
            for (a, b) in a.iter().zip(b) {
                assert!((a - b).abs() < 1e-8);
            }
        }
        if operation["ending"] == "cancel" {
            app.cancel_interactive_command(true);
        } else {
            app.command_input.clear();
            app.run_command();
        }
        assert!(app.active_command.is_none());
        assert!(app.evaluate_uv_session.is_none());
        let mut actual = app
            .document
            .objects()
            .filter(|o| o.id() != id)
            .map(|o| match o.geometry() {
                Geometry::Point(p) => p.to_array(),
                _ => panic!("unexpected geometry"),
            })
            .collect::<Vec<_>>();
        actual.sort_by(|a, b| a.partial_cmp(b).unwrap());
        let expected: Vec<[f64; 3]> =
            serde_json::from_value(result["value"]["created_points"].clone()).unwrap();
        assert_eq!(actual.len(), expected.len());
        for (a, b) in actual.iter().zip(expected) {
            for (a, b) in a.iter().zip(b) {
                assert!((a - b).abs() < 1e-8);
            }
        }
        assert_eq!(result["value"]["source_geometry_unchanged"], true);
        assert_eq!(format!("{:?}", app.document.object(id).unwrap()), source);
        // The nested Rhino probe reports "Nothing to undo/redo". Its diagnostic
        // after_undo/after_redo fields are deliberately not undo-parity assertions.
    }
}

#[test]
fn evaluate_uv_pre_and_postselection_preserve_options_and_query_history() {
    for preselected in [false, true] {
        let mut app = test_app();
        let surface = viboceros_geometry::NurbsSurface::try_bilinear([
            point(0., 0., 0.),
            point(4., 0., 0.),
            point(4., 2., 0.),
            point(0., 2., 0.),
        ])
        .unwrap();
        let id = app
            .document
            .add_geometry(Geometry::NurbsSurface(surface))
            .unwrap();
        if preselected {
            app.document
                .select_object(id, viboceros_document::SelectionMode::Replace)
                .unwrap();
        }
        app.command_input = "EvaluateUVPt Normalized=Yes".into();
        app.run_command();
        if !preselected {
            assert!(app.object_prompt.is_some());
            app.select_prompt_objects([id], viboceros_document::SelectionMode::Replace);
            let selected = format!("{:?}", app.document);
            assert!(app.try_continue_object_prompt(""));
            assert_eq!(format!("{:?}", app.document), selected);
        }
        assert!(app.object_prompt.is_none());
        assert_eq!(
            app.active_command,
            Some(InteractiveCommand::EvaluateUv {
                options: viboceros_command::EvaluateUvOptions {
                    normalized: true,
                    create_point: false
                }
            })
        );
        let before = format!("{:?}", app.document);
        assert!(app.accept_drafting_point(point(1., 1., 3.)));
        assert!(app.active_command.is_some());
        assert_eq!(
            app.command_log.back().unwrap(),
            "Surface UV coordinates = 0.25,0.5 (normalized)"
        );
        assert_eq!(format!("{:?}", app.document), before);
        app.command_input.clear();
        app.run_command();
        assert!(app.active_command.is_none());
        assert_eq!(format!("{:?}", app.document), before);
        assert!(app.try_start_interactive_command("EvaluateUVPt CreatePoint=Yes"));
        app.command_input = "Normalized=Yes".into();
        app.run_command();
        let pending = app.active_command;
        app.command_input = "Normalized=Maybe".into();
        app.run_command();
        assert_eq!(app.active_command, pending);
        assert_eq!(format!("{:?}", app.document), before);
        assert!(app.try_continue_point_input("w1,1,3"));
        assert_eq!(app.document.objects().count(), 2);
        assert!(app.evaluate_uv_session.is_some());
        app.execute_command("Undo");
        assert!(app.active_command.is_none());
        assert!(app.evaluate_uv_session.is_none());
        assert_eq!(app.document.objects().count(), 1);
        let before = format!("{:?}", app.document);
        assert!(app.try_start_interactive_command("EvaluateUVPt"));
        app.cancel_interactive_command(true);
        assert_eq!(format!("{:?}", app.document), before);
    }
}

#[test]
fn evaluate_uv_failed_pick_keeps_point_prompt_and_previous_anchor() {
    let mut app = test_app();
    app.execute_command("SrfPt 0,0,0 4,0,0 4,2,0 0,2,0");
    app.execute_command("SelAll");
    assert!(app.try_start_interactive_command("EvaluateUVPt"));
    let pending = app.active_command;
    app.document.clear_selection();
    let before = format!("{:?}", app.document);
    let previous_last = app.last_point;
    assert!(!app.accept_drafting_point(point(1., 1., 3.)));
    assert_eq!(app.active_command, pending);
    assert_eq!(app.last_point, previous_last);
    assert_eq!(format!("{:?}", app.document), before);
}

#[test]
fn evaluate_uv_session_keeps_prior_markers_on_failure_and_finishes_as_one_undo_step() {
    for cancel in [false, true] {
        let mut app = test_app();
        app.execute_command("SrfPt 0,0,0 4,0,0 4,2,0 0,2,0");
        app.execute_command("SelAll");
        let source = app.document.selected_object_ids().next().unwrap();
        assert!(app.try_start_interactive_command("EvaluateUVPt CreatePoint=Yes"));
        assert!(app.accept_drafting_point(point(1., 1., 3.)));
        app.document.clear_selection();
        let before = format!("{:?}", app.document);
        let anchor = app.last_point;
        assert!(!app.accept_drafting_point(point(2., 1., 3.)));
        assert_eq!(app.last_point, anchor);
        assert_eq!(format!("{:?}", app.document), before);
        app.document
            .select_object(source, viboceros_document::SelectionMode::Replace)
            .unwrap();
        assert!(app.accept_drafting_point(point(3., 1., 3.)));
        assert_eq!(app.document.objects().count(), 3);
        if cancel {
            app.cancel_interactive_command(true);
        } else {
            app.command_input = "  ".into();
            app.run_command();
        }
        assert!(app.active_command.is_none());
        assert!(app.evaluate_uv_session.is_none());
        assert_eq!(app.document.undo_label(), Some("EvaluateUVPt"));
        assert_eq!(app.document.objects().count(), 3);
        app.execute_command("Undo");
        assert_eq!(app.document.objects().count(), 1);
        let before = format!("{:?}", app.document);
        assert!(app.try_start_interactive_command("EvaluateUVPt CreatePoint=No"));
        assert!(app.accept_drafting_point(point(2., 1., 3.)));
        assert!(app.accept_drafting_point(point(3., 1., 3.)));
        app.cancel_interactive_command(true);
        assert_eq!(format!("{:?}", app.document), before);
        app.execute_command("Redo");
        assert_eq!(app.document.objects().count(), 3);
    }
}

#[test]
fn evaluate_uv_preferences_survive_prompt_cancellation_and_invalid_options_do_not_replace_them() {
    let mut app = test_app();
    app.execute_command("SrfPt 0,0,0 4,0,0 4,2,0 0,2,0");
    // Postselection options are remembered even before a surface is accepted.
    app.command_input = "EvaluateUVPt".into();
    app.run_command();
    assert!(app.object_prompt.is_some());
    assert!(app.try_continue_object_prompt("Normalized=Yes CreatePoint=Yes"));
    app.cancel_interactive_command(true);
    app.execute_command("SelAll");
    assert!(app.try_start_interactive_command("EvaluateUVPt"));
    let expected = Some(InteractiveCommand::EvaluateUv {
        options: viboceros_command::EvaluateUvOptions {
            normalized: true,
            create_point: true,
        },
    });
    assert_eq!(app.active_command, expected);
    app.command_input = "Normalized=No CreatePoint=Maybe".into();
    app.run_command();
    assert_eq!(app.active_command, expected);
    app.cancel_interactive_command(true);
    assert!(app.try_start_interactive_command("EvaluateUVPt"));
    assert_eq!(app.active_command, expected);
    assert!(app.try_continue_evaluate_uv("CreatePoint=No"));
    app.cancel_interactive_command(true);
    assert!(app.try_start_interactive_command("EvaluateUVPt"));
    assert_eq!(
        app.active_command,
        Some(InteractiveCommand::EvaluateUv {
            options: viboceros_command::EvaluateUvOptions {
                normalized: true,
                create_point: false
            },
        })
    );
    assert!(app.evaluate_uv_session.is_none());
}
