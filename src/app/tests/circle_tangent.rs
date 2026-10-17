use super::*;
fn enter(app: &mut VibocerosApp, text: &str) {
    app.command_input = text.into();
    app.run_command();
}
#[test]
fn circle_tangent_native_frontend_contacts_and_actual_history() {
    super::ellipse_around::assert_native_profiles_for(
        "Circle",
        &[
            (
                include_str!("../../../tools/rhino_oracle/fixtures/circle_tangent_getters.json"),
                include_str!(
                    "../../../tools/rhino_oracle/observations/circle_tangent_getters_rhino_20261017.json"
                ),
                10,
            ),
            (
                include_str!("../../../tools/rhino_oracle/fixtures/circle_tangent_special.json"),
                include_str!(
                    "../../../tools/rhino_oracle/observations/circle_tangent_special_rhino_20261017.json"
                ),
                6,
            ),
            (
                include_str!("../../../tools/rhino_oracle/fixtures/circle_tangent_curves.json"),
                include_str!(
                    "../../../tools/rhino_oracle/observations/circle_tangent_curves_rhino_20261017.json"
                ),
                5,
            ),
            (
                include_str!("../../../tools/rhino_oracle/fixtures/circle_tangent_spatial.json"),
                include_str!(
                    "../../../tools/rhino_oracle/observations/circle_tangent_spatial_rhino_20261017.json"
                ),
                6,
            ),
        ],
    );
}
fn p(x: f64, y: f64, z: f64) -> Point3 {
    Point3::try_new(x, y, z).unwrap()
}
fn guides(app: &mut VibocerosApp) -> [ObjectId; 2] {
    [p(12., 0., 0.), p(0., 12., 0.)].map(|end| {
        app.document
            .add_geometry(Geometry::Line(
                viboceros_geometry::LineSegment::try_new(
                    p(0., 0., 0.),
                    end,
                    app.document.tolerance(),
                )
                .unwrap(),
            ))
            .unwrap()
    })
}
#[test]
fn circle_tangent_rejects_stale_sources_and_cancels_without_history() {
    for deform in [false, true] {
        for change in 0..5 {
            let mut app = test_app();
            let [a, b] = guides(&mut app);
            enter(
                &mut app,
                if deform {
                    "Circle Deformable Degree=4 PointCount=12 Tangent"
                } else {
                    "Circle Tangent"
                },
            );
            assert!(app.pick_circle_tangent_at(a, None, p(3., 0., 0.)));
            enter(&mut app, "FromFirstPoint");
            match change {
                0 => {
                    app.document
                        .replace_object_geometries([(a, Geometry::Point(p(1., 0., 0.)))])
                        .unwrap();
                }
                1 => {
                    app.document.set_objects_visibility([a], false).unwrap();
                }
                2 => {
                    app.document.set_objects_locked([a], true).unwrap();
                }
                3 => {
                    app.document
                        .set_tolerance(Tolerance::try_new(2e-9, 1e-12, 1e-10).unwrap());
                }
                _ => {
                    app.document.delete_objects([a]).unwrap();
                }
            }
            app.document.clear_history().unwrap();
            app.validate_circle_tangent();
            assert!(
                app.circle_tangent_session
                    .as_ref()
                    .unwrap()
                    .input
                    .anchor()
                    .is_none()
            );
            assert!(app.picking_circle_tangent());
            assert!(app.pick_circle_tangent_at(b, None, p(0., 4., 0.)));
            assert!(app.circle_tangent_session.is_some());
            assert!(app.document.objects().all(|o| [a, b].contains(&o.id())));
            enter(&mut app, "Cancel");
            assert!(app.circle_tangent_session.is_none());
            assert!(app.document.undo().unwrap().is_none());
        }
    }
}
#[test]
fn circle_tangent_point_preview_errors_and_interrupt_are_readonly() {
    let mut app = test_app();
    let [a, b] = guides(&mut app);
    app.document.clear_history().unwrap();
    enter(&mut app, "Circle");
    enter(&mut app, "Tangent");
    assert!(app.pick_circle_tangent_at(a, None, p(3., 0., 0.)));
    enter(&mut app, "Point");
    let before = format!("{:?}", app.document);
    let preview = app
        .circle_tangent_session
        .as_ref()
        .unwrap()
        .input
        .preview_point(p(0., 4., 0.))
        .unwrap();
    assert_eq!(preview.center(), p(0., 2., 0.));
    assert_eq!(format!("{:?}", app.document), before);
    enter(&mut app, "w0,4,0");
    enter(&mut app, "Enter");
    assert!(app.circle_tangent_session.is_none());
    assert_eq!(app.commands.circle_preferences().radius, 2.);
    assert!(!app.document.is_selected(a));
    assert!(!app.document.is_selected(b));
    assert_eq!(app.document.undo().unwrap().as_deref(), Some("Circle"));
    enter(&mut app, "Circle Tangent");
    assert!(app.pick_circle_tangent_at(a, None, p(3., 0., 0.)));
    enter(&mut app, "-2");
    assert!(app.circle_tangent_session.is_some());
    enter(&mut app, "Line");
    assert!(app.circle_tangent_session.is_none());
    assert!(matches!(
        app.active_command,
        Some(InteractiveCommand::Line { .. })
    ));
}

#[test]
fn circle_tangent_viewport_pointer_routes_object_and_surface_edge_contacts() {
    for edge_source in [false, true] {
        let mut app = test_app();
        let [mut a, b] = guides(&mut app);
        let mut edge = None;
        if edge_source {
            app.document.delete_objects([a]).unwrap();
            let source: viboceros_oracle::CircleAroundFixture =
                serde_json::from_value(serde_json::json!({
                    "actions":[],"curves":[{"type":"box_brep","min":[0,0,0],"max":[12,6,2]}]
                }))
                .unwrap();
            a = app
                .document
                .add_geometry(
                    source
                        .source_geometries(app.document.tolerance())
                        .unwrap()
                        .remove(0),
                )
                .unwrap();
            edge = (0..12).find(|&i| {
                let c = viboceros_command::curve_reference::resolve(
                    app.document.object(a).unwrap().geometry(),
                    Some(i),
                    app.document.tolerance(),
                )
                .unwrap();
                let a = c.curve().start_point().unwrap();
                let b = c.curve().end_point().unwrap();
                a.y().abs() < 1e-10
                    && b.y().abs() < 1e-10
                    && a.z().abs() < 1e-10
                    && b.z().abs() < 1e-10
            });
            assert!(edge.is_some());
        }
        app.document.clear_history().unwrap();
        app.active_viewport = 0;
        app.viewports[0] = crate::viewport::Viewport::new(crate::viewport::ViewKind::Top);
        let context = egui::Context::default();
        let mut rect = egui::Rect::NOTHING;
        context
            .run_ui(
                egui::RawInput {
                    screen_rect: Some(egui::Rect::from_min_size(
                        egui::Pos2::ZERO,
                        egui::vec2(800., 600.),
                    )),
                    ..Default::default()
                },
                |ui| {
                    rect = ui.available_rect_before_wrap();
                    app.viewports[0].show(
                        ui,
                        &app.document,
                        crate::viewport::ViewportInput::default(),
                        &[],
                        0,
                        true,
                    );
                },
            )
            .drop_without_applying_deltas();
        enter(&mut app, "Circle Tangent");
        let first = app.viewports[0].project(p(3., 0., 0.), rect).unwrap();
        let output = if let Some(edge) = edge {
            ViewportOutput {
                selection_pointer: Some(first),
                edge_click: Some(vec![crate::viewport::EdgePick { object: a, edge }]),
                ..Default::default()
            }
        } else {
            ViewportOutput {
                selection_pointer: Some(first),
                selection_click: Some(crate::viewport::SelectionClick {
                    object_id: Some(a),
                    mode: SelectionMode::Replace,
                }),
                ..Default::default()
            }
        };
        assert!(app.handle_viewport_action(output));
        assert!(
            app.circle_tangent_session
                .as_ref()
                .unwrap()
                .input
                .anchor()
                .unwrap()
                .distance_to(p(3., 0., 0.))
                .unwrap()
                < 1e-5
        );
        let second = app.viewports[0].project(p(0., 4., 0.), rect).unwrap();
        assert!(app.handle_viewport_action(ViewportOutput {
            selection_pointer: Some(second),
            selection_click: Some(crate::viewport::SelectionClick {
                object_id: Some(b),
                mode: SelectionMode::Replace
            }),
            ..Default::default()
        }));
        let input = &app.circle_tangent_session.as_ref().unwrap().input;
        let received = input.anchor().unwrap();
        let preview = input.preview_current().unwrap();
        enter(&mut app, "Enter");
        assert!(
            app.circle_tangent_session.is_none(),
            "{:?}",
            app.command_log
        );
        let Geometry::Circle(circle) = app
            .document
            .objects()
            .find(|o| ![a, b].contains(&o.id()))
            .unwrap()
            .geometry()
        else {
            panic!()
        };
        assert_eq!(*circle, preview);
        assert!((circle.radius() - received.y()).abs() < 1e-8);
        assert_eq!(app.document.selected_object_ids().count(), 0);
        assert_eq!(app.document.undo().unwrap().as_deref(), Some("Circle"));
        assert!(app.document.undo().unwrap().is_none());
    }
}
