use super::*;

#[test]
fn sweep_two_rail_closed_option_reviews_previews_and_commits() {
    let mut app = test_app();
    for input in [
        "Circle 0,0,0 1",
        "Circle 0,0,0 3",
        "Line 1,0,0 3,0,0",
        "Line 0,1,0 0,3,0",
    ] {
        enter(&mut app, input);
    }
    let before = app.document.objects().cloned().collect::<Vec<_>>();
    assert_eq!(before.len(), 4, "{:?}", app.command_log);
    app.document.clear_history().unwrap();
    app.viewports[0] = Viewport::new(ViewKind::Top);
    app.maximized_viewport = Some(0);
    let context = egui::Context::default();
    enter(&mut app, "Sweep2");
    for object in &before {
        app.apply_selection_click(SelectionClick {
            object_id: Some(object.id()),
            mode: SelectionMode::Add,
        });
    }
    enter(&mut app, "");
    assert!(app.command_hint().contains("Closed"));
    assert!(app.command_log.back().unwrap().contains("Closed=No"));
    let span = wait_for_draft(&mut app, &context);
    enter(&mut app, "Closed");
    assert!(app.command_hint().contains("Choose Yes or No"));
    enter(&mut app, "Yes");
    let cycle = wait_for_draft(&mut app, &context);
    assert_ne!(cycle, span);
    assert!(app.command_log.back().unwrap().contains("Closed=Yes"));
    enter(&mut app, "Undo");
    assert_eq!(wait_for_draft(&mut app, &context), span);
    enter(&mut app, "Closed=Yes");
    assert_eq!(wait_for_draft(&mut app, &context), cycle);
    assert_eq!(app.document.objects().cloned().collect::<Vec<_>>(), before);
    assert_eq!(app.document.undo_label(), None);

    enter(&mut app, "");
    assert!(app.command_line_idle(), "{:?}", app.command_log);
    let Geometry::Brep(brep) = app.document.objects().last().unwrap().geometry() else {
        panic!("Expected a Sweep2 B-rep");
    };
    assert_eq!(brep.faces().len(), 4);
    assert_eq!(
        brep.wireframe_curves(1, app.document.tolerance()).unwrap(),
        cycle
    );
    assert_eq!(app.document.undo_label(), Some("Sweep2"));
    enter(&mut app, "Undo");
    assert_eq!(app.document.objects().cloned().collect::<Vec<_>>(), before);
}
