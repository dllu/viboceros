use super::boolean_intersection::enter;
use super::*;
use viboceros_document::SelectionMode;

fn fixture() -> (VibocerosApp, viboceros_document::ObjectId) {
    let mut app = test_app();
    let id = app
        .document
        .add_geometry(Geometry::NurbsCurve(
            NurbsCurve::try_new(
                3,
                vec![
                    Point3::try_new(0., 0., 0.).unwrap(),
                    Point3::try_new(1., 2., 0.).unwrap(),
                    Point3::try_new(3., -1., 0.).unwrap(),
                    Point3::try_new(4., 0., 0.).unwrap(),
                ],
                vec![0., 0., 0., 0., 1., 1., 1., 1.],
            )
            .unwrap(),
        ))
        .unwrap();
    app.document
        .select_objects_direct([id], SelectionMode::Replace)
        .unwrap();
    app.document.clear_history().unwrap();
    (app, id)
}
fn ready(app: &VibocerosApp) -> viboceros_document::GeometrySnapshot {
    app.rebuild_preview
        .as_ref()
        .unwrap()
        .prepared
        .as_ref()
        .unwrap()
        .outputs()
        .next()
        .unwrap()
        .clone()
}
fn drawn(app: &VibocerosApp) -> &Document {
    crate::app::surface_rebuild::viewport_document(
        &app.document,
        app.boolean_two_prompt.as_ref().and_then(|p| p.scene()),
        app.tween_surfaces_prompt.as_ref().and_then(|p| p.scene()),
        app.rebuild_preview.as_ref().and_then(|p| p.scene()),
    )
}

#[test]
fn curve_rebuild_previews_typed_edits_and_accepts_exact_geometry_in_one_history_step() {
    let (mut app, id) = fixture();
    let before = app.document.objects().cloned().collect::<Vec<_>>();
    enter(&mut app, "RebuildCrv");
    assert!(app.object_prompt.is_some());
    let first = ready(&app);
    assert_eq!(app.document.objects().cloned().collect::<Vec<_>>(), before);
    assert!(app.document.undo_label().is_none());
    enter(&mut app, "PointCount");
    enter(&mut app, "7");
    enter(&mut app, "Degree 2");
    enter(&mut app, "PreserveTangents=Yes");
    let output = ready(&app);
    assert!(!output.shares_storage_with(&first));
    let Geometry::NurbsCurve(curve) = &*output else {
        panic!()
    };
    assert_eq!(curve.degree(), 2);
    assert_eq!(curve.control_points().len(), 7);
    let scene = app.rebuild_preview.as_ref().unwrap().scene().unwrap();
    assert_eq!(scene.object(id).unwrap().geometry(), &*output);
    assert_eq!(app.document.objects().cloned().collect::<Vec<_>>(), before);
    enter(&mut app, "Preview");
    assert!(output.shares_storage_with(&ready(&app)));
    enter(&mut app, "");
    assert!(app.object_prompt.is_none());
    assert_eq!(app.document.object(id).unwrap().geometry(), &*output);
    assert_eq!(app.document.undo_label(), Some("Rebuild"));
    enter(&mut app, "Undo");
    assert_eq!(app.document.objects().cloned().collect::<Vec<_>>(), before);
    enter(&mut app, "Redo");
    assert_eq!(app.document.object(id).unwrap().geometry(), &*output);
}
#[test]
fn curve_rebuild_failed_preparation_recovers_and_cancellation_preserves_redo() {
    let (mut app, id) = fixture();
    app.commands
        .execute(&mut app.document, "Rebuild PointCount=6 Degree=2")
        .unwrap();
    enter(&mut app, "Undo");
    let before = app.document.objects().cloned().collect::<Vec<_>>();
    let redo = app.document.redo_label().map(str::to_owned);
    enter(&mut app, "Rebuild");
    let output = ready(&app);
    enter(&mut app, "Degree=0");
    assert!(output.shares_storage_with(&ready(&app)));
    enter(&mut app, "PointCount=0");
    assert!(app.rebuild_preview.as_ref().unwrap().prepared.is_none());
    assert!(app.rebuild_preview.as_ref().unwrap().scene().is_none());
    enter(&mut app, "");
    assert!(app.object_prompt.is_some());
    assert_eq!(app.document.object(id).unwrap(), &before[0]);
    enter(&mut app, "PointCount=6");
    assert!(app.rebuild_preview.as_ref().unwrap().scene().is_some());
    enter(&mut app, "Cancel");
    assert!(app.rebuild_preview.is_none());
    assert_eq!(app.document.objects().cloned().collect::<Vec<_>>(), before);
    assert_eq!(app.document.redo_label(), redo.as_deref());
}
#[test]
fn curve_rebuild_stale_source_rejects_option_edits_and_acceptance() {
    for input in ["PointCount=8", ""] {
        let (mut app, id) = fixture();
        enter(&mut app, "Rebuild");
        app.document
            .set_object_names([(id, Some("changed".into()))])
            .unwrap();
        let before = app.document.objects().cloned().collect::<Vec<_>>();
        enter(&mut app, input);
        assert!(app.object_prompt.is_none());
        assert!(app.rebuild_preview.is_none());
        assert_eq!(app.document.objects().cloned().collect::<Vec<_>>(), before);
    }
}
#[test]
fn curve_rebuild_postselection_previews_and_inline_scripts_accept_directly() {
    for inline in [false, true] {
        let (mut app, id) = fixture();
        app.document.clear_selection();
        enter(
            &mut app,
            if inline {
                "Rebuild PointCount=6 Degree=2"
            } else {
                "Rebuild"
            },
        );
        assert!(matches!(
            app.object_prompt.as_ref().unwrap().phase,
            crate::app::object_selection::ObjectPromptPhase::Selecting
        ));
        app.document
            .select_objects_direct([id], SelectionMode::Replace)
            .unwrap();
        enter(&mut app, "");
        if inline {
            assert!(app.object_prompt.is_none());
            assert_eq!(app.document.undo_label(), Some("Rebuild"));
        } else {
            assert!(app.rebuild_preview.is_some());
            assert!(app.document.undo_label().is_none());
            enter(&mut app, "");
        }
        let Geometry::NurbsCurve(curve) = app.document.object(id).unwrap().geometry() else {
            panic!()
        };
        assert_eq!(curve.degree(), if inline { 2 } else { 3 });
    }
}

#[test]
fn closed_curve_preview_reuses_copy_policy_and_refreshes_background_without_rebuilding() {
    let (mut app, id) = fixture();
    let square = viboceros_geometry::Polyline3::try_new(
        vec![
            Point3::try_new(0., 0., 0.).unwrap(),
            Point3::try_new(2., 0., 0.).unwrap(),
            Point3::try_new(2., 2., 0.).unwrap(),
            Point3::try_new(0., 2., 0.).unwrap(),
            Point3::try_new(0., 0., 0.).unwrap(),
        ],
        app.document.tolerance(),
    )
    .unwrap();
    app.document
        .replace_object_geometries([(id, Geometry::Polyline(square))])
        .unwrap();
    app.document.clear_history().unwrap();
    let source = app.document.object(id).unwrap().clone();
    enter(&mut app, "Rebuild");
    enter(&mut app, "Points=8");
    let output = ready(&app);
    let Geometry::NurbsCurve(curve) = &*output else {
        panic!()
    };
    assert!(curve.is_periodic());
    assert!(curve.is_closed().unwrap());
    enter(&mut app, "DeleteInput=No");
    assert!(output.shares_storage_with(&ready(&app)));
    assert_eq!(
        app.rebuild_preview
            .as_ref()
            .unwrap()
            .scene()
            .unwrap()
            .objects()
            .len(),
        2
    );
    let background = app
        .document
        .add_geometry(Geometry::Point(Point3::try_new(7., 0., 0.).unwrap()))
        .unwrap();
    app.validate_rebuild_preview();
    assert!(output.shares_storage_with(&ready(&app)));
    assert!(
        app.rebuild_preview
            .as_ref()
            .unwrap()
            .scene()
            .unwrap()
            .object(background)
            .is_some()
    );
    enter(&mut app, "");
    assert_eq!(app.document.object(id).unwrap(), &source);
    assert_eq!(app.document.objects().last().unwrap().geometry(), &*output);
}

#[test]
fn curve_preview_routes_drawn_objects_without_changing_the_editable_model() {
    let (mut app, id) = fixture();
    let before = app.document.objects().cloned().collect::<Vec<_>>();
    enter(&mut app, "Rebuild");
    enter(&mut app, "Degree=1 PointCount=2");
    let output = ready(&app);
    assert_eq!(drawn(&app).object(id).unwrap().geometry(), &*output);
    assert_ne!(
        drawn(&app).object(id).unwrap().geometry(),
        app.document.object(id).unwrap().geometry()
    );
    for mode in ["Wireframe", "Shaded", "Ghosted"] {
        enter(
            &mut app,
            &format!("SetDisplayMode Viewport=All Mode={mode}"),
        );
        assert!(output.shares_storage_with(&ready(&app)));
        assert_eq!(drawn(&app).objects().len(), 1);
        assert_eq!(app.document.objects().cloned().collect::<Vec<_>>(), before);
    }
    enter(&mut app, "DeleteInput=No");
    assert_eq!(drawn(&app).objects().len(), 2);
    assert_eq!(drawn(&app).object(id).unwrap(), &before[0]);
    assert_eq!(app.document.objects().len(), 1);
    enter(&mut app, "PointCount=0");
    assert!(std::ptr::eq(drawn(&app), &app.document));
    enter(&mut app, "PointCount=2");
    assert_eq!(drawn(&app).objects().len(), 2);
    enter(&mut app, "Cancel");
    assert!(std::ptr::eq(drawn(&app), &app.document));
    assert_eq!(app.document.objects().cloned().collect::<Vec<_>>(), before);
}

#[test]
fn curve_preview_rejects_all_stale_source_categories_before_editing_or_accepting() {
    for change in 0..8 {
        for input in ["PointCount=8", ""] {
            let (mut app, id) = fixture();
            enter(&mut app, "Rebuild");
            match change {
                0 => {
                    app.document.add_group(None, [id]).unwrap();
                }
                1 => {
                    app.document
                        .set_object_geometry_user_text([id], "Code", Some("changed"))
                        .unwrap();
                }
                2 => {
                    app.document.set_tolerance(
                        viboceros_geometry::Tolerance::try_new(1e-5, 1e-12, 1e-10).unwrap(),
                    );
                }
                3 => {
                    app.document.clear_selection();
                }
                4 => {
                    let layer = app
                        .document
                        .add_layer("Other", viboceros_document::ColorRgb::BLACK)
                        .unwrap();
                    app.document.set_current_layer(layer).unwrap();
                }
                5 => {
                    app.document.delete_object(id).unwrap();
                }
                6 => {
                    app.document
                        .set_objects_color([id], Some(viboceros_document::ColorRgb::new(2, 3, 4)))
                        .unwrap();
                }
                _ => {
                    let Geometry::NurbsCurve(curve) = app.document.object(id).unwrap().geometry()
                    else {
                        panic!()
                    };
                    let geometry = Geometry::NurbsCurve(curve.reversed().unwrap());
                    app.document
                        .replace_object_geometries([(id, geometry)])
                        .unwrap();
                }
            }
            let before = app.document.objects().cloned().collect::<Vec<_>>();
            let undo = app.document.undo_label().map(str::to_owned);
            enter(&mut app, input);
            assert!(
                app.object_prompt.is_none(),
                "change={change}, input={input}"
            );
            assert!(app.rebuild_preview.is_none());
            assert_eq!(app.document.objects().cloned().collect::<Vec<_>>(), before);
            assert_eq!(app.document.undo_label(), undo.as_deref());
        }
    }
}

#[test]
fn equal_geometry_assignment_keeps_the_curve_preview_and_prepared_storage() {
    let (mut app, id) = fixture();
    enter(&mut app, "Rebuild");
    let output = ready(&app);
    let geometry = app.document.object(id).unwrap().geometry().clone();
    assert_eq!(
        app.document
            .replace_object_geometries([(id, geometry)])
            .unwrap(),
        0
    );
    app.validate_rebuild_preview();
    assert!(app.object_prompt.is_some());
    assert!(output.shares_storage_with(&ready(&app)));
    assert!(!app.document.can_undo());
}

fn choices_frame(
    app: &mut VibocerosApp,
    context: &egui::Context,
    events: Vec<egui::Event>,
) -> egui::FullOutput {
    context.run_ui(
        egui::RawInput {
            screen_rect: Some(egui::Rect::from_min_size(
                egui::Pos2::ZERO,
                egui::Vec2::new(1000., 300.),
            )),
            events,
            ..Default::default()
        },
        |ui| app.show_rebuild_choices(ui),
    )
}
fn click_choice(app: &mut VibocerosApp, context: &egui::Context, label: &str) {
    let output = choices_frame(app, context, vec![]);
    let pos = output
        .shapes
        .iter()
        .find_map(|shape| {
            if let egui::Shape::Text(t) = &shape.shape
                && t.galley.job.text == label
            {
                Some(t.pos + t.galley.rect.center().to_vec2())
            } else {
                None
            }
        })
        .unwrap_or_else(|| panic!("missing choice {label}"));
    output.drop_without_applying_deltas();
    for pressed in [true, false] {
        choices_frame(
            app,
            context,
            vec![
                egui::Event::PointerMoved(pos),
                egui::Event::PointerButton {
                    pos,
                    button: egui::PointerButton::Primary,
                    pressed,
                    modifiers: egui::Modifiers::NONE,
                },
            ],
        )
        .drop_without_applying_deltas();
    }
}
#[test]
fn curve_rebuild_option_buttons_edit_and_accept_the_same_prepared_outputs() {
    let (mut app, id) = fixture();
    let before = app.document.objects().cloned().collect::<Vec<_>>();
    enter(&mut app, "Rebuild");
    let output = ready(&app);
    let context = egui::Context::default();
    click_choice(&mut app, &context, "DeleteInput=Yes");
    assert!(output.shares_storage_with(&ready(&app)));
    assert_eq!(drawn(&app).objects().len(), 2);
    click_choice(&mut app, &context, "PointCount=10");
    assert!(matches!(
        app.object_prompt.as_ref().unwrap().phase,
        crate::app::object_selection::ObjectPromptPhase::RebuildValue("PointCount")
    ));
    click_choice(&mut app, &context, "Keep value");
    click_choice(&mut app, &context, "PreserveTangents=No");
    let output = ready(&app);
    assert_eq!(app.document.objects().cloned().collect::<Vec<_>>(), before);
    enter(&mut app, "PointCount=0");
    click_choice(&mut app, &context, "Accept");
    assert!(app.object_prompt.is_some());
    assert_eq!(app.document.objects().cloned().collect::<Vec<_>>(), before);
    enter(&mut app, "PointCount=10");
    let final_output = ready(&app);
    assert_eq!(*output, *final_output);
    click_choice(&mut app, &context, "Accept");
    assert!(app.object_prompt.is_none());
    assert_eq!(app.document.object(id).unwrap(), &before[0]);
    assert_eq!(
        app.document.objects().last().unwrap().geometry(),
        &*final_output
    );
    assert_eq!(app.document.undo_label(), Some("Rebuild"));
}
#[test]
fn rebuild_choice_rendering_cancels_stale_sources_before_button_actions() {
    let (mut app, id) = fixture();
    enter(&mut app, "Rebuild");
    app.document
        .set_object_names([(id, Some("changed".into()))])
        .unwrap();
    let before = app.document.objects().cloned().collect::<Vec<_>>();
    let context = egui::Context::default();
    choices_frame(&mut app, &context, vec![]).drop_without_applying_deltas();
    assert!(app.object_prompt.is_none());
    assert!(app.rebuild_preview.is_none());
    assert_eq!(app.document.objects().cloned().collect::<Vec<_>>(), before);
}

#[test]
fn surface_rebuild_buttons_keep_independent_axes_and_reuse_output_options() {
    let mut app = test_app();
    enter(&mut app, "SrfPt 0,0,0 4,0,0 4,3,0 0,3,0");
    enter(&mut app, "SelAll");
    app.document.clear_history().unwrap();
    let before = app.document.objects().cloned().collect::<Vec<_>>();
    enter(&mut app, "Rebuild");
    let output = ready(&app);
    let context = egui::Context::default();
    click_choice(&mut app, &context, "UPointCount=10");
    enter(&mut app, "5");
    let ready_surface = ready(&app);
    assert!(!output.shares_storage_with(&ready_surface));
    let Geometry::Brep(b) = &*ready_surface else {
        panic!()
    };
    assert_eq!(b.faces()[0].surface().control_point_count_u(), 5);
    assert_eq!(b.faces()[0].surface().control_point_count_v(), 10);
    click_choice(&mut app, &context, "DeleteInput=Yes");
    assert!(ready_surface.shares_storage_with(&ready(&app)));
    assert_eq!(drawn(&app).objects().len(), 2);
    assert_eq!(app.document.objects().cloned().collect::<Vec<_>>(), before);
    click_choice(&mut app, &context, "Cancel");
    assert!(app.object_prompt.is_none());
    assert_eq!(app.document.objects().cloned().collect::<Vec<_>>(), before);
    assert!(!app.document.can_undo());
}
