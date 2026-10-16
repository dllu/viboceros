use super::*;

fn enter(app: &mut VibocerosApp, text: &str) {
    app.command_input = text.into();
    app.run_command();
}
fn setup() -> (VibocerosApp, ObjectId, ObjectId) {
    let mut app = test_app();
    enter(&mut app, "Line 0,0,0 0,10,0");
    let guide = app.document.objects().next().unwrap().id();
    enter(&mut app, "Point 1,0,0");
    let source = app
        .document
        .objects()
        .find(|o| o.id() != guide)
        .unwrap()
        .id();
    app.document
        .select_objects_direct([source], SelectionMode::Replace)
        .unwrap();
    app.document.clear_history().unwrap();
    (app, source, guide)
}
#[test]
fn orient_curve_repeated_copy_preview_and_one_undo_preserve_the_guide() {
    let (mut app, source, guide) = setup();
    let before = format!("{:?}", app.document);
    enter(&mut app, "OrientOnCrv Copy=Yes");
    enter(&mut app, "w0,0,0");
    assert!(app.picking_orient_curve());
    assert_eq!(
        app.viewport_object_filter(),
        Some(viboceros_command::ObjectSelectionFilter::Curves)
    );
    app.pick_orient_curve(Some(guide));
    let p = app.affine_preview().unwrap();
    let map = p
        .definition
        .transform_at(
            p.frame.unwrap(),
            point(0., 3., 0.),
            app.document.tolerance(),
        )
        .unwrap();
    assert!(
        map.transform_point(point(1., 0., 0.))
            .unwrap()
            .distance_to(point(0., 4., 0.))
            .unwrap()
            < 1e-12
    );
    assert_eq!(format!("{:?}", app.document), before);
    enter(&mut app, "w0,3,0");
    enter(&mut app, "w0,6,0");
    assert!(app.active_command.is_some());
    assert_eq!(app.document.objects().count(), 4);
    assert_eq!(
        app.document.object(source).unwrap().geometry(),
        &Geometry::Point(point(1., 0., 0.))
    );
    enter(&mut app, "");
    assert!(app.active_command.is_none());
    assert_eq!(app.document.undo().unwrap().as_deref(), Some("OrientOnCrv"));
    assert_eq!(app.document.objects().count(), 2);
    assert!(app.document.undo().unwrap().is_none());
    app.document.redo().unwrap();
    assert_eq!(app.document.objects().count(), 4);
}
#[test]
fn orient_curve_source_getter_on_curve_rotation_refs_and_cancel_work() {
    let (mut app, source, guide) = setup();
    app.document.clear_selection();
    enter(&mut app, "OrientOnCurve");
    assert!(app.object_prompt.is_some());
    enter(&mut app, &format!("SelID {source}"));
    enter(&mut app, "");
    enter(&mut app, "OnCurve");
    enter(&mut app, &format!("SelID {guide}"));
    enter(&mut app, "w0,2,0");
    enter(&mut app, "Perpendicular");
    enter(&mut app, "XFlip");
    enter(&mut app, "Rotate=Yes");
    enter(&mut app, "w0,5,0");
    assert!(app.affine_preview().is_some());
    enter(&mut app, "w1,5,0");
    enter(&mut app, "w0,5,1");
    assert!(app.active_command.is_none(), "{:?}", app.command_log);
    assert_eq!(app.document.objects().count(), 2);
    assert!(!app.document.is_selected(source));
    assert_eq!(app.document.undo_label(), Some("OrientOnCrv"));
    app.document.undo().unwrap();
    enter(&mut app, "OrientOnCrv");
    enter(&mut app, "Cancel");
    assert_eq!(app.document.undo_label(), None);
}
#[test]
fn orient_curve_rejects_options_and_noncurve_guides_without_mutation() {
    let (mut app, source, guide) = setup();
    let before = format!("{:?}", app.document);
    enter(&mut app, "OrientOnCrv");
    enter(&mut app, "w0,0,0");
    app.pick_orient_curve(Some(source));
    assert!(app.picking_orient_curve());
    app.pick_orient_curve(Some(guide));
    enter(&mut app, "XFlip");
    enter(&mut app, "Copy=Maybe");
    assert_eq!(format!("{:?}", app.document), before);
    enter(&mut app, "Cancel");
    assert!(app.document.undo().unwrap().is_none());
}

#[test]
fn orient_curve_freezes_the_base_plane_and_retains_rotation_defaults_between_commands() {
    use viboceros_command::construction_plane::WorldPlane;
    let (mut app, source, guide) = setup();
    app.commands
        .remember_pending_transform_scalar("OrientOnCrv", 45.);
    app.viewports[app.active_viewport].set_construction_plane(WorldPlane::Right.frame());
    enter(&mut app, "OrientOnCrv");
    enter(&mut app, "w0,0,0");
    app.viewports[app.active_viewport].set_construction_plane(WorldPlane::Top.frame());
    app.pick_orient_curve(Some(guide));
    enter(&mut app, "w0,3,0");
    assert_eq!(
        app.document.object(source).unwrap().geometry(),
        &Geometry::Point(point(0., 3., 1.))
    );
    assert_eq!(
        app.commands.transform_scalar_default("OrientOnCrv"),
        Some(45.)
    );
    app.document.undo().unwrap();
    enter(&mut app, "OrientOnCrv Perpendicular=Yes");
    enter(&mut app, "w0,0,0");
    app.pick_orient_curve(Some(guide));
    enter(&mut app, "Rotate=Yes");
    enter(&mut app, "w0,4,0");
    enter(&mut app, "NaN");
    assert!(app.active_command.is_some());
    enter(&mut app, "");
    let Geometry::Point(p) = app.document.object(source).unwrap().geometry() else {
        panic!("point")
    };
    let r = 0.5_f64.sqrt();
    assert!(p.distance_to(point(-r, 4., r)).unwrap() < 1e-12);
    assert!(app.active_command.is_none());
}
