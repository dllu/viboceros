use super::super::arc_blend::{click, frame};
use super::*;
use viboceros_command::blend_curve::Handle;
fn setup() -> (VibocerosApp, egui::Context, egui::Rect, ObjectId) {
    let data = native::read("blend_curve_closed_single");
    let value = &data["results"][0]["value"];
    let mut app = test_app();
    app.document = edges::setup(value);
    app.viewports[0] = Viewport::new(ViewKind::Top);
    app.grid_snap = false;
    app.osnap = false;
    app.smart_track = false;
    let id = reference_inputs::guide(&app.document, 0);
    let context = egui::Context::default();
    frame(&mut app, &context, vec![]);
    app.viewports[0].zoom_out(0.5).unwrap();
    let (_, rect) = frame(&mut app, &context, vec![]);
    enter(&mut app, "BlendCrv");
    for world in [
        Point3::try_new(0., 5., 0.).unwrap(),
        Point3::try_new(10.25, 3., 0.).unwrap(),
    ] {
        let position = app.viewports[0].project(world, rect).unwrap();
        let output = click(&mut app, &context, position);
        app.handle_viewport_action(output);
    }
    assert!(
        app.blend_curve_session.as_ref().unwrap().input.ready(),
        "{:?}",
        app.command_log
    );
    (app, context, rect, id)
}
fn controls(app: &VibocerosApp) -> Vec<Point3> {
    app.blend_curve_session
        .as_ref()
        .unwrap()
        .input
        .preview()
        .unwrap()
        .control_points()
        .iter()
        .map(|p| p.point())
        .collect()
}
#[test]
fn windowless_closed_source_picks_contact_cancel_and_flip_keep_one_history_entry() {
    let (mut app, _, _, id) = setup();
    let parent = app.document.object(id).unwrap().clone();
    let before = controls(&app);
    assert!(
        before[0]
            .distance_to(Point3::try_new(0., 5., 0.).unwrap())
            .unwrap()
            < 0.1
    );
    assert!(
        !app.blend_curve_session
            .as_ref()
            .unwrap()
            .input
            .preferences()
            .pick_point
    );
    app.select_blend_curve_handle(Handle::Start);
    let display = app
        .blend_curve_session
        .as_ref()
        .unwrap()
        .cursor_curve()
        .unwrap();
    let t = display
        .closest_parameter(
            Point3::try_new(-5., 0., 0.).unwrap(),
            app.document.tolerance(),
        )
        .unwrap();
    app.hover_blend_curve_parameter(t);
    let session = app.blend_curve_session.as_ref().unwrap();
    assert_eq!(controls(&app), before);
    assert_ne!(session.preview_input().preview(), session.input.preview());
    enter(&mut app, "Cancel");
    assert_eq!(controls(&app), before);
    app.select_blend_curve_handle(Handle::Start);
    enter(&mut app, "w-5,0,0");
    let moved = controls(&app);
    assert!(
        moved[0]
            .distance_to(Point3::try_new(-5., 0., 0.).unwrap())
            .unwrap()
            < 1e-10
    );
    assert!(moved[1].y() > moved[0].y());
    let length = moved[0].distance_to(moved[1]).unwrap();
    enter(&mut app, "Flip");
    app.pick_blend_curve_at(id, None, moved[0]);
    let flipped = controls(&app);
    assert!(flipped[0].distance_to(moved[0]).unwrap() < 1e-10);
    assert!(flipped[1].y() < flipped[0].y());
    assert!((flipped[0].distance_to(flipped[1]).unwrap() - length).abs() < 1e-10);
    assert_eq!(app.document.object(id).unwrap(), &parent);
    assert!(!app.document.can_undo());
    enter(&mut app, "");
    assert_eq!(app.document.undo_label(), Some("BlendCrv"));
    app.document.undo().unwrap();
    assert!(!app.document.can_undo());
    assert_eq!(app.document.object(id).unwrap(), &parent);
    app.document.redo().unwrap();
    assert_eq!(app.document.object(id).unwrap(), &parent);
}
#[test]
fn a_closed_source_replaced_by_an_open_curve_resets_and_restores_natural_end_picking() {
    let (mut app, _, _, id) = setup();
    app.select_blend_curve_handle(Handle::Start);
    app.document
        .replace_object_geometries([(
            id,
            Geometry::Line(
                viboceros_geometry::LineSegment::try_new(
                    Point3::try_new(-5., 0., 0.).unwrap(),
                    Point3::try_new(5., 0., 0.).unwrap(),
                    app.document.tolerance(),
                )
                .unwrap(),
            ),
        )])
        .unwrap();
    app.validate_blend_curve();
    let input = &app.blend_curve_session.as_ref().unwrap().input;
    assert!(!input.ready());
    assert!(input.preview().is_none());
    assert!(input.active_handle().is_none());
    app.pick_blend_curve_at(id, None, Point3::try_new(-1., 0., 0.).unwrap());
    app.pick_blend_curve_at(
        reference_inputs::guide(&app.document, 1),
        None,
        Point3::try_new(10.25, 3., 0.).unwrap(),
    );
    assert_eq!(controls(&app)[0], Point3::try_new(-5., 0., 0.).unwrap());
}
