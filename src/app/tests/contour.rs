use super::*;
fn enter(app: &mut VibocerosApp, text: &str) {
    app.command_input = text.into();
    app.run_command();
}
#[test]
fn contour_getter_uses_units_and_admits_all_planes_as_one_undo_step() {
    let mut app = test_app();
    enter(&mut app, "Line -2,0,0 2,0,0");
    enter(&mut app, "SelAll");
    enter(&mut app, "Contour");
    app.accept_drafting_point(Point3::try_new(0., 0., 0.).unwrap());
    app.accept_drafting_point(Point3::try_new(1., 0., 0.).unwrap());
    enter(&mut app, "0");
    assert!(app.active_command.is_some());
    assert_eq!(app.document.objects().len(), 1);
    enter(&mut app, "1mm");
    assert!(app.active_command.is_none());
    assert_eq!(app.document.objects().len(), 6);
    enter(&mut app, "Undo");
    assert_eq!(app.document.objects().len(), 1);
}
#[test]
fn contour_cancel_and_degenerate_direction_do_not_change_the_model() {
    let mut app = test_app();
    enter(&mut app, "Line -2,0,0 2,0,0");
    enter(&mut app, "SelAll");
    let label = app.document.undo_label().map(str::to_owned);
    enter(&mut app, "Contour");
    enter(&mut app, "Range");
    let point = Point3::try_new(0., 0., 0.).unwrap();
    app.accept_drafting_point(point);
    app.accept_drafting_point(point);
    assert!(matches!(
        app.active_command,
        Some(InteractiveCommand::Contour {
            direction: None,
            ..
        })
    ));
    enter(&mut app, "Cancel");
    assert!(app.active_command.is_none());
    assert_eq!(app.document.objects().len(), 1);
    assert_eq!(app.document.undo_label(), label.as_deref());
}
#[test]
fn contour_command_first_selection_hands_off_options_and_point_input() {
    let mut app = test_app();
    enter(&mut app, "Line -2,0,0 2,0,0");
    enter(&mut app, "SelNone");
    enter(&mut app, "Contour Range=Yes GroupObjectsByContourPlane=Yes");
    assert!(app.object_prompt.is_some());
    enter(&mut app, "SelAll");
    enter(&mut app, "");
    assert!(app.object_prompt.is_none());
    assert!(matches!(
        app.active_command,
        Some(InteractiveCommand::Contour {
            options: viboceros_command::contour::ContourOptions {
                range: true,
                group: true,
                ..
            },
            ..
        })
    ));
    enter(&mut app, "0,0,0");
    enter(&mut app, "2,0,0");
    enter(&mut app, "0,0,0");
    enter(&mut app, "1,0,0");
    assert!(
        app.active_command.is_none(),
        "active={:?}; log={:?}",
        app.active_command,
        app.command_log
    );
    assert_eq!(app.document.objects().len(), 3);
    assert_eq!(app.document.groups().len(), 0);
}
