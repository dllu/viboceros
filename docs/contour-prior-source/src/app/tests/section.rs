use super::*;
fn p(x: f64, y: f64, z: f64) -> Point3 {
    Point3::try_new(x, y, z).unwrap()
}
fn enter(app: &mut VibocerosApp, input: &str) {
    app.command_input = input.into();
    app.run_command();
}
#[test]
fn section_getter_accepts_two_points_repeats_planes_and_finishes_on_enter() {
    let mut app = test_app();
    enter(&mut app, "Line 0,-2,0 0,2,0");
    enter(&mut app, "SelAll");
    let source = app.document.objects().next().unwrap().id();
    enter(&mut app, "Section");
    assert!(matches!(
        app.active_command,
        Some(InteractiveCommand::Section { start: None, .. })
    ));
    app.accept_drafting_point(p(-1., 0., 0.));
    app.accept_drafting_point(p(1., 0., 0.));
    assert_eq!(app.document.objects().len(), 2);
    assert!(app.document.is_selected(source));
    assert!(matches!(
        app.active_command,
        Some(InteractiveCommand::Section { start: None, .. })
    ));
    enter(&mut app, "");
    assert!(app.active_command.is_none());
    enter(&mut app, "Undo");
    assert_eq!(app.document.objects().len(), 1);
}
#[test]
fn section_cancels_a_pending_plane_without_geometry_or_history_changes() {
    let mut app = test_app();
    enter(&mut app, "Line 0,-2,0 0,2,0");
    enter(&mut app, "SelAll");
    let before = app.document.objects().cloned().collect::<Vec<_>>();
    let label = app.document.undo_label().map(str::to_owned);
    enter(&mut app, "Section ExtendSection=No");
    app.accept_drafting_point(p(-1., 0., 0.));
    enter(&mut app, "Cancel");
    assert_eq!(app.document.objects().cloned().collect::<Vec<_>>(), before);
    assert_eq!(app.document.undo_label(), label.as_deref());
}
