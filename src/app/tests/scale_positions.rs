use super::*;
use viboceros_command::scale_positions::ScaleMode;

#[test]
fn scale_positions_native_modes_groups_grips_repetition_and_memory_match() {
    super::scale_nu::replay_scale_nu_options(
        include_str!("../../../tools/rhino_oracle/fixtures/scale_positions.json"),
        include_str!("../../../tools/rhino_oracle/observations/scale_positions.json"),
        57,
        "ScalePositions",
        99,
    );
}

#[test]
fn scale_positions_native_source_order_reference_numbers_copy_retry_and_defaults_match() {
    super::scale_nu::replay_scale_nu_options(
        include_str!("../../../tools/rhino_oracle/fixtures/scale_positions_input.json"),
        include_str!("../../../tools/rhino_oracle/observations/scale_positions_input.json"),
        64,
        "ScalePositions",
        64,
    );
}

#[test]
fn scale_positions_preview_keeps_grouped_shapes_independent_and_rejected_input_retryable() {
    let mut app = test_app();
    let enter = |app: &mut VibocerosApp, text: &str| {
        app.command_input = text.into();
        app.run_command();
    };
    enter(&mut app, "Line 2,0,0 4,0,0");
    enter(&mut app, "Line 8,0,0 10,0,0");
    enter(&mut app, "SelAll");
    enter(&mut app, "Group");
    let ids = app.document.selected_object_ids().collect::<Vec<_>>();
    let before = format!("{:?}", app.document);
    enter(&mut app, "ScalePositions");
    enter(&mut app, "Mode");
    enter(&mut app, "2D");
    enter(&mut app, "w0,0,0");
    assert_eq!(
        app.transform_default_hint().as_deref(),
        Some("Mode=2D; Enter accepts the default: 1")
    );
    enter(&mut app, "0");
    assert!(app.active_command.is_some());
    assert_eq!(
        app.commands.scale_mode_default("ScalePositions"),
        Some(ScaleMode::ThreeDimensional)
    );
    enter(&mut app, "w1,0,0");
    let preview = app.affine_preview().unwrap();
    let map = preview
        .definition
        .transform_at(
            preview.frame.unwrap(),
            point(2., 0., 0.),
            app.document.tolerance(),
        )
        .unwrap();
    let layout = preview.rigid_layout.unwrap();
    for (id, x) in ids.into_iter().zip([3., 9.]) {
        assert_eq!(layout.center(id), Some(point(x, 0., 0.)));
        let translation =
            viboceros_command::rigid_transform::rigid_map(layout.center(id).unwrap(), map).unwrap();
        let Geometry::Line(line) = app.document.object(id).unwrap().geometry() else {
            panic!("expected line")
        };
        let posed = line
            .transformed(translation, app.document.tolerance())
            .unwrap();
        assert_eq!(posed.start(), point(line.start().x() + x, 0., 0.));
        assert_eq!(posed.length().unwrap(), line.length().unwrap());
    }
    assert_eq!(format!("{:?}", app.document), before);
    enter(&mut app, "_Cancel");
    assert!(app.active_command.is_none());
    assert_eq!(
        app.commands.scale_mode_default("ScalePositions"),
        Some(ScaleMode::ThreeDimensional)
    );
}

fn enter(app: &mut VibocerosApp, text: &str) {
    app.command_input = text.into();
    app.run_command();
}

#[test]
fn scale_positions_native_cursor_planes_axis_constraints_and_input_history_match() {
    replay_cursor(
        include_str!("../../../tools/rhino_oracle/fixtures/scale_positions_cursor.json"),
        include_str!("../../../tools/rhino_oracle/observations/scale_positions_cursor.json"),
        64,
        64,
    );
}

#[test]
fn scale_positions_native_origin_boundaries_and_enter_defaults_match() {
    replay_cursor(
        include_str!("../../../tools/rhino_oracle/fixtures/scale_positions_origin.json"),
        include_str!("../../../tools/rhino_oracle/observations/scale_positions_origin.json"),
        64,
        62,
    );
}

fn replay_cursor(fixture: &str, observed: &str, count: usize, expected_replays: usize) {
    use super::grip_transform::{compare, geometry, snapshot};
    use crate::viewport::{DraftingInput, ViewportInput};
    use serde_json::{Value, json};
    let fixture: Value = serde_json::from_str(fixture).unwrap();
    let observed: Value = serde_json::from_str(observed).unwrap();
    let operations = fixture["operations"].as_array().unwrap();
    let rows = observed["results"].as_array().unwrap();
    assert_eq!(operations.len(), count);
    assert_eq!(rows.len(), count);
    let mut matched = 0;
    for (op, row) in operations.iter().zip(rows) {
        if op["origin"] == "FloatAbove" {
            // Retain the unstable native boundary inputs as raw diagnostics.
            // They are excluded explicitly, independent of output geometry.
            continue;
        }
        matched += 1;
        let label = op["id"].as_str().unwrap();
        assert_eq!(row["id"], label);
        let native = &row["value"];
        let mut app = test_app();
        app.active_viewport = 1;
        app.viewports[1] = crate::viewport::clip_tests::captured_view(&native["pending"]["camera"]);
        enter(&mut app, "Point 1,1,1");
        enter(&mut app, "SelAll");
        // Establish remembered state through the same public reference command.
        // The native prompt's rounded factor is never injected into the registry.
        for token in native["seed_macro"].as_str().unwrap().split_whitespace() {
            enter(&mut app, token);
        }
        enter(&mut app, "SelAll");
        enter(&mut app, "Delete");
        app.document.clear_history().unwrap();
        app.document
            .begin_transaction("ScalePositions cursor source")
            .unwrap();
        let source_geometry = if op["source"] == "point" {
            Geometry::Point(point(2., 3., 4.))
        } else {
            geometry(op, &native["before"][0])
        };
        let source = app.document.add_geometry(source_geometry).unwrap();
        app.document.commit_transaction().unwrap();
        app.document
            .select_objects_direct([source], SelectionMode::Replace)
            .unwrap();
        let objects = |app: &VibocerosApp| {
            let mut value = snapshot(app, source, None);
            for row in value.as_array_mut().unwrap() {
                row.as_object_mut()
                    .unwrap()
                    .retain(|key, _| matches!(key.as_str(), "point" | "curve" | "selected"));
            }
            value
        };
        compare(&objects(&app), &native["before"], label);
        let tokens = native["recipe"]["macro"]
            .as_str()
            .unwrap()
            .split_whitespace()
            .collect::<Vec<_>>();
        for token in tokens.iter().take_while(|&&token| token != "_Pause") {
            enter(&mut app, token);
        }
        assert!(
            app.active_command.is_some(),
            "{label}: {:?}",
            app.command_log
        );
        let before = app.document.objects().cloned().collect::<Vec<_>>();
        let scalar = app.commands.transform_scalar_default("ScalePositions");
        let mode = app.commands.scale_mode_default("ScalePositions");
        let copy = app.commands.copy_default("ScalePositions");
        let preview = app.affine_preview().unwrap();
        let center = preview.rigid_layout.unwrap().center(source).unwrap();
        compare(
            &json!(center.to_array()),
            &native["bounds"]["center"],
            &format!("cached preview center {label}"),
        );
        let cursor = crate::viewport::affine_preview::tests::captured_cursor(
            &app.document,
            &native["pending"],
            ViewportInput {
                drafting: DraftingInput {
                    active: true,
                    osnap: viboceros_drafting::ObjectSnapModes::NONE,
                    ..Default::default()
                },
                affine_preview: Some(preview),
                ..Default::default()
            },
        );
        if op["view"] == "Front" && op["input"] != "Reference" {
            assert!(
                cursor.is_none(),
                "{label}: edge-on free plane should reject this click"
            );
        } else {
            let (_, map) = cursor.unwrap();
            app.update_affine_preview(Some(map));
        }
        assert_eq!(
            app.document.objects().cloned().collect::<Vec<_>>(),
            before,
            "{label}"
        );
        assert_eq!(
            app.commands.transform_scalar_default("ScalePositions"),
            scalar,
            "{label}"
        );
        assert_eq!(
            app.commands.scale_mode_default("ScalePositions"),
            mode,
            "{label}"
        );
        assert_eq!(app.commands.copy_default("ScalePositions"), copy, "{label}");
        compare(&objects(&app), &native["pending"]["objects"], label);
        match op["finish"].as_str().unwrap() {
            "ClickCancel" => {
                compare(
                    &objects(&app),
                    &native["pending"]["rejected_click"]["objects"],
                    label,
                );
                enter(&mut app, "_Cancel");
            }
            "Click" => {
                let (destination, map) = cursor.unwrap();
                let delta = center
                    .vector_to(map.transform_point(center).unwrap())
                    .unwrap()
                    .to_array();
                let mut preview_objects = objects(&app);
                let output = &mut preview_objects[0];
                let translate = |p: &mut Value| {
                    for axis in 0..3 {
                        p[axis] = json!(p[axis].as_f64().unwrap() + delta[axis]);
                    }
                };
                if let Some(p) = output.get_mut("point") {
                    translate(p);
                } else {
                    for p in output["curve"]["control_points"].as_array_mut().unwrap() {
                        translate(&mut p["point"]);
                    }
                }
                assert!(
                    app.accept_filtered_drafting_point(destination, false),
                    "{label}: {:?}",
                    app.command_log
                );
                let index = usize::from(
                    op["copy"].as_bool().unwrap() && native["after"].as_array().unwrap().len() > 1,
                );
                if index == 1 {
                    preview_objects[0]["selected"] = json!(false);
                }
                compare(
                    &preview_objects[0],
                    &native["after"][index],
                    &format!("preview {label}"),
                );
            }
            "Typed" => enter(&mut app, native["recipe"]["typed_target"].as_str().unwrap()),
            _ => unreachable!(),
        }
        if op["copy"] == true {
            enter(&mut app, "_Enter");
        }
        assert!(
            app.active_command.is_none(),
            "{label}: {:?}",
            app.command_log
        );
        compare(
            &objects(&app),
            &native["after"],
            &format!("EndCommand {label}"),
        );
        compare(
            &objects(&app),
            &native["after_script"],
            &format!("post-macro {label}"),
        );
        let check_preferences = |app: &VibocerosApp| {
            assert_eq!(
                app.commands
                    .scale_mode_default("ScalePositions")
                    .unwrap()
                    .name(),
                native["preferences"]["mode"].as_str().unwrap(),
                "{label}"
            );
            assert_eq!(
                app.commands.copy_default("ScalePositions"),
                native["preferences"]["copy"].as_bool(),
                "{label}"
            );
            assert!(
                (app.commands
                    .transform_scalar_default("ScalePositions")
                    .unwrap()
                    - native["preferences"]["factor"].as_f64().unwrap())
                .abs()
                    <= 5e-6,
                "{label}: factor {:?} vs {:?}",
                app.commands.transform_scalar_default("ScalePositions"),
                native["preferences"]
            );
        };
        check_preferences(&app);
        assert_eq!(
            app.document.undo_label(),
            Some(if native["undo"].as_array().unwrap().is_empty() {
                "ScalePositions cursor source"
            } else {
                "ScalePositions"
            }),
            "{label}"
        );
        enter(&mut app, "Undo");
        compare(&objects(&app), &native["undo"], &format!("Undo {label}"));
        check_preferences(&app);
        enter(&mut app, "Redo");
        compare(&objects(&app), &native["redo"], &format!("Redo {label}"));
        check_preferences(&app);
    }
    assert_eq!(matched, expected_replays);
}
