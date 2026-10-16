use super::*;
fn enter(app: &mut VibocerosApp, s: &str) {
    app.command_input = s.into();
    app.run_command();
}
#[test]
fn curvature_graph_picker_options_persist_and_display_controls_leave_active_drawing_intact() {
    let mut app = test_app();
    enter(&mut app, "Circle 0,0,0 2");
    enter(&mut app, "SelNone");
    enter(&mut app, "CurvatureGraph Scale=102 Density=2");
    assert!(app.object_prompt.is_some());
    enter(&mut app, "SelAll");
    enter(&mut app, "");
    assert!(app.object_prompt.is_none());
    let a = app
        .commands
        .curvature_graph_view(&app.document)
        .unwrap()
        .unwrap();
    assert_eq!(a.options.scale, 102);
    assert_eq!(a.options.density, 2);
    enter(&mut app, "Line");
    assert!(app.active_command.is_some());
    let before = app.document.undo_label().map(str::to_owned);
    app.set_curvature_graph_display("CurvatureGraph Scale=100 Color=1,2,3");
    assert!(app.active_command.is_some());
    assert_eq!(app.document.undo_label(), before.as_deref());
    assert!(std::sync::Arc::ptr_eq(
        &a.graphs,
        &app.commands
            .curvature_graph_view(&app.document)
            .unwrap()
            .unwrap()
            .graphs
    ));
    enter(&mut app, "0,0,0");
    enter(&mut app, "3,0,0");
    assert_eq!(app.document.objects().len(), 2);
    assert!(
        app.commands
            .curvature_graph_view(&app.document)
            .unwrap()
            .is_some()
    );
    enter(&mut app, "CurvatureGraphOff");
    assert!(
        app.commands
            .curvature_graph_view(&app.document)
            .unwrap()
            .is_none()
    );
}
#[test]
fn curvature_graph_add_remove_getters_and_cancel_preserve_prior_display() {
    let mut app = test_app();
    enter(&mut app, "Circle 0,0,0 2");
    enter(&mut app, "SelAll");
    enter(&mut app, "CurvatureGraph");
    let a = app
        .commands
        .curvature_graph_view(&app.document)
        .unwrap()
        .unwrap();
    enter(&mut app, "SelNone");
    enter(&mut app, "CurvatureGraph Add");
    assert!(app.object_prompt.is_some());
    app.cancel_current_prompt_or_selection();
    assert!(std::sync::Arc::ptr_eq(
        &a.graphs,
        &app.commands
            .curvature_graph_view(&app.document)
            .unwrap()
            .unwrap()
            .graphs
    ));
    enter(&mut app, "Line 4,0,0 5,1,0");
    enter(&mut app, "SelNone");
    enter(&mut app, "CurvatureGraph Add");
    enter(&mut app, "SelAll");
    enter(&mut app, "");
    assert_eq!(
        app.commands
            .curvature_graph_view(&app.document)
            .unwrap()
            .unwrap()
            .sources,
        2
    );
    enter(&mut app, "CurvatureGraph Remove");
    enter(&mut app, "SelAll");
    enter(&mut app, "");
    assert_eq!(
        app.commands
            .curvature_graph_view(&app.document)
            .unwrap()
            .unwrap()
            .sources,
        0
    );
}
