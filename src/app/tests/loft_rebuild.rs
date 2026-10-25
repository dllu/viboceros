use super::*;

fn enter(app: &mut VibocerosApp, input: &str) {
    app.command_input = input.into();
    app.run_command();
}

#[test]
fn loft_rebuild_point_prompt_undo_and_model_history_are_independent() {
    let mut app = test_app();
    app.commands
        .execute(&mut app.document, "Circle 0,0,1 1")
        .unwrap();
    let id = app.document.objects().last().unwrap().id();
    app.document.clear_history().unwrap();
    let before = app.document.objects().cloned().collect::<Vec<_>>();
    enter(&mut app, "Loft");
    app.apply_selection_click(SelectionClick {
        object_id: Some(id),
        mode: SelectionMode::Replace,
    });
    enter(&mut app, "Rebuild");
    enter(&mut app, "6");
    enter(&mut app, "Undo");
    enter(&mut app, "RebuildCount");
    enter(&mut app, "8");
    enter(&mut app, "Rebuild=513");
    enter(&mut app, "StartPoint");
    enter(&mut app, "w0,0,-1");
    enter(&mut app, "Point");
    enter(&mut app, "w0,0,4");
    assert_eq!(app.document.objects().cloned().collect::<Vec<_>>(), before);
    assert_eq!(app.document.undo_label(), None);
    enter(&mut app, "");
    let output = app.document.objects().find(|o| o.id() != id).unwrap();
    let Geometry::Brep(brep) = output.geometry() else {
        panic!("Loft B-rep")
    };
    assert!(brep.is_solid(), "{:?}", app.command_log);
    assert_eq!(brep.faces()[0].surface().degree_v(), 3);
    assert_eq!(brep.faces()[0].surface().control_point_count_v(), 9);
    assert_eq!(app.document.undo_label(), Some("Loft"));
    let after = app.document.objects().cloned().collect::<Vec<_>>();
    enter(&mut app, "Undo");
    assert_eq!(app.document.objects().cloned().collect::<Vec<_>>(), before);
    enter(&mut app, "Redo");
    assert_eq!(app.document.objects().cloned().collect::<Vec<_>>(), after);
}
