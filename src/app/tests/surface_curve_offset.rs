use super::*;
#[path = "../../../crates/viboceros-command/tests/support/surface_curve_offset.rs"]
mod native;
fn enter(app: &mut VibocerosApp, text: &str) {
    app.command_input = text.into();
    app.run_command();
}
fn scene(app: &mut VibocerosApp) -> (ObjectId, ObjectId) {
    let source = app
        .document
        .add_geometry(Geometry::Line(
            viboceros_geometry::LineSegment::try_new(
                point(-4., 0., 0.),
                point(4., 0., 0.),
                app.document.tolerance(),
            )
            .unwrap(),
        ))
        .unwrap();
    let surface = app
        .document
        .add_geometry(Geometry::NurbsSurface(
            viboceros_geometry::NurbsSurface::try_bilinear([
                point(-10., -10., 0.),
                point(10., -10., 0.),
                point(10., 10., 0.),
                point(-10., 10., 0.),
            ])
            .unwrap(),
        ))
        .unwrap();
    (source, surface)
}

#[test]
fn curved_surface_offset_getters_extend_generators_and_rings_with_cached_confirmation() {
    for (op, value) in native::curved_rows().into_iter().filter(|(op, _)| {
        let case = op["case"].as_str().unwrap();
        case.starts_with("curved_axis") || case.starts_with("curved_ring")
    }) {
        let mut app = test_app();
        let (doc, scene) = native::setup(&op, &value);
        app.document = doc;
        let before = format!("{:?}", app.document);
        enter(&mut app, "OffsetCrvOnSrf Number=1");
        app.pick_surface_offset_face(Some(scene.target), Some(0), None);
        assert_eq!(format!("{:?}", app.document), before);
        let preview = app
            .surface_offset_session
            .as_ref()
            .unwrap()
            .preview_curves()
            .as_ptr();
        enter(&mut app, "Number=1");
        assert_eq!(
            app.surface_offset_session
                .as_ref()
                .unwrap()
                .preview_curves()
                .as_ptr(),
            preview
        );
        let distance = op["actions"]
            .as_array()
            .unwrap()
            .last()
            .unwrap()
            .as_f64()
            .unwrap();
        enter(&mut app, &distance.to_string());
        assert!(
            app.active_command.is_none(),
            "{} {:?}",
            op["id"],
            app.command_log
        );
        let mut maximum = native::check(&app.document, &scene, &value, "after_script");
        app.document.undo().unwrap();
        maximum = maximum.max(native::check(&app.document, &scene, &value, "undo"));
        app.document.redo().unwrap();
        maximum = maximum.max(native::check(&app.document, &scene, &value, "redo"));
        assert!(maximum < 1e-6, "{} {maximum:e}", op["id"]);
    }
}

#[test]
fn surface_offset_point_selection_windows_toggles_and_escape_preserve_redo() {
    let mut app = test_app();
    let (source, surface) = scene(&mut app);
    let first = app
        .document
        .add_geometry(Geometry::Point(point(0., 2., 0.)))
        .unwrap();
    let second = app
        .document
        .add_geometry(Geometry::Point(point(1., -1., 0.)))
        .unwrap();
    app.document
        .select_object(source, SelectionMode::Replace)
        .unwrap();
    enter(&mut app, "Move 0,0,0 1,0,0");
    app.document.undo().unwrap();
    enter(&mut app, "OffsetCrvOnSrf ThroughPoint");
    app.pick_surface_offset_face(Some(surface), None, None);
    enter(&mut app, "Select");
    let before = format!("{:?}", app.document);
    app.apply_selection_window(SelectionWindow {
        object_ids: vec![surface, first, second, first],
        mode: SelectionMode::Replace,
        crossing: false,
        inverted: false,
    });
    assert_eq!(app.surface_offset_selected_points(), &[first, second]);
    app.apply_selection_click(SelectionClick {
        object_id: Some(first),
        mode: SelectionMode::Toggle,
    });
    assert_eq!(app.surface_offset_selected_points(), &[second]);
    enter(&mut app, &first.to_string());
    assert_eq!(app.surface_offset_selected_points(), &[second, first]);
    assert_eq!(
        app.accept_surface_offset_point(point(0., 5., 0.)),
        Some(false)
    );
    app.update_surface_offset_hover(point(0., 5., 0.));
    assert_eq!(format!("{:?}", app.document), before);
    app.cancel_current_prompt_or_selection();
    assert!(app.active_command.is_none());
    assert_eq!(format!("{:?}", app.document), before);
    app.document.redo().unwrap();
    app.document.undo().unwrap();
    enter(
        &mut app,
        &format!("OffsetCrvOnSrf ThroughPoint Source={source} Surface={surface}"),
    );
    enter(&mut app, "Select");
    enter(&mut app, &format!("{second} {first}"));
    assert_eq!(app.surface_offset_selected_points(), &[second, first]);
    enter(&mut app, "Enter");
    assert!(app.active_command.is_none());
    let ys = app
        .document
        .objects()
        .skip(4)
        .map(|o| {
            let c = o.geometry().curve_ref().unwrap();
            c.evaluate(c.parameter_at(0.5).unwrap()).unwrap().y()
        })
        .collect::<Vec<_>>();
    assert_eq!(ys.len(), 2);
    assert!((ys[0] + 1.).abs() < 1e-7 && (ys[1] - 2.).abs() < 1e-7);
}

#[test]
fn surface_offset_native_selected_point_getters_keep_order_and_cancel_without_edits() {
    for (op, value) in native::select_rows() {
        let mut app = test_app();
        let (doc, scene) = native::setup(&op, &value);
        app.document = doc;
        enter(&mut app, "OffsetCrvOnSrf");
        let initial_count = app.document.objects().count();
        let mut selected_count = 0;
        for action in op["actions"].as_array().unwrap() {
            if let Some(index) = action["curve"].as_u64() {
                if index == 1 {
                    app.pick_surface_offset_face(Some(scene.target), Some(0), None);
                } else {
                    let id = scene.references[index as usize - 2];
                    assert!(app.picking_surface_offset_points());
                    app.apply_selection_click(SelectionClick {
                        object_id: Some(id),
                        mode: SelectionMode::Replace,
                    });
                    selected_count += 1;
                    assert_eq!(app.surface_offset_selected_points().len(), selected_count);
                    assert_eq!(app.document.objects().count(), initial_count);
                    assert_eq!(app.document.selected_object_count(), 0);
                    assert!(app.document.undo_label().is_none());
                }
            } else {
                let word = action.as_str().unwrap();
                enter(&mut app, word);
                if word == "Select" {
                    assert!(app.picking_surface_offset_points());
                    assert_eq!(
                        app.surface_offset_session
                            .as_ref()
                            .unwrap()
                            .cursor_surface(),
                        None
                    );
                }
            }
        }
        assert!(
            app.active_command.is_none(),
            "{} {:?}",
            op["id"],
            app.command_log
        );
        let mut maximum = native::check(&app.document, &scene, &value, "after_script");
        if app.document.objects().count() > initial_count {
            app.document.undo().unwrap();
            maximum = maximum.max(native::check(&app.document, &scene, &value, "undo"));
            assert!(app.document.undo().unwrap().is_none());
            app.document.redo().unwrap();
            maximum = maximum.max(native::check(&app.document, &scene, &value, "redo"));
        } else {
            assert!(app.document.undo_label().is_none());
        }
        native::qualify(&op, maximum);
        println!(
            "SURFACE_OFFSET_SELECT_GETTER {}",
            serde_json::json!({"id":op["id"],"maximum_bidirectional_sample_error":maximum,"qualified":maximum<1e-6,"outputs":value["after_script"].as_array().unwrap().len()-value["before"].as_array().unwrap().len()})
        );
    }
}

#[test]
fn surface_offset_target_before_source_prepares_default_preview() {
    let mut app = test_app();
    let (source, surface) = scene(&mut app);
    let before = format!("{:?}", app.document);
    enter(&mut app, &format!("OffsetCrvOnSrf Surface={surface}"));
    assert!(app.picking_surface_offset_curve());
    app.pick_surface_offset_curve(Some(source), None);
    let curves = app
        .surface_offset_session
        .as_ref()
        .unwrap()
        .preview_curves();
    assert_eq!(curves.len(), 1);
    let midpoint = curves[0]
        .evaluate(curves[0].parameter_at(0.5).unwrap())
        .unwrap();
    assert!((midpoint.y() - 1.).abs() < 1e-8);
    assert_eq!(format!("{:?}", app.document), before);
    enter(&mut app, "Enter");
    assert!(app.active_command.is_none());
    assert_eq!(app.document.objects().count(), 3);
    app.document.undo().unwrap();
    assert_eq!(app.document.objects().count(), 2);
    app.document.redo().unwrap();
    assert_eq!(app.document.objects().count(), 3);
}

#[test]
fn surface_offset_previews_flip_number_and_atomic_history() {
    let mut app = test_app();
    let (source, surface) = scene(&mut app);
    app.document
        .select_object(source, SelectionMode::Replace)
        .unwrap();
    let before = format!("{:?}", app.document);
    enter(&mut app, "OffsetCrvOnSrf");
    assert!(app.picking_surface_offset_face());
    app.pick_surface_offset_face(Some(surface), Some(0), Some(point(0., 3., 0.)));
    let preview = app
        .surface_offset_session
        .as_ref()
        .unwrap()
        .preview_curves()
        .as_ptr();
    assert_eq!(
        app.surface_offset_session
            .as_ref()
            .unwrap()
            .preview_curves()
            .as_ptr(),
        preview
    );
    assert_eq!(format!("{:?}", app.document), before);
    enter(&mut app, "Number");
    enter(&mut app, "3");
    enter(&mut app, "Flip");
    let curves = app
        .surface_offset_session
        .as_ref()
        .unwrap()
        .preview_curves();
    assert_eq!(curves.len(), 3);
    for (i, c) in curves.iter().enumerate() {
        let q = c.evaluate(c.parameter_at(0.5).unwrap()).unwrap();
        assert!((q.y() + (i + 1) as f64).abs() < 1e-8);
    }
    enter(&mut app, "Enter");
    assert!(app.active_command.is_none());
    assert_eq!(
        app.document.selected_object_ids().collect::<Vec<_>>(),
        vec![source]
    );
    assert_eq!(app.document.undo_label(), Some("OffsetCrvOnSrf"));
    app.document.undo().unwrap();
    assert_eq!(app.document.objects().count(), 2);
    app.document.redo().unwrap();
    assert_eq!(app.document.objects().count(), 5);
    enter(&mut app, "OffsetCrvOnSrf");
    app.pick_surface_offset_face(Some(surface), None, Some(point(0., 3., 0.)));
    let session = app.surface_offset_session.as_ref().unwrap();
    assert_eq!(session.preview_curves().len(), 3);
    assert!(
        session.preview_curves()[0]
            .evaluate(session.preview_curves()[0].parameter_at(0.5).unwrap())
            .unwrap()
            .y()
            > 0.
    );
    app.cancel_current_prompt_or_selection();
}

#[test]
fn surface_offset_escape_stale_captures_and_failed_options_preserve_redo() {
    let mut app = test_app();
    let (source, surface) = scene(&mut app);
    app.document
        .select_object(source, SelectionMode::Replace)
        .unwrap();
    enter(&mut app, "Move 0,0,0 1,0,0");
    app.document.undo().unwrap();
    let before = format!("{:?}", app.document);
    enter(&mut app, "OffsetCrvOnSrf");
    app.pick_surface_offset_face(Some(surface), None, Some(point(0., 2., 0.)));
    enter(&mut app, "Number=0");
    assert!(app.surface_offset_session.is_some());
    app.cancel_current_prompt_or_selection();
    assert_eq!(format!("{:?}", app.document), before);
    app.document.redo().unwrap();
    app.document.undo().unwrap();
    enter(&mut app, "OffsetCrvOnSrf");
    app.pick_surface_offset_face(Some(surface), None, None);
    app.document
        .replace_object_geometries([(surface, Geometry::Point(point(0., 0., 0.)))])
        .unwrap();
    assert!(!app.validate_surface_offset());
    assert!(app.active_command.is_none());
    assert!(app.surface_offset_session.is_none());
}

#[test]
fn surface_offset_temporary_curve_input_is_local_and_directional() {
    let mut app = test_app();
    let (source, surface) = scene(&mut app);
    let before = format!("{:?}", app.document);
    enter(&mut app, "OffsetCrvOnSrf");
    enter(&mut app, "SubCrv");
    app.pick_surface_offset_curve(Some(source), None);
    app.accept_surface_offset_point(point(2., 0., 0.));
    app.update_surface_offset_hover(point(-2., 0., 0.));
    assert_eq!(
        app.surface_offset_session
            .as_ref()
            .unwrap()
            .preview_curves()
            .len(),
        1
    );
    assert_eq!(format!("{:?}", app.document), before);
    app.cancel_current_prompt_or_selection();
    assert!(app.active_command.is_some());
    assert!(app.picking_surface_offset_curve());
    enter(&mut app, "SubCrv");
    app.pick_surface_offset_curve(Some(source), None);
    app.accept_surface_offset_point(point(2., 0., 0.));
    app.accept_surface_offset_point(point(-2., 0., 0.));
    assert!(app.picking_surface_offset_face());
    app.pick_surface_offset_face(Some(surface), None, Some(point(0., -3., 0.)));
    enter(&mut app, "1");
    let c = app
        .document
        .objects()
        .last()
        .unwrap()
        .geometry()
        .curve_ref()
        .unwrap();
    assert!(
        c.start_point()
            .unwrap()
            .distance_to(point(10., -1., 0.))
            .unwrap()
            < 1e-8
    );
    assert!(
        c.end_point()
            .unwrap()
            .distance_to(point(-10., -1., 0.))
            .unwrap()
            < 1e-8
    );
}

#[test]
fn surface_curve_offset_native_getters_keep_selection_layers_and_grouped_history() {
    for (op, value) in native::rows() {
        let mut app = test_app();
        let (doc, scene) = native::setup(&op, &value);
        app.document = doc;
        let before = format!("{:?}", app.document);
        enter(&mut app, "OffsetCrvOnSrf");
        // The captures run in one Rhino process and retain Number. Replay each
        // scene independently with the captured final Number explicitly set.
        for action in op["actions"].as_array().unwrap() {
            if let Some(index) = action["curve"].as_u64() {
                if index == 0 {
                    assert!(app.pick_surface_offset_curve(Some(scene.source), None));
                } else {
                    assert!(app.pick_surface_offset_face(Some(scene.target), Some(0), None));
                }
            } else if let Some(text) = action.as_str() {
                enter(&mut app, text);
            } else {
                enter(&mut app, &format!("Number={}", native::number(&value)));
                assert_eq!(format!("{:?}", app.document), before);
                enter(&mut app, &action.as_f64().unwrap().to_string());
            }
        }
        assert!(
            app.active_command.is_none(),
            "{} {:?}",
            op["id"],
            app.command_log
        );
        let mut maximum = native::check(&app.document, &scene, &value, "after_script");
        app.document.undo().unwrap();
        maximum = maximum.max(native::check(&app.document, &scene, &value, "undo"));
        assert!(app.document.undo().unwrap().is_none());
        app.document.redo().unwrap();
        maximum = maximum.max(native::check(&app.document, &scene, &value, "redo"));
        native::qualify(&op, maximum);
    }
}

#[test]
fn surface_curve_offset_through_point_native_repeats_previews_and_cancel() {
    for (op, value) in native::through_rows() {
        let mut app = test_app();
        let (doc, scene) = native::setup(&op, &value);
        app.document = doc;
        enter(&mut app, "OffsetCrvOnSrf");
        for action in op["actions"].as_array().unwrap() {
            if action["curve"].as_u64().is_some() {
                app.pick_surface_offset_face(Some(scene.target), Some(0), None);
            } else if let Some(text) = action.as_str() {
                enter(&mut app, text);
            } else {
                let [x, y, z]: [f64; 3] = serde_json::from_value(action.clone()).unwrap();
                let before = format!("{:?}", app.document);
                let point = point(x, y, z);
                app.update_surface_offset_hover(point);
                let preview = app
                    .surface_offset_session
                    .as_ref()
                    .unwrap()
                    .preview_curves()
                    .as_ptr();
                app.update_surface_offset_hover(point);
                assert_eq!(
                    preview,
                    app.surface_offset_session
                        .as_ref()
                        .unwrap()
                        .preview_curves()
                        .as_ptr()
                );
                assert_eq!(format!("{:?}", app.document), before);
                assert_eq!(
                    app.surface_offset_session
                        .as_ref()
                        .unwrap()
                        .cursor_surface(),
                    Some((scene.target, Some(0), false))
                );
                enter(&mut app, &format!("w{x},{y},{z}"));
                assert!(
                    app.active_command.is_some(),
                    "{} {:?}",
                    op["id"],
                    app.command_log
                );
                if op["case"] == "through_on_source" {
                    assert_eq!(format!("{:?}", app.document), before);
                }
            }
        }
        assert!(
            app.active_command.is_none(),
            "{} {:?}",
            op["id"],
            app.command_log
        );
        let mut maximum = native::check(&app.document, &scene, &value, "after_script");
        if value["after_script"].as_array().unwrap().len() > 2 {
            app.document.undo().unwrap();
            maximum = maximum.max(native::check(&app.document, &scene, &value, "undo"));
            assert!(app.document.undo().unwrap().is_none());
            app.document.redo().unwrap();
            maximum = maximum.max(native::check(&app.document, &scene, &value, "redo"));
        } else {
            assert!(app.document.undo_label().is_none());
        }
        native::qualify(&op, maximum);
    }
}

#[test]
fn failed_through_points_preserve_redo_and_external_history_edits_end_the_getter() {
    let mut app = test_app();
    let (source, surface) = scene(&mut app);
    app.document
        .select_object(source, SelectionMode::Replace)
        .unwrap();
    enter(&mut app, "Move 0,0,0 1,0,0");
    app.document.undo().unwrap();
    let before = format!("{:?}", app.document);
    enter(&mut app, "OffsetCrvOnSrf ThroughPoint");
    app.pick_surface_offset_face(Some(surface), None, None);
    assert_eq!(
        app.accept_surface_offset_point(point(0., 0., 0.)),
        Some(false)
    );
    app.cancel_current_prompt_or_selection();
    assert_eq!(format!("{:?}", app.document), before);
    app.document.redo().unwrap();
    app.document.undo().unwrap();
    enter(&mut app, "OffsetCrvOnSrf ThroughPoint");
    app.pick_surface_offset_face(Some(surface), None, None);
    assert_eq!(
        app.accept_surface_offset_point(point(0., 2., 0.)),
        Some(true)
    );
    app.document
        .add_geometry(Geometry::Point(point(0., 0., 3.)))
        .unwrap();
    assert!(!app.validate_surface_offset());
    assert!(app.active_command.is_none());
    assert_eq!(app.document.objects().count(), 4);
}
