use super::{curve_fillet::enter, *};
use viboceros_geometry::CurveSegment3;

fn source(app: &mut VibocerosApp) -> ObjectId {
    app.commands
        .execute(&mut app.document, "Polyline 0,0,0 6,0,0 6,6,0")
        .unwrap();
    app.document.objects().last().unwrap().id()
}

fn assert_radius(geometry: &Geometry, expected: f64) {
    let Geometry::PolyCurve(curve) = geometry else {
        panic!("expected the rounded polycurve");
    };
    let CurveSegment3::Arc(arc) = curve.segments()[1] else {
        panic!("expected the corner arc");
    };
    assert!((arc.radius() - expected).abs() < 1e-12);
}

#[test]
fn fillet_corners_accepts_preselection_units_defaults_and_spatial_distances() {
    let mut app = test_app();
    let id = source(&mut app);
    app.commands
        .execute(&mut app.document, "Line 1,1,0 5,5,0")
        .unwrap();
    let peer = app.document.objects().last().unwrap().id();
    let peer_geometry = app
        .document
        .object(peer)
        .unwrap()
        .geometry_snapshot()
        .clone();
    enter(&mut app, "SelAll");
    enter(&mut app, "FilletCorners");
    enter(&mut app, "");
    let result = app.document.object(id).unwrap().geometry();
    assert_radius(result, 1.);
    assert!(app.document.is_selected(id));
    assert!(app.document.is_selected(peer));
    assert!(
        app.document
            .object(peer)
            .unwrap()
            .geometry_snapshot()
            .shares_storage_with(&peer_geometry)
    );
    enter(&mut app, "Undo");

    enter(&mut app, "FilletCorners");
    enter(&mut app, "0.075cm");
    assert_radius(app.document.object(id).unwrap().geometry(), 0.75);
    assert_eq!(app.document.undo_label(), Some("FilletCorners"));
    enter(&mut app, "Undo");
    enter(&mut app, "FilletCorners");
    enter(&mut app, "w0,0,0");
    enter(&mut app, "");
    assert!(app.fillet_corners_session.is_some());
    enter(&mut app, "w0,0,3");
    assert_radius(app.document.object(id).unwrap().geometry(), 0.75);
    enter(&mut app, "Undo");

    enter(&mut app, "FilletCorners");
    enter(&mut app, "w0,0,0");
    enter(&mut app, "2");
    assert!(app.fillet_corners_session.is_some());
    enter(&mut app, "rw0,0,3");
    assert_radius(app.document.object(id).unwrap().geometry(), 2.);
    assert!(app.command_line_idle());
}

#[test]
fn fillet_corners_postselection_and_preview_run_through_the_viewport() {
    let mut app = test_app();
    let id = source(&mut app);
    app.commands
        .execute(&mut app.document, "Point -4,3,0")
        .unwrap();
    app.document.clear_history().unwrap();
    app.viewports[0] = Viewport::new(ViewKind::Top);
    app.maximized_viewport = Some(0);
    enter(&mut app, "FilletCorners");
    let context = egui::Context::default();
    let frame = |app: &mut VibocerosApp, events| {
        let output = context.run_ui(
            egui::RawInput {
                screen_rect: Some(egui::Rect::from_min_size(
                    egui::Pos2::ZERO,
                    egui::vec2(1000., 800.),
                )),
                events,
                ..Default::default()
            },
            |ui| app.show_frame(ui, None),
        );
        let segments = output
            .shapes
            .iter()
            .filter(|shape| {
                matches!(shape.shape, egui::Shape::LineSegment { stroke, .. }
                if stroke.color == egui::Color32::from_rgb(20, 115, 190))
            })
            .count();
        output.drop_without_applying_deltas();
        segments
    };
    assert_eq!(frame(&mut app, vec![]), 0);
    let click = |app: &mut VibocerosApp, pos| {
        frame(app, vec![egui::Event::PointerMoved(pos)]);
        for pressed in [true, false] {
            frame(
                app,
                vec![egui::Event::PointerButton {
                    pos,
                    button: egui::PointerButton::Primary,
                    pressed,
                    modifiers: egui::Modifiers::NONE,
                }],
            );
        }
    };
    let pixel = |app: &VibocerosApp, point| {
        let view = &app.viewports[0];
        view.project(point, view.screen_rect().unwrap()).unwrap()
    };
    let unrelated = pixel(&app, point(-4., 3., 0.));
    click(&mut app, unrelated);
    assert_eq!(app.document.selected_object_count(), 0);
    frame(&mut app, vec![]);
    let curve = pixel(&app, point(6., 3., 0.));
    click(&mut app, curve);
    assert_eq!(
        app.document.selected_object_ids().collect::<Vec<_>>(),
        vec![id],
        "{:?}",
        app.command_log
    );
    enter(&mut app, "");
    frame(&mut app, vec![]);
    let first = pixel(&app, point(0., 2., 0.));
    click(&mut app, first);
    assert!(app.viewport_drafting_input(false, false).anchor.is_some());
    frame(&mut app, vec![]);
    let target = pixel(&app, point(1., 2., 0.));
    frame(&mut app, vec![egui::Event::PointerMoved(target)]);
    assert!(frame(&mut app, vec![]) > 2);
    let preview = app.fillet_corners_session.as_ref().unwrap().preview()[0].clone();
    assert!(!app.document.can_undo());
    assert!(matches!(
        app.document.object(id).unwrap().geometry(),
        Geometry::Polyline(_)
    ));
    assert_eq!(app.document.objects().len(), 2);
    click(&mut app, target);
    assert_eq!(
        app.document.object(id).unwrap().geometry(),
        &Geometry::PolyCurve(preview)
    );
    assert!(app.command_line_idle());
    assert!(!app.document.is_selected(id));
    enter(&mut app, "Undo");
    assert!(matches!(
        app.document.object(id).unwrap().geometry(),
        Geometry::Polyline(_)
    ));
    assert!(!app.document.is_selected(id));
    enter(&mut app, "Redo");
    assert!(!app.document.is_selected(id));
}

#[test]
fn fillet_corners_invalid_hover_and_cancel_preserve_sources_and_redo() {
    let mut app = test_app();
    let id = source(&mut app);
    app.document
        .add_geometry(Geometry::Point(point(9., 9., 9.)))
        .unwrap();
    app.document.undo().unwrap();
    enter(&mut app, "FilletCorners");
    enter(&mut app, "SelAll");
    enter(&mut app, "");
    enter(&mut app, "w0,0,0");
    assert!(app.update_fillet_corners_preview(Some(point(2., 0., 0.))));
    assert_eq!(
        app.fillet_corners_session.as_ref().unwrap().preview().len(),
        1
    );
    assert!(!app.update_fillet_corners_preview(Some(point(0., 2., 0.))));
    assert!(app.update_fillet_corners_preview(Some(point(12., 0., 0.))));
    assert!(
        app.fillet_corners_session
            .as_ref()
            .unwrap()
            .preview()
            .is_empty()
    );
    enter(&mut app, "w12,0,0");
    assert!(app.fillet_corners_session.is_some());
    assert!(matches!(
        app.document.object(id).unwrap().geometry(),
        Geometry::Polyline(_)
    ));
    app.cancel_current_prompt_or_selection();
    assert!(app.command_line_idle());
    assert!(!app.document.is_selected(id));
    assert!(app.document.can_redo());
}
