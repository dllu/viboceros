use super::*;
use serde_json::Value;
use viboceros_command::set_view::SetViewPrompt;
use viboceros_drafting::{ObjectSnapKind, ObjectSnapModes};
use viboceros_geometry::{Frame3, Vector3};
use viboceros_io::ThreeDmProjection;

fn enter(app: &mut VibocerosApp, text: &str) {
    app.command_input = text.into();
    app.run_command();
}

fn position(value: &Value) -> Point3 {
    Point3::try_from(serde_json::from_value::<[f64; 3]>(value.clone()).unwrap()).unwrap()
}

fn vector(value: &Value) -> Vector3 {
    Vector3::try_from(serde_json::from_value::<[f64; 3]>(value.clone()).unwrap()).unwrap()
}

#[test]
fn typed_set_view_workflows_match_all_saved_rhino_option_transitions() {
    let mut checked = 0;
    for (request, observation) in [
        (
            include_str!("../../../tools/rhino_oracle/fixtures/set_view_prompt.json"),
            include_str!("../../../tools/rhino_oracle/observations/set_view_prompt.json"),
        ),
        (
            include_str!("../../../tools/rhino_oracle/fixtures/set_view_nested_cplane.json"),
            include_str!("../../../tools/rhino_oracle/observations/set_view_nested_cplane.json"),
        ),
    ] {
        let request: Value = serde_json::from_str(request).unwrap();
        let observation: Value = serde_json::from_str(observation).unwrap();
        let operations = request["operations"].as_array().unwrap();
        let results = observation["results"].as_array().unwrap();
        assert_eq!(operations.len(), results.len());
        for (operation, result) in operations.iter().zip(results) {
            assert_eq!(operation["id"], result["id"]);
            let before = &result["value"]["before"];
            let mut app = test_app();
            let mut source = Viewport::named_view_to_3dm(
                app.viewports[1].named_view_snapshot(),
                "Rhino prompt input".into(),
            )
            .unwrap();
            source.camera_location = position(&before["camera_location"]);
            source.camera_direction = vector(&before["camera_direction"]);
            source.camera_up = vector(&before["camera_up"]);
            source.target = Some(position(&before["camera_target"]));
            source.construction_plane = Frame3::try_from_directions(
                position(&before["cplane_origin"]),
                vector(&before["cplane_x"]),
                vector(&before["cplane_y"]),
                app.document.tolerance(),
            )
            .unwrap();
            app.active_viewport = 1;
            app.viewports[1].restore_named_view(Viewport::named_view_from_3dm(&source).unwrap());
            let steps = operation["steps"].as_array().unwrap();
            let rows = result["value"]["steps"].as_array().unwrap();
            assert_eq!(steps.len(), rows.len());
            for (tokens, row) in steps.iter().zip(rows) {
                assert_eq!(tokens, &row["tokens"]);
                enter(&mut app, "SetView");
                for token in tokens.as_array().unwrap() {
                    enter(&mut app, token.as_str().unwrap());
                }
                assert!(app.set_view_prompt.is_none(), "{tokens}");
                assert!(app.plane_prompt.is_none(), "{tokens}");
                let actual = Viewport::named_view_to_3dm(
                    app.viewports[1].named_view_snapshot(),
                    "Actual".into(),
                )
                .unwrap();
                let expected = &row["state"];
                assert_eq!(
                    actual.projection,
                    if expected["two_point_perspective"].as_bool().unwrap() {
                        ThreeDmProjection::TwoPointPerspective
                    } else if expected["perspective"].as_bool().unwrap() {
                        ThreeDmProjection::Perspective
                    } else {
                        ThreeDmProjection::Parallel
                    },
                    "{tokens}"
                );
                // This probe records option transitions, without input frusta.
                // Optical framing and clipping have separate camera probes.
                for (value, field) in [
                    (actual.camera_direction.to_array(), "camera_direction"),
                    (actual.camera_up.to_array(), "camera_up"),
                    (actual.target.unwrap().to_array(), "camera_target"),
                    (
                        actual.construction_plane.origin().to_array(),
                        "cplane_origin",
                    ),
                    (
                        actual.construction_plane.x_axis().as_vector().to_array(),
                        "cplane_x",
                    ),
                    (
                        actual.construction_plane.y_axis().as_vector().to_array(),
                        "cplane_y",
                    ),
                ] {
                    let expected: [f64; 3] =
                        serde_json::from_value(expected[field].clone()).unwrap();
                    for (a, b) in value.into_iter().zip(expected) {
                        assert!(
                            (a - b).abs() < 2e-12,
                            "{} {tokens} {field}: {a} vs {b}",
                            operation["id"]
                        );
                    }
                }
                assert_eq!(app.document.objects().count(), 0);
                checked += 1;
            }
        }
    }
    assert_eq!(checked, 33);
}

#[test]
fn set_view_suspends_model_point_input_and_preserves_selection_and_history() {
    let mut app = test_app();
    for input in [
        "Point 8,9,10",
        "Point 11,12,13",
        "Undo",
        "SelAll",
        "Line",
        "0",
        "5",
        ".xy",
        "End",
    ] {
        enter(&mut app, input);
    }
    let pending = app.active_command;
    let last_point = app.last_point;
    let drafting_plane = app.drafting_plane;
    let constraint = app.point_constraint;
    let filter = app.point_filter;
    let snaps = app.snaps.model_override;
    let undo = app.document.undo_label().map(str::to_owned);
    let redo = app.document.redo_label().map(str::to_owned);
    let objects = app.document.objects().cloned().collect::<Vec<_>>();
    let camera = app.viewports[0].camera_snapshot();
    enter(&mut app, "SetView");
    for invalid in ["Top", "w100,200,300", "Delete", "Line"] {
        enter(&mut app, invalid);
        assert_eq!(
            app.set_view_prompt.as_ref().unwrap().prompt,
            SetViewPrompt::CoordinateSystem
        );
    }
    assert!(!app.accept_filtered_drafting_point(point(100., 200., 300.), false));
    assert!(!app.accept_drafting_point(point(100., 200., 300.)));
    assert_eq!(app.viewport_object_filter(), None);
    enter(&mut app, "World");
    enter(&mut app, "Bottom");
    assert!(app.set_view_prompt.is_none());
    assert_eq!(app.viewports[0].kind(), ViewKind::Bottom);
    assert_eq!(app.active_command, pending);
    assert_eq!(app.last_point, last_point);
    assert_eq!(app.drafting_plane, drafting_plane);
    assert_eq!(app.point_constraint, constraint);
    assert_eq!(app.point_filter, filter);
    assert_eq!(app.snaps.model_override, snaps);
    assert_eq!(app.document.undo_label(), undo.as_deref());
    assert_eq!(app.document.redo_label(), redo.as_deref());
    assert_eq!(app.document.objects().cloned().collect::<Vec<_>>(), objects);
    enter(&mut app, "UndoView");
    assert_eq!(app.viewports[0].camera_snapshot(), camera);
    enter(&mut app, "RedoView");
    enter(&mut app, "w3,4,9");
    enter(&mut app, "w0,0,0");
    assert!(app.active_command.is_none());
    assert_eq!(app.document.objects().count(), 2);
}

#[test]
fn canceling_any_option_stage_retains_the_model_and_camera() {
    for partial in ["SetView", "'_SetView _World", "SetView CPlane"] {
        for cancel in ["", "!", "Escape"] {
            let mut app = test_app();
            enter(&mut app, "Line");
            enter(&mut app, "0");
            let pending = app.active_command;
            let camera = app.viewports[0].camera_snapshot();
            enter(&mut app, partial);
            if cancel == "Escape" {
                app.cancel_current_prompt_or_selection();
            } else {
                enter(&mut app, cancel);
            }
            assert!(app.set_view_prompt.is_none());
            assert_eq!(app.active_command, pending);
            assert_eq!(app.viewports[0].camera_snapshot(), camera);
            enter(&mut app, "w1,2,3");
            assert_eq!(app.document.objects().count(), 1);
        }
    }
}

#[test]
fn set_view_restores_a_paused_cplane_prompt_and_its_one_pick_snap() {
    for finish in ["Top", "", "!"] {
        let mut app = test_app();
        for input in ["Line", "0", "Mid", "CPlane", "End", "SetView World"] {
            enter(&mut app, input);
        }
        let pending = app.active_command;
        assert!(app.plane_prompt.is_none());
        assert_eq!(app.snaps.plane_override, None);
        enter(&mut app, finish);
        assert!(app.set_view_prompt.is_none());
        assert!(app.plane_prompt.is_some());
        assert_eq!(
            app.snaps.plane_override,
            Some(ObjectSnapModes::only(ObjectSnapKind::End))
        );
        assert_eq!(
            app.snaps.model_override,
            Some(ObjectSnapModes::only(ObjectSnapKind::Mid))
        );
        enter(&mut app, "w4,5,6");
        assert!(app.plane_prompt.is_none());
        assert_eq!(
            app.viewports[0].construction_plane().origin(),
            point(4., 5., 6.)
        );
        assert_eq!(app.active_command, pending);
        assert_eq!(app.document.objects().count(), 0);
        assert_eq!(app.last_point, Some(point(0., 0., 0.)));
        enter(&mut app, "w1,2,3");
        assert_eq!(app.document.objects().count(), 1);
    }
}

#[test]
fn nested_cplane_and_zoom_prompts_return_to_set_view_before_the_model() {
    let mut app = test_app();
    for input in ["Line", "0", "SetView World", "CPlane"] {
        enter(&mut app, input);
    }
    let pending = app.active_command;
    assert!(app.plane_prompt.is_some());
    assert!(!app.set_view_options_active());
    app.cancel_current_prompt_or_selection();
    assert!(app.set_view_options_active());
    enter(&mut app, "Zoom Factor");
    assert!(!app.set_view_options_active());
    enter(&mut app, "2");
    assert!(app.set_view_options_active());
    assert_eq!(app.active_command, pending);
    enter(&mut app, "'_SetView _World _Right");
    assert!(app.set_view_prompt.is_none());
    assert_eq!(app.viewports[0].kind(), ViewKind::Right);
    enter(&mut app, "w1,2,3");
    assert_eq!(app.document.objects().count(), 1);
}

#[test]
fn set_view_pauses_and_resumes_a_prior_zoom_factor_on_its_original_viewport() {
    let mut app = test_app();
    super::interface::layout_viewports(&egui::Context::default(), &mut app);
    for input in ["Line", "0", "Zoom Factor", "SetView"] {
        enter(&mut app, input);
    }
    assert_eq!(app.zoom_factor_pending, None);
    let top = app.viewports[0].camera_snapshot();
    let perspective = app.viewports[1].camera_snapshot();
    app.active_viewport = 1;
    enter(&mut app, "");
    assert_eq!(app.zoom_factor_pending, Some(0));
    enter(&mut app, "2");
    assert_eq!(app.zoom_factor_pending, None);
    assert_ne!(app.viewports[0].camera_snapshot(), top);
    assert_eq!(app.viewports[1].camera_snapshot(), perspective);
    assert!(app.active_command.is_some());
    assert_eq!(app.document.objects().count(), 0);
}

#[test]
fn set_view_suspends_a_copy_cplane_source_pick_before_normal_input_can_cancel_it() {
    let mut app = test_app();
    for input in ["Line", "0", "CopyCPlaneSettingsToAll", "SetView World"] {
        enter(&mut app, input);
    }
    assert!(app.copy_cplane_source.is_none());
    assert!(app.set_view_options_active());
    enter(&mut app, "Back");
    assert!(app.set_view_prompt.is_none());
    assert_eq!(
        app.copy_cplane_source,
        Some(crate::app::construction_plane::CopyCPlaneKind::Settings)
    );
    let planes = app
        .viewports
        .iter()
        .map(Viewport::construction_plane)
        .collect::<Vec<_>>();
    enter(&mut app, "");
    assert!(app.copy_cplane_source.is_none());
    for (viewport, plane) in app.viewports.iter().zip(planes) {
        assert_eq!(viewport.grid_settings(), app.viewports[0].grid_settings());
        assert_eq!(viewport.construction_plane(), plane);
    }
    assert!(app.active_command.is_some());
    enter(&mut app, "w1,2,3");
    assert_eq!(app.document.objects().count(), 1);
}

#[test]
fn choosing_a_view_restores_prior_zoom_target_window_and_snap_size_prompts() {
    for input in ["Zoom Target", "Zoom Window", "SnapSize"] {
        let mut app = test_app();
        enter(&mut app, "Line");
        enter(&mut app, "0");
        enter(&mut app, input);
        enter(&mut app, "SetView CPlane");
        assert!(app.zoom_target.is_none());
        assert!(!app.zoom_window_pending);
        assert!(app.snap_size_pending.is_none());
        enter(&mut app, "Front");
        assert!(app.set_view_prompt.is_none());
        match input {
            "Zoom Target" => assert!(matches!(app.zoom_target, Some(ZoomTargetState::PickTarget))),
            "Zoom Window" => assert!(app.zoom_window_pending),
            "SnapSize" => assert_eq!(
                app.snap_size_pending,
                Some((viboceros_command::interface::ViewportTarget::Active, 0))
            ),
            _ => unreachable!(),
        }
        app.cancel_current_prompt_or_selection();
        assert!(app.active_command.is_some());
        enter(&mut app, "w1,2,3");
        assert_eq!(app.document.objects().count(), 1);
    }
}

#[test]
fn closing_viewports_remaps_or_cancels_a_suspended_cplane_pick() {
    for close_original in [false, true] {
        let mut app = test_app();
        app.active_viewport = 3;
        for input in ["Line", "0", "CPlane", "End", "SetView World"] {
            enter(&mut app, input);
        }
        if !close_original {
            app.active_viewport = 0;
        }
        enter(&mut app, "CloseViewport");
        assert_eq!(app.viewports.len(), 3);
        enter(&mut app, "");
        if close_original {
            assert!(app.plane_prompt.is_none());
            assert!(app.snaps.plane_override.is_none());
            enter(&mut app, "w1,2,3");
            assert_eq!(app.document.objects().count(), 1);
        } else {
            assert_eq!(app.plane_prompt.as_ref().unwrap().viewport, 2);
            enter(&mut app, "w4,5,6");
            assert_eq!(
                app.viewports[2].construction_plane().origin(),
                point(4., 5., 6.)
            );
            assert_eq!(app.document.objects().count(), 0);
        }
    }
}

#[test]
fn escape_cancels_the_view_prompt_before_an_underlying_selection_prompt() {
    for selection in [
        "SelWindow",
        "SelCircular",
        "SelBoundary",
        "SelFence",
        "Lasso",
        "ShowEnds",
    ] {
        let mut app = test_app();
        enter(&mut app, selection);
        assert!(
            match selection {
                "SelWindow" => app.selection_window_override.is_some(),
                "SelCircular" => app.circular_selection.is_some(),
                "SelBoundary" => app.boundary_selection.is_some(),
                "SelFence" => app.fence_selection.is_some(),
                "Lasso" => app.lasso_selection.is_some(),
                "ShowEnds" => app.end_analysis_pick.is_some(),
                _ => unreachable!(),
            },
            "{selection} should open a selection prompt"
        );
        enter(&mut app, "SetView World");
        enter(&mut app, "CPlane");
        app.cancel_current_prompt_or_selection();
        assert!(app.plane_prompt.is_none());
        assert!(app.set_view_options_active());
        app.cancel_current_prompt_or_selection();
        assert!(app.set_view_prompt.is_none());
        assert!(
            match selection {
                "SelWindow" => app.selection_window_override.is_some(),
                "SelCircular" => app.circular_selection.is_some(),
                "SelBoundary" => app.boundary_selection.is_some(),
                "SelFence" => app.fence_selection.is_some(),
                "Lasso" => app.lasso_selection.is_some(),
                "ShowEnds" => app.end_analysis_pick.is_some(),
                _ => unreachable!(),
            },
            "{selection} should still be pending after SetView"
        );
    }
}

#[test]
fn child_cplane_options_do_not_answer_suspended_selection_prompts() {
    for selection in ["SelWindow", "SelCircular", "ShowEnds"] {
        let mut app = test_app();
        for input in [
            selection,
            "SetView World",
            "CPlane",
            "World",
            "",
            "CPlane",
            "World",
            "Top",
            "Bottom",
        ] {
            enter(&mut app, input);
        }
        assert!(app.set_view_prompt.is_none());
        assert!(app.plane_prompt.is_none());
        assert!(
            match selection {
                "SelWindow" => app.selection_window_override.is_some(),
                "SelCircular" => app.circular_selection.is_some(),
                "ShowEnds" => app.end_analysis_pick.is_some(),
                _ => unreachable!(),
            },
            "nested options should preserve {selection}"
        );
        assert_eq!(app.document.objects().count(), 0);
    }
}

fn choices_frame(
    context: &egui::Context,
    app: &mut VibocerosApp,
    events: Vec<egui::Event>,
) -> egui::FullOutput {
    context.run_ui(
        egui::RawInput {
            screen_rect: Some(egui::Rect::from_min_size(
                egui::Pos2::ZERO,
                egui::vec2(800., 600.),
            )),
            events,
            ..Default::default()
        },
        |ui| app.show_set_view_choices(ui),
    )
}

#[test]
fn option_buttons_complete_the_same_view_transition_as_typed_choices() {
    let mut app = test_app();
    enter(&mut app, "Line");
    enter(&mut app, "0");
    enter(&mut app, "SetView");
    let context = egui::Context::default();
    for choice in ["World", "Bottom"] {
        let output = choices_frame(&context, &mut app, vec![]);
        let position = output
            .shapes
            .iter()
            .find_map(|shape| match &shape.shape {
                egui::Shape::Text(text) if text.galley.job.text == choice => {
                    Some(text.pos + text.galley.rect.center().to_vec2())
                }
                _ => None,
            })
            .expect("the requested option button should be drawn");
        output.drop_without_applying_deltas();
        for pressed in [true, false] {
            choices_frame(
                &context,
                &mut app,
                vec![
                    egui::Event::PointerMoved(position),
                    egui::Event::PointerButton {
                        pos: position,
                        button: egui::PointerButton::Primary,
                        pressed,
                        modifiers: egui::Modifiers::NONE,
                    },
                ],
            )
            .drop_without_applying_deltas();
        }
    }
    assert!(app.set_view_prompt.is_none());
    assert_eq!(app.viewports[0].kind(), ViewKind::Bottom);
    enter(&mut app, "w1,2,3");
    assert_eq!(app.document.objects().count(), 1);
}
