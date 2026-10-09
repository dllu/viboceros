use super::*;

#[test]
fn command_first_explode_block_picks_instances_and_accepts_group_output() {
    let mut app = test_app();
    let source = source(&mut app);
    app.document
        .select_object(source, SelectionMode::Replace)
        .unwrap();
    enter(&mut app, "Block 10,20,30 part");
    let root = app.document.objects().next().unwrap().id();
    enter(&mut app, "ExplodeBlock");
    assert_eq!(
        app.object_prompt.as_ref().unwrap().description.filter,
        viboceros_command::ObjectSelectionFilter::Blocks
    );
    app.select_prompt_objects([root], SelectionMode::Add);
    enter(&mut app, "GroupOutput=Yes");
    enter(&mut app, "");
    assert!(app.object_prompt.is_none());
    assert!(app.document.object(root).is_none());
    let output = app.document.objects().next().unwrap();
    assert_eq!(output.geometry(), &Geometry::Point(p(11., 22., 33.)));
    assert!(output.top_group().is_some());
    enter(&mut app, "Undo");
    assert!(app.document.object(root).is_some());
}

#[test]
fn all_blocks_action_finishes_without_source_picking_and_cancellation_does_not_edit() {
    let mut app = test_app();
    let source = source(&mut app);
    app.document
        .select_object(source, SelectionMode::Replace)
        .unwrap();
    enter(&mut app, "Block 10,20,30 part");
    let before = app.document.objects().cloned().collect::<Vec<_>>();
    enter(&mut app, "ExplodeBlock");
    app.cancel_interactive_command(true);
    assert_eq!(app.document.objects().cloned().collect::<Vec<_>>(), before);
    enter(&mut app, "ExplodeBlock");
    enter(&mut app, "AllBlocks");
    assert!(app.object_prompt.is_none());
    assert!(
        app.document
            .objects()
            .all(|object| !matches!(object.geometry(), Geometry::BlockInstance(_)))
    );
}

#[test]
fn quoted_names_containing_option_delimiters_work_in_starters_and_complete_scripts() {
    let mut app = test_app();
    let source = source(&mut app);
    app.document
        .select_object(source, SelectionMode::Replace)
        .unwrap();
    enter(&mut app, "Block 10,20,30 \"Part=A\"");
    enter(&mut app, "Insert \"Part=A\"");
    assert_eq!(
        app.active_command,
        Some(InteractiveCommand::Insert { has_name: true })
    );
    enter(&mut app, "0,0,0");
    assert_eq!(app.document.objects().len(), 2);
    enter(&mut app, "Insert \"Part=A\" 5,0,0");
    assert_eq!(app.document.objects().len(), 3);
}
fn p(x: f64, y: f64, z: f64) -> Point3 {
    Point3::try_new(x, y, z).unwrap()
}
fn enter(app: &mut VibocerosApp, input: &str) {
    app.command_input = input.into();
    app.run_command();
}
fn source(app: &mut VibocerosApp) -> ObjectId {
    enter(app, "Point 11,22,33");
    app.document.objects().next().unwrap().id()
}

#[test]
fn preselected_creation_gets_base_and_quoted_name_then_insertion_gets_name_options_and_point() {
    let mut app = test_app();
    let source = source(&mut app);
    app.document
        .select_object(source, SelectionMode::Replace)
        .unwrap();
    enter(&mut app, "Block");
    assert_eq!(
        app.active_command,
        Some(InteractiveCommand::Block { base: None })
    );
    assert!(app.accept_drafting_point(p(10., 20., 30.)));
    enter(&mut app, "\"Part A\"");
    assert!(app.active_command.is_none());
    assert!(app.document.object(source).is_none());
    let root = app.document.objects().next().unwrap().id();
    enter(&mut app, "Insert");
    enter(&mut app, "\"Part A\"");
    enter(&mut app, "Scale=2,3,4 Rotation=90");
    assert!(app.accept_drafting_point(p(10., 20., 30.)));
    let inserted = app.document.objects().last().unwrap();
    assert_ne!(inserted.id(), root);
    let Geometry::BlockInstance(instance) = inserted.geometry() else {
        panic!()
    };
    let Geometry::Point(point) = &*instance.members()[0].geometry else {
        panic!()
    };
    assert!(point.distance_to(p(4., 22., 42.)).unwrap() < 1e-12);
    enter(&mut app, "Undo");
    assert_eq!(app.document.objects().len(), 1);
    enter(&mut app, "Redo");
    assert_eq!(app.document.objects().len(), 2);
}

#[test]
fn command_first_creation_selects_sources_before_picking_the_base() {
    let mut app = test_app();
    let source = source(&mut app);
    enter(&mut app, "Block");
    assert!(app.object_prompt.is_some());
    app.select_prompt_objects([source], SelectionMode::Add);
    enter(&mut app, "");
    assert_eq!(
        app.active_command,
        Some(InteractiveCommand::Block { base: None })
    );
    enter(&mut app, "10,20,30");
    enter(&mut app, "part");
    assert!(matches!(
        app.document.objects().next().unwrap().geometry(),
        Geometry::BlockInstance(_)
    ));
    assert_eq!(app.document.undo_label(), Some("Block"));
}

#[test]
fn cancellation_unknown_names_and_invalid_insert_options_preserve_model_and_redo() {
    let mut app = test_app();
    let source = source(&mut app);
    app.document
        .select_object(source, SelectionMode::Replace)
        .unwrap();
    let before = app.document.objects().cloned().collect::<Vec<_>>();
    let undo = app.document.undo_label().map(str::to_owned);
    enter(&mut app, "Block");
    app.accept_drafting_point(p(10., 20., 30.));
    app.cancel_interactive_command(true);
    assert_eq!(app.document.objects().cloned().collect::<Vec<_>>(), before);
    assert_eq!(app.document.undo_label(), undo.as_deref());
    enter(&mut app, "Block 10,20,30 part");
    enter(&mut app, "Point 4,0,0");
    enter(&mut app, "Undo");
    let before = format!("{:?}", app.document);
    enter(&mut app, "Insert");
    enter(&mut app, "unknown");
    assert_eq!(
        app.active_command,
        Some(InteractiveCommand::Insert { has_name: false })
    );
    enter(&mut app, "part");
    enter(&mut app, "Scale=0");
    assert!(app.block_session.is_some());
    app.cancel_interactive_command(true);
    assert_eq!(format!("{:?}", app.document), before);
    enter(&mut app, "Redo");
    assert_eq!(app.document.objects().len(), 2);
}

#[test]
fn typed_base_and_definition_starters_keep_world_axis_rotation_options() {
    let mut app = test_app();
    let source = source(&mut app);
    app.document
        .select_object(source, SelectionMode::Replace)
        .unwrap();
    enter(&mut app, "Block 10,20,30");
    enter(&mut app, "part");
    enter(&mut app, "Insert part Scale=-2,3,4 Rotation=90 Axis=0,0,1");
    assert_eq!(
        app.active_command,
        Some(InteractiveCommand::Insert { has_name: true })
    );
    enter(&mut app, "10,20,30");
    let Geometry::BlockInstance(instance) = app.document.objects().last().unwrap().geometry()
    else {
        panic!()
    };
    let Geometry::Point(point) = &*instance.members()[0].geometry else {
        panic!()
    };
    assert!(point.distance_to(p(4., 18., 42.)).unwrap() < 1e-12);
}
