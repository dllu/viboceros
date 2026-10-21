use super::*;
use crate::app::tests::test_app;
#[path = "../../../crates/viboceros-command/tests/support/face_create.rs"]
mod native;

fn enter(app: &mut VibocerosApp, text: &str) {
    app.command_input = text.into();
    app.run_command();
}

#[test]
fn face_point_getters_match_native_geometry_ngons_and_identity_through_history() {
    let mut gaps = Vec::new();
    for row in native::rows() {
        let value = &row["value"];
        let scene = native::setup(value);
        let mut app = test_app();
        app.document = scene.document;
        enter(&mut app, &native::header(value));
        assert!(app.face_create_session.is_some());
        for action in native::actions(value) {
            enter(&mut app, &action);
        }
        if app.face_create_session.is_some() {
            enter(&mut app, "");
        }
        assert!(
            app.face_create_session.is_none(),
            "{} {:?}",
            row["id"],
            app.command_log
        );
        assert!(app.active_command.is_none());
        let check = native::check(
            &app.document,
            scene.source,
            scene.original.as_ref(),
            value,
            "after_script",
        );
        if check["qualified"] != true {
            gaps.push(row["id"].clone());
        }
        assert_eq!(app.document.undo_label(), Some("3DFace"));
        app.document.undo().unwrap();
        assert_eq!(
            native::check(
                &app.document,
                scene.source,
                scene.original.as_ref(),
                value,
                "undo"
            )["qualified"],
            true
        );
        app.document.redo().unwrap();
        assert_eq!(
            native::check(
                &app.document,
                scene.source,
                scene.original.as_ref(),
                value,
                "redo"
            ),
            check
        );
        println!(
            "FACE_CREATE_GETTER {}",
            serde_json::json!({"id":row["id"],"qualified":check["qualified"],"check":check,"grouped_history":true})
        );
    }
    assert!(gaps.is_empty(), "{gaps:?}");
}

#[test]
fn complete_script_and_partial_cancel_keep_redo_and_preview_drafts_separate() {
    let rows = native::rows();
    let value = &rows[0]["value"];
    let scene = native::setup(value);
    let mut app = test_app();
    app.document = scene.document;
    app.commands
        .execute(&mut app.document, "Point 9,9,9")
        .unwrap();
    app.document.undo().unwrap();
    let redo = app.document.redo_label().map(str::to_owned);
    enter(&mut app, "3DFace");
    enter(&mut app, "0,0,0");
    enter(&mut app, "4,0,0");
    app.update_face_create_hover(Point3::try_new(4., 3., 0.).unwrap());
    assert!(
        !app.face_create_session
            .as_ref()
            .unwrap()
            .preview_curves()
            .is_empty()
    );
    assert_eq!(app.document.objects().count(), 0);
    assert_eq!(app.document.redo_label(), redo.as_deref());
    app.cancel_current_prompt_or_selection();
    assert!(
        app.face_create_session
            .as_ref()
            .unwrap()
            .draft
            .as_ref()
            .unwrap()
            .points
            .is_empty()
    );
    app.cancel_current_prompt_or_selection();
    assert!(app.face_create_session.is_none());
    assert_eq!(app.document.redo_label(), redo.as_deref());
    enter(&mut app, &native::command(value));
    assert!(app.face_create_session.is_none());
    assert_eq!(app.document.objects().count(), 1);
    assert_eq!(
        native::check(&app.document, None, None, value, "after_script")["qualified"],
        true
    );
}

#[test]
fn appending_from_a_boundary_edge_uses_reversed_face_winding_and_one_group() {
    let rows = native::rows();
    let value = &rows.iter().find(|r| r["id"] == "append_flat").unwrap()["value"];
    let scene = native::setup(value);
    let id = scene.source.unwrap();
    let mut app = test_app();
    app.document = scene.document;
    let Geometry::Mesh(mesh) = app.document.object(id).unwrap().geometry() else {
        panic!("mesh")
    };
    let edge = mesh
        .diagnostic_edges()
        .iter()
        .find(|e| e.points.iter().all(|p| p.x() == 4.))
        .unwrap()
        .index;
    enter(&mut app, "3DFace Output=Mesh FromEdge=Yes Mode=SingleFace");
    assert!(app.picking_face_edge());
    assert!(app.pick_face_edge_click(&[crate::viewport::EdgePick { object: id, edge }]));
    enter(&mut app, "7,0,0");
    enter(&mut app, "7,3,0");
    assert!(app.face_create_session.is_none());
    assert_eq!(
        native::check(
            &app.document,
            scene.source,
            scene.original.as_ref(),
            value,
            "after_script"
        )["qualified"],
        true
    );
    app.document.undo().unwrap();
    assert_eq!(
        native::check(
            &app.document,
            scene.source,
            scene.original.as_ref(),
            value,
            "undo"
        )["qualified"],
        true
    );
}

#[test]
fn stale_source_or_history_discards_the_draft_without_overwriting_new_work() {
    let scene = native::setup(&native::rows()[0]["value"]);
    let mut app = test_app();
    app.document = scene.document;
    enter(&mut app, "3DFace");
    enter(&mut app, "0,0,0");
    app.commands
        .execute(&mut app.document, "Point 5,5,5")
        .unwrap();
    enter(&mut app, "4,0,0");
    assert!(app.face_create_session.is_none());
    assert_eq!(app.document.objects().count(), 1);
    assert_eq!(app.document.undo_label(), Some("Point"));
}

#[test]
fn changing_options_preserves_corners_and_append_id_selects_the_requested_mesh() {
    let rows = native::rows();
    let value = &rows.iter().find(|r| r["id"] == "append_flat").unwrap()["value"];
    let scene = native::setup(value);
    let id = scene.source.unwrap();
    let mut app = test_app();
    app.document = scene.document;
    app.document.clear_selection();
    enter(&mut app, "3DFace");
    enter(&mut app, &format!("Append={id}"));
    assert_eq!(app.face_create_session.as_ref().unwrap().target, Some(id));
    enter(&mut app, "4,0,0");
    enter(&mut app, "WeldAngle=0");
    assert_eq!(
        app.face_create_session
            .as_ref()
            .unwrap()
            .draft
            .as_ref()
            .unwrap()
            .points
            .len(),
        1
    );
    enter(&mut app, "FromEdge=Yes");
    assert!(
        !app.face_create_session
            .as_ref()
            .unwrap()
            .preferences
            .from_edge
    );
    assert_eq!(
        app.face_create_session
            .as_ref()
            .unwrap()
            .draft
            .as_ref()
            .unwrap()
            .points
            .len(),
        1
    );
    for point in ["7,0,0", "7,3,0", "4,3,0"] {
        enter(&mut app, point);
    }
    assert!(app.face_create_session.is_none());
    assert_eq!(
        native::check(
            &app.document,
            Some(id),
            scene.original.as_ref(),
            value,
            "after_script"
        )["qualified"],
        true
    );
}

#[test]
fn physical_native_edge_and_escape_controls_match_document_and_history() {
    for row in native::controls() {
        let value = &row["value"];
        let scene = native::setup(value);
        let mut app = test_app();
        app.document = scene.document;
        app.commands
            .execute(&mut app.document, "Point 99,99,99")
            .unwrap();
        app.document.undo().unwrap();
        if let Some(id) = scene.source {
            app.document
                .select_objects_direct([id], viboceros_document::SelectionMode::Replace)
                .unwrap();
        }
        let from_edge = value["input"]["edge_pick"].is_array();
        let header = if from_edge {
            native::header(value).replace("FromEdge=No", "FromEdge=Yes")
        } else {
            native::header(value)
        };
        enter(&mut app, &header);
        if from_edge {
            let id = scene.source.unwrap();
            let Geometry::Mesh(mesh) = app.document.object(id).unwrap().geometry() else {
                panic!("mesh")
            };
            let edge = mesh
                .diagnostic_edges()
                .iter()
                .find(|e| e.points.iter().all(|p| p.x() == 4.))
                .unwrap()
                .index;
            assert!(app.pick_face_edge_click(&[crate::viewport::EdgePick { object: id, edge }]));
        }
        for action in native::actions(value) {
            enter(&mut app, &action);
        }
        if value["input"]["cancel"] == true {
            for _ in 0..3 {
                if app.face_create_session.is_none() {
                    break;
                }
                app.cancel_current_prompt_or_selection();
            }
        } else if app.face_create_session.is_some() {
            enter(&mut app, "");
        }
        assert!(
            app.face_create_session.is_none(),
            "{} {:?}",
            row["id"],
            app.command_log
        );
        let check = native::check(
            &app.document,
            scene.source,
            scene.original.as_ref(),
            value,
            "after_script",
        );
        assert_eq!(check["qualified"], true, "{} {check}", row["id"]);
        if app.document.undo_label().is_some() {
            assert_eq!(app.document.undo_label(), Some("3DFace"));
            app.document.undo().unwrap();
            assert_eq!(
                native::check(
                    &app.document,
                    scene.source,
                    scene.original.as_ref(),
                    value,
                    "undo"
                )["qualified"],
                true
            );
            app.document.redo().unwrap();
            assert_eq!(
                native::check(
                    &app.document,
                    scene.source,
                    scene.original.as_ref(),
                    value,
                    "redo"
                ),
                check
            );
        } else {
            assert_eq!(app.document.redo_label(), Some("Point"));
        }
        println!(
            "FACE_CREATE_CONTROLS {}",
            serde_json::json!({"id":row["id"],"qualified":true,"check":check,"grouped_history":true})
        );
    }
}

#[test]
fn in_plane_hover_matches_the_projected_corner_before_admission() {
    let mut app = test_app();
    enter(&mut app, "3DFace InPlane=Yes");
    for point in ["0,0,0", "4,0,0", "4,3,2"] {
        enter(&mut app, point);
    }
    app.update_face_create_hover(Point3::try_new(0., 3., 0.).unwrap());
    let expected = Point3::try_new(0., 27. / 13., 18. / 13.).unwrap();
    let preview = app.face_create_session.as_ref().unwrap().preview_curves();
    assert!(
        preview[0]
            .control_points()
            .iter()
            .any(|p| p.point().distance_to(expected).unwrap() < 1e-12)
    );
    assert_eq!(app.document.objects().count(), 0);
    enter(&mut app, "0,3,0");
    let Geometry::Mesh(mesh) = app.document.objects().next().unwrap().geometry() else {
        panic!("mesh")
    };
    assert!(mesh.vertices()[3].distance_to(expected).unwrap() < 1e-12);
}

#[test]
fn edge_chain_getters_replay_physical_native_inputs_through_screen_space_picks() {
    let mut gaps = vec![];
    for row in native::drafts() {
        let value = &row["value"];
        let scene = native::setup(value);
        let mut app = test_app();
        app.document = scene.document;
        enter(&mut app, &native::draft_header(value));
        let mut failures = vec![];
        for action in value["input"]["points"].as_array().unwrap() {
            if let Some(pick) = action.get("edge_pick") {
                let pick =
                    Point3::try_from(serde_json::from_value::<[f64; 3]>(pick.clone()).unwrap())
                        .unwrap();
                assert!(
                    app.picking_face_edge(),
                    "{} {:?}",
                    row["id"],
                    app.command_log
                );
                let view = crate::viewport::Viewport::new(crate::viewport::ViewKind::Top);
                let rect = egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(800., 600.));
                let pointer = view.project(pick, rect).unwrap();
                let picks = view.pick_mesh_boundary_edges(
                    pointer,
                    rect,
                    app.face_boundary_edges().unwrap(),
                );
                assert!(!picks.is_empty(), "{} missing actual edge hit", row["id"]);
                let output = crate::viewport::ViewportOutput {
                    mesh_boundary_edge_click: Some(picks),
                    ..Default::default()
                };
                assert!(app.handle_viewport_action(output));
            } else if let Some(word) = action.as_str() {
                enter(&mut app, if word == "Enter" { "" } else { word });
            } else {
                let point: [f64; 3] = serde_json::from_value(action.clone()).unwrap();
                let token = point
                    .iter()
                    .map(ToString::to_string)
                    .collect::<Vec<_>>()
                    .join(",");
                enter(&mut app, &token);
            }
            if let Some(log) = app.command_log.back()
                && log.starts_with("Error:")
            {
                failures.push(log.clone());
            }
        }
        if value["input"]["cancel"] == true {
            for _ in 0..4 {
                if app.face_create_session.is_none() {
                    break;
                }
                app.cancel_current_prompt_or_selection();
            }
        } else {
            for _ in 0..2 {
                if app.face_create_session.is_none() {
                    break;
                }
                enter(&mut app, "");
            }
        }
        let accepted_native_tokens = app.face_create_session.is_none();
        if !accepted_native_tokens {
            for _ in 0..3 {
                if app.face_create_session.is_none() {
                    break;
                }
                app.cancel_current_prompt_or_selection();
            }
        }
        assert!(
            app.face_create_session.is_none(),
            "{} {:?}",
            row["id"],
            app.command_log
        );
        let check = native::check(
            &app.document,
            scene.source,
            scene.original.as_ref(),
            value,
            "after_script",
        );
        if check["qualified"] != true {
            gaps.push(row["id"].clone());
        }
        if app.document.undo_label().is_some() {
            app.document.undo().unwrap();
            assert_eq!(
                native::check(
                    &app.document,
                    scene.source,
                    scene.original.as_ref(),
                    value,
                    "undo"
                )["qualified"],
                true
            );
            app.document.redo().unwrap();
            assert_eq!(
                native::check(
                    &app.document,
                    scene.source,
                    scene.original.as_ref(),
                    value,
                    "redo"
                ),
                check
            );
        }
        println!(
            "FACE_DRAFT_GETTER {}",
            serde_json::json!({"id":row["id"],"qualified":check["qualified"],"check":check,"grouped_history":true,"errors":failures,"accepted_native_tokens":accepted_native_tokens})
        );
    }
    assert!(gaps.is_empty(), "{gaps:?}");
    println!("FACE_DRAFT_GAPS {gaps:?}");
}

#[test]
fn a_standalone_mesh_can_start_an_edge_chain_before_document_admission() {
    let mut app = test_app();
    enter(&mut app, "3DFace Mode=MultipleFaces");
    for p in ["0,0,0", "4,0,0", "4,3,0", "0,3,0"] {
        enter(&mut app, p);
    }
    assert_eq!(app.document.objects().count(), 0);
    enter(&mut app, "FromEdge=Yes");
    enter(&mut app, "Chain=Yes");
    assert!(app.picking_face_edge());
    assert!(!app.picking_face_append());
    let view = crate::viewport::Viewport::new(crate::viewport::ViewKind::Top);
    let rect = egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(800., 600.));
    let pointer = view
        .project(Point3::try_new(4., 1.5, 0.).unwrap(), rect)
        .unwrap();
    let picks = view.pick_mesh_boundary_edges(pointer, rect, app.face_boundary_edges().unwrap());
    assert!(app.handle_viewport_action(crate::viewport::ViewportOutput {
        mesh_boundary_edge_click: Some(picks),
        ..Default::default()
    }));
    enter(&mut app, "7,0,0");
    enter(&mut app, "7,3,0");
    assert_eq!(app.document.objects().count(), 0);
    enter(&mut app, "");
    enter(&mut app, "");
    assert!(app.face_create_session.is_none());
    let object = app.document.objects().next().unwrap();
    let Geometry::Mesh(mesh) = object.geometry() else {
        panic!("mesh")
    };
    assert_eq!(mesh.vertices().len(), 6);
    assert_eq!(mesh.faces().len(), 2);
    assert_eq!(app.document.undo_label(), Some("3DFace"));
    app.document.undo().unwrap();
    assert_eq!(app.document.objects().count(), 0);
}

#[test]
fn reversed_edge_hover_uses_the_corner_order_that_will_be_committed() {
    let rows = native::drafts();
    let value = &rows.iter().find(|r| r["id"] == "edge_flip_plain").unwrap()["value"];
    let scene = native::setup(value);
    let mut app = test_app();
    app.document = scene.document;
    enter(&mut app, &native::draft_header(value));
    let draft = app
        .face_create_session
        .as_ref()
        .unwrap()
        .draft
        .as_ref()
        .unwrap();
    let edge = native::edge_index(
        &draft.current_mesh().unwrap().unwrap(),
        &serde_json::json!([4., 1.5, 0.]),
    );
    app.pick_face_edge(edge);
    enter(&mut app, "7,3,0");
    app.update_face_create_hover(Point3::try_new(7., 0., 0.).unwrap());
    let preview = app
        .face_create_session
        .as_ref()
        .unwrap()
        .preview_curves()
        .last()
        .unwrap();
    let native_mesh = &value["after_script"][0]["mesh"];
    let native_face = native_mesh["faces"][0].as_array().unwrap();
    assert_eq!(preview.control_points().len(), 5);
    for (point, index) in preview
        .control_points()
        .iter()
        .zip(native_face.iter().chain(native_face.first()))
    {
        let expected = Point3::try_from(
            serde_json::from_value::<[f64; 3]>(
                native_mesh["vertices"][index.as_u64().unwrap() as usize].clone(),
            )
            .unwrap(),
        )
        .unwrap();
        assert!(point.point().distance_to(expected).unwrap() < 1e-12);
    }
    enter(&mut app, "7,0,0");
    assert_eq!(app.last_point, Some(Point3::try_new(7., 0., 0.).unwrap()));
    assert_eq!(
        native::check(
            &app.document,
            scene.source,
            scene.original.as_ref(),
            value,
            "after_script"
        )["qualified"],
        true
    );
}

#[path = "../../../crates/viboceros-command/tests/support/subd.rs"]
mod subd_native;
#[test]
fn subd_point_getters_keep_native_geometry_and_grouped_history() {
    for row in subd_native::rows() {
        let value = &row["value"];
        let mut app = test_app();
        app.document = subd_native::setup();
        enter(&mut app, &subd_native::header(value));
        for point in subd_native::actions(value) {
            enter(&mut app, &point);
        }
        if app.face_create_session.is_some() {
            enter(&mut app, "");
        }
        assert!(app.face_create_session.is_none(), "{:?}", app.command_log);
        let check = subd_native::check(&app.document, value);
        assert_eq!(app.document.undo_label(), Some("3DFace"));
        app.document.undo().unwrap();
        assert_eq!(app.document.objects().count(), 0);
        app.document.redo().unwrap();
        assert_eq!(subd_native::check(&app.document, value), check);
        println!(
            "SUBD_GETTER {}",
            serde_json::json!({"id":row["id"],"check":check,"grouped_history":true})
        );
    }
}

#[test]
fn subd_output_option_and_cancellation_preserve_the_redo_branch() {
    let mut app = test_app();
    enter(&mut app, "Point 9,9,9");
    app.document.undo().unwrap();
    enter(&mut app, "3DFace");
    enter(&mut app, "Output");
    enter(&mut app, "SubD");
    enter(&mut app, "0,0,0");
    enter(&mut app, "4,0,0");
    assert!(app.cancel_face_corner());
    assert!(app.cancel_face_corner());
    assert!(app.face_create_session.is_none());
    assert_eq!(app.document.objects().count(), 0);
    assert_eq!(app.document.redo_label(), Some("Point"));
    enter(&mut app, "3DFace Output=SubD PolygonType=Triangle");
    enter(&mut app, "0,0,0");
    enter(&mut app, "4,0,0");
    enter(&mut app, "4,3,0");
    assert!(matches!(
        app.document.objects().next().unwrap().geometry(),
        Geometry::SubD(_)
    ));
    assert_eq!(app.document.redo_label(), None);
}

#[path = "../../../crates/viboceros-command/tests/support/subd_append.rs"]
mod append_native;
#[test]
fn subd_append_getters_match_native_objects_boundary_clicks_and_history() {
    for row in append_native::rows() {
        let value = &row["value"];
        let mut scene = append_native::setup(value);
        let mut app = test_app();
        std::mem::swap(&mut scene.document, &mut app.document);
        enter(&mut app, &append_native::header(value));
        for action in value["input"]["points"].as_array().unwrap() {
            if let Some(point) = action.get("edge_pick") {
                let view = crate::viewport::Viewport::new(crate::viewport::ViewKind::Top);
                let rect = egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(800., 600.));
                let pointer = view.project(append_native::pick(point), rect).unwrap();
                let picks = view.pick_mesh_boundary_edges(
                    pointer,
                    rect,
                    app.face_boundary_edges().unwrap(),
                );
                assert!(!picks.is_empty(), "{}", row["id"]);
                assert!(app.handle_viewport_action(crate::viewport::ViewportOutput {
                    mesh_boundary_edge_click: Some(picks),
                    ..Default::default()
                }));
            } else if let Some(word) = action.as_str() {
                enter(&mut app, if word == "Enter" { "" } else { word });
            } else {
                enter(
                    &mut app,
                    &action
                        .as_array()
                        .unwrap()
                        .iter()
                        .map(|x| x.as_f64().unwrap().to_string())
                        .collect::<Vec<_>>()
                        .join(","),
                );
            }
            if app.face_create_session.is_some() {
                assert_eq!(
                    app.document.objects().count(),
                    value["before"].as_array().unwrap().len()
                );
                if let Some(id) = scene.source {
                    assert!(
                        scene.original.as_ref().unwrap().shares_storage_with(
                            app.document.object(id).unwrap().geometry_snapshot()
                        )
                    );
                }
            }
        }
        if value["input"]["cancel"] == true {
            while app.face_create_session.is_some() {
                assert!(app.cancel_face_corner());
            }
        } else {
            for _ in 0..3 {
                if app.face_create_session.is_some() {
                    enter(&mut app, "");
                }
            }
        }
        assert!(
            app.face_create_session.is_none(),
            "{} {:?}",
            row["id"],
            app.command_log
        );
        std::mem::swap(&mut scene.document, &mut app.document);
        let check = append_native::check(&scene, value, "after_script");
        if value["input"]["cancel"] != true {
            assert_eq!(scene.document.undo_label(), Some("3DFace"));
            scene.document.undo().unwrap();
            append_native::check(&scene, value, "undo");
            scene.document.redo().unwrap();
            assert_eq!(append_native::check(&scene, value, "redo"), check);
        } else {
            assert_eq!(scene.document.undo_label(), None);
        }
        println!(
            "SUBD_APPEND_GETTER {}",
            serde_json::json!({"id":row["id"],"check":check,"grouped_history":true})
        );
    }
}

#[test]
fn subd_append_targets_infer_output_and_allow_a_family_switch_before_corners() {
    let rows = append_native::rows();
    for mode in ["preselected", "picker", "explicit", "switch"] {
        let value = &rows
            .iter()
            .find(|r| {
                r["id"]
                    == if mode == "switch" {
                        "subd_extra_output_mesh"
                    } else {
                        "subd_append_flat"
                    }
            })
            .unwrap()["value"];
        let mut scene = append_native::setup(value);
        let mut app = test_app();
        std::mem::swap(&mut scene.document, &mut app.document);
        let id = scene.source.unwrap();
        if mode != "preselected" {
            app.document
                .select_objects_direct([id], viboceros_document::SelectionMode::Remove)
                .unwrap();
        }
        if mode == "explicit" {
            enter(&mut app, &format!("3DFace Append={id}"));
        } else {
            enter(&mut app, "3DFace");
        }
        if mode == "picker" || mode == "switch" {
            enter(&mut app, "Append");
            assert!(app.picking_face_append());
            assert!(app.pick_face_append(Some(id)));
        }
        assert_eq!(
            app.face_create_session.as_ref().unwrap().preferences.output,
            solver::Output::SubD
        );
        if mode == "switch" {
            enter(&mut app, "Output=Mesh");
        }
        for point in value["input"]["points"].as_array().unwrap() {
            enter(
                &mut app,
                &point
                    .as_array()
                    .unwrap()
                    .iter()
                    .map(|x| x.as_f64().unwrap().to_string())
                    .collect::<Vec<_>>()
                    .join(","),
            );
        }
        assert!(app.face_create_session.is_none(), "{:?}", app.command_log);
        std::mem::swap(&mut scene.document, &mut app.document);
        append_native::check(&scene, value, "after_script");
        scene.document.undo().unwrap();
        append_native::check(&scene, value, "undo");
    }
}
