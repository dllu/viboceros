use super::*;

#[test]
fn in_place_block_edit_uses_the_shared_getter_and_model_commands() {
    let (mut app, root, _, _, _) = replacement_fixture();
    enter(&mut app, "BlockEdit");
    assert!(app.document.is_block_editing());
    assert!(!app.document.is_object_selectable(root));
    let members = app.document.block_edit_objects();
    app.document
        .select_objects_direct(members, SelectionMode::Replace)
        .unwrap();
    enter(&mut app, "Move 0,0,0 2,-3,4");
    enter(&mut app, "BlockEdit SaveAndClose");
    assert!(!app.document.is_block_editing());
    assert_eq!(app.document.undo_label(), Some("BlockEdit"));
    let Geometry::BlockInstance(i) = app.document.object(root).unwrap().geometry() else {
        panic!()
    };
    let Geometry::Point(point) = &*i.members()[0].geometry else {
        panic!()
    };
    assert_eq!(*point, p(3., -3., 4.));
}

#[test]
fn in_place_edit_cancel_and_file_rejection_preserve_the_model_and_redo() {
    let (mut app, root, _, _, _) = replacement_fixture();
    app.document
        .add_geometry(Geometry::Point(p(8., 9., 10.)))
        .unwrap();
    app.document.undo().unwrap();
    let before = app.document.objects().cloned().collect::<Vec<_>>();
    enter(&mut app, "BlockEdit");
    assert!(app.document.is_block_editing());
    enter(&mut app, "Point 3,4,5");
    let edited = app.document.objects().cloned().collect::<Vec<_>>();
    enter(&mut app, "Open /missing.3dm");
    assert!(app.document.is_block_editing());
    assert_eq!(app.document.objects().cloned().collect::<Vec<_>>(), edited);
    enter(&mut app, "BlockEdit DiscardAndCancel");
    assert_eq!(app.document.objects().cloned().collect::<Vec<_>>(), before);
    assert!(app.document.can_redo());
    assert!(app.document.object(root).is_some());
}

#[test]
fn reset_scale_mode_choice_persists_after_cancel_and_preselection_runs_immediately() {
    let (mut app, _, _, _, _) = replacement_fixture();
    enter(&mut app, "Insert Original 10,20,30 Scale=2,3,4");
    let root = app.document.objects().last().unwrap().id();
    app.document.clear_selection();
    let before = app.document.objects().cloned().collect::<Vec<_>>();
    enter(&mut app, "BlockResetScale");
    assert_eq!(
        app.object_prompt.as_ref().unwrap().description.filter,
        viboceros_command::ObjectSelectionFilter::Blocks
    );
    enter(&mut app, "Mode");
    enter(&mut app, "Automatic");
    assert_eq!(
        app.object_prompt.as_ref().unwrap().description.choices[0].value,
        "Automatic"
    );
    enter(&mut app, "Cancel");
    assert!(app.object_prompt.is_none());
    assert_eq!(app.document.objects().cloned().collect::<Vec<_>>(), before);
    app.document
        .select_objects_direct([root], SelectionMode::Replace)
        .unwrap();
    enter(&mut app, "BlockResetScale");
    assert!(app.object_prompt.is_none());
    let Geometry::BlockInstance(i) = app.document.object(root).unwrap().geometry() else {
        panic!()
    };
    assert_eq!(
        i.reference().transform().linear_rows(),
        [[3., 0., 0.], [0., 3., 0.], [0., 0., 3.]]
    );
    assert_eq!(app.document.undo_label(), Some("BlockResetScale"));
}

#[test]
fn reset_scale_postselection_excludes_other_types_and_accepts_one_mode() {
    let (mut app, _, _, _, point) = replacement_fixture();
    enter(&mut app, "Insert Original 10,20,30 Scale=2,3,4");
    let root = app.document.objects().last().unwrap().id();
    app.document.clear_selection();
    enter(&mut app, "BlockResetScale");
    app.select_prompt_objects([point], SelectionMode::Add);
    assert!(!app.document.is_selected(point));
    app.select_prompt_objects([root], SelectionMode::Add);
    enter(&mut app, "Mode=One");
    let before = app.document.objects().cloned().collect::<Vec<_>>();
    enter(&mut app, "Mode=Invalid");
    assert_eq!(app.document.objects().cloned().collect::<Vec<_>>(), before);
    enter(&mut app, "");
    assert!(app.object_prompt.is_none());
    let Geometry::BlockInstance(i) = app.document.object(root).unwrap().geometry() else {
        panic!()
    };
    assert_eq!(
        i.reference().transform().linear_rows(),
        [[1., 0., 0.], [0., 1., 0.], [0., 0., 1.]]
    );
    app.document.undo().unwrap();
    assert_eq!(app.document.objects().cloned().collect::<Vec<_>>(), before);
}

fn replacement_fixture() -> (VibocerosApp, ObjectId, ObjectId, ObjectId, ObjectId) {
    let mut app = test_app();
    for (name, x) in [("Original", 1.), ("Target = 10", 7.)] {
        let id = app
            .document
            .add_geometry(Geometry::Point(p(x, 0., 0.)))
            .unwrap();
        app.document
            .create_block_from_objects(name, p(0., 0., 0.), [id])
            .unwrap();
    }
    let roots = app.document.objects().map(|o| o.id()).collect::<Vec<_>>();
    enter(&mut app, "Insert Original 10,0,0");
    let peer = app.document.objects().last().unwrap().id();
    let decoy = app
        .document
        .add_geometry(Geometry::Point(p(12., 0., 0.)))
        .unwrap();
    app.document.add_group(None, [roots[1], decoy]).unwrap();
    app.document
        .select_objects_direct([roots[0]], SelectionMode::Replace)
        .unwrap();
    (app, roots[0], peer, roots[1], decoy)
}

#[test]
fn block_add_command_first_gets_target_then_multiple_sources_and_commits_once() {
    let (mut app, target, _, _, source) = replacement_fixture();
    app.document.clear_selection();
    enter(&mut app, "AddObjectsToBlock");
    assert!(app.picking_block_add_target());
    app.apply_selection_click(SelectionClick {
        object_id: Some(source),
        mode: SelectionMode::Replace,
    });
    assert!(app.picking_block_add_target());
    app.apply_selection_click(SelectionClick {
        object_id: Some(target),
        mode: SelectionMode::Replace,
    });
    assert!(!app.picking_block_add_target());
    let before = app.document.objects().cloned().collect::<Vec<_>>();
    app.apply_selection_click(SelectionClick {
        object_id: Some(source),
        mode: SelectionMode::Replace,
    });
    assert_eq!(app.document.objects().cloned().collect::<Vec<_>>(), before);
    enter(&mut app, "");
    assert!(!app.adding_to_block());
    assert!(app.document.object(source).is_none());
    assert_eq!(app.document.undo_label(), Some("AddObjectsToBlock"));
    app.document.undo().unwrap();
    assert_eq!(app.document.objects().cloned().collect::<Vec<_>>(), before);
}

#[test]
fn block_add_cancel_restores_selection_and_does_not_break_redo() {
    let (mut app, target, _, _, source) = replacement_fixture();
    let extra = app
        .document
        .add_geometry(Geometry::Point(p(18., 0., 0.)))
        .unwrap();
    app.document.undo().unwrap();
    let before = app.document.objects().cloned().collect::<Vec<_>>();
    let undo = app.document.undo_label().map(str::to_owned);
    enter(&mut app, "AddObjectsToBlock");
    assert!(app.adding_to_block());
    assert!(!app.picking_block_add_target());
    app.apply_selection_click(SelectionClick {
        object_id: Some(source),
        mode: SelectionMode::Replace,
    });
    enter(&mut app, "AddObjectsToBlock");
    assert!(app.adding_to_block());
    assert!(!app.picking_block_add_target());
    app.apply_selection_click(SelectionClick {
        object_id: Some(source),
        mode: SelectionMode::Replace,
    });
    enter(&mut app, "Cancel");
    assert!(!app.adding_to_block());
    assert_eq!(app.document.objects().cloned().collect::<Vec<_>>(), before);
    assert_eq!(
        app.document.selected_object_ids().collect::<Vec<_>>(),
        vec![target]
    );
    assert_eq!(app.document.undo_label(), undo.as_deref());
    app.document.redo().unwrap();
    assert!(app.document.object(extra).is_some());
}

#[test]
fn block_add_window_excludes_target_and_failure_keeps_the_getter_open() {
    let (mut app, target, _, _, source) = replacement_fixture();
    enter(&mut app, "AddObjectsToBlock");
    enter(&mut app, "");
    assert!(app.adding_to_block());
    app.apply_selection_window(SelectionWindow {
        object_ids: vec![target, source],
        mode: SelectionMode::Replace,
        crossing: true,
        inverted: false,
    });
    assert!(!app.document.is_selected(target));
    assert!(app.document.is_selected(source));
    app.document.set_objects_locked([target], true).unwrap();
    let before = app.document.objects().cloned().collect::<Vec<_>>();
    enter(&mut app, "");
    assert!(app.adding_to_block());
    assert_eq!(app.document.objects().cloned().collect::<Vec<_>>(), before);
    app.cancel_interactive_command(false);
}

#[test]
fn replacement_target_click_uses_definition_without_selecting_its_group() {
    let (mut app, source, peer, target, decoy) = replacement_fixture();
    let before = app.document.objects().cloned().collect::<Vec<_>>();
    let target_record = app.document.object(target).unwrap().clone();
    let target_definition = match target_record.geometry() {
        Geometry::BlockInstance(i) => i.reference().definition(),
        _ => panic!(),
    };
    enter(&mut app, "ReplaceBlock");
    assert!(app.picking_replace_block());
    assert_eq!(
        app.viewport_object_filter(),
        Some(viboceros_command::ObjectSelectionFilter::Blocks)
    );
    for id in [None, Some(decoy)] {
        app.apply_selection_click(SelectionClick {
            object_id: id,
            mode: SelectionMode::Replace,
        });
        assert_eq!(app.document.objects().cloned().collect::<Vec<_>>(), before);
        assert_eq!(
            app.document.selected_object_ids().collect::<Vec<_>>(),
            vec![source]
        );
    }
    app.apply_selection_window(SelectionWindow {
        object_ids: vec![target, decoy],
        mode: SelectionMode::Replace,
        crossing: true,
        inverted: false,
    });
    assert_eq!(
        app.document.selected_object_ids().collect::<Vec<_>>(),
        vec![source]
    );
    assert!(app.handle_viewport_action(ViewportOutput {
        selection_click: Some(SelectionClick {
            object_id: Some(target),
            mode: SelectionMode::Replace
        }),
        ..Default::default()
    }));
    assert!(!app.replacing_block());
    assert_eq!(app.document.object(target).unwrap(), &target_record);
    assert_eq!(
        app.document.selected_object_ids().collect::<Vec<_>>(),
        vec![source]
    );
    assert!(
        matches!(app.document.object(source).unwrap().geometry(), Geometry::BlockInstance(i) if i.reference().definition() == target_definition)
    );
    assert!(
        matches!(app.document.object(peer).unwrap().geometry(), Geometry::BlockInstance(i) if i.reference().definition() != target_definition)
    );
    assert_eq!(app.document.undo_label(), Some("ReplaceBlock"));
    app.document.undo().unwrap();
    assert_eq!(app.document.objects().cloned().collect::<Vec<_>>(), before);
}

#[test]
fn replacement_command_first_picking_transitions_to_a_read_only_target_getter() {
    let (mut app, source, _, target, _) = replacement_fixture();
    app.document.clear_selection();
    enter(&mut app, "ReplaceBlock");
    assert!(app.object_prompt.is_some());
    app.apply_selection_click(SelectionClick {
        object_id: Some(source),
        mode: SelectionMode::Replace,
    });
    enter(&mut app, "");
    assert!(app.object_prompt.is_none());
    assert!(app.picking_replace_block());
    app.apply_selection_click(SelectionClick {
        object_id: Some(target),
        mode: SelectionMode::Replace,
    });
    assert!(app.active_command.is_none());
    assert_eq!(app.document.undo_label(), Some("ReplaceBlock"));
}

#[test]
fn replacement_name_entry_blocks_picks_and_keeps_scope_after_invalid_names() {
    let (mut app, source, peer, target, _) = replacement_fixture();
    enter(&mut app, "ReplaceBlock");
    enter(&mut app, "All missing");
    enter(&mut app, "BlockDefinitionName");
    assert_eq!(app.viewport_object_filter(), None);
    let before = format!("{:?}", app.document);
    app.apply_selection_click(SelectionClick {
        object_id: Some(target),
        mode: SelectionMode::Replace,
    });
    assert_eq!(format!("{:?}", app.document), before);
    enter(&mut app, "missing");
    assert_eq!(format!("{:?}", app.document), before);
    enter(&mut app, "\"Target = 10\"");
    assert!(!app.replacing_block());
    let definition = app
        .document
        .block_definition_by_name("Target = 10")
        .unwrap()
        .id();
    for id in [source, peer] {
        assert!(
            matches!(app.document.object(id).unwrap().geometry(), Geometry::BlockInstance(i) if i.reference().definition() == definition)
        );
    }
}

#[test]
fn replacement_explicit_name_getter_accepts_scope_words_command_names_and_equals() {
    let (mut app, source, _, _, _) = replacement_fixture();
    let original = app
        .document
        .block_definition_by_name("Original")
        .unwrap()
        .id();
    for name in ["All", "None", "Delete", "Part=A"] {
        let target = app
            .document
            .duplicate_block_definition(original, name)
            .unwrap();
        enter(&mut app, "ReplaceBlock");
        enter(&mut app, "BlockDefinitionName");
        enter(&mut app, name);
        assert!(!app.replacing_block(), "name: {name}");
        assert!(
            matches!(app.document.object(source).unwrap().geometry(), Geometry::BlockInstance(i) if i.reference().definition() == target)
        );
        app.document.undo().unwrap();
    }
}

fn replacement_chooser_frame(
    app: &mut VibocerosApp,
    context: &egui::Context,
    events: Vec<egui::Event>,
) -> egui::FullOutput {
    context.run_ui(
        egui::RawInput {
            screen_rect: Some(egui::Rect::from_min_size(
                egui::Pos2::ZERO,
                egui::Vec2::new(800., 600.),
            )),
            events,
            ..Default::default()
        },
        |ui| app.show_replace_block_chooser(ui.ctx()),
    )
}

fn replacement_chooser_click(
    app: &mut VibocerosApp,
    context: &egui::Context,
    output: &egui::FullOutput,
    label: &str,
) {
    let pos = output
        .shapes
        .iter()
        .find_map(|s| match &s.shape {
            egui::Shape::Text(t) if t.galley.text() == label => {
                Some(t.pos + t.galley.rect.center().to_vec2())
            }
            _ => None,
        })
        .unwrap_or_else(|| panic!("missing chooser label {label}"));
    for pressed in [true, false] {
        replacement_chooser_frame(
            app,
            context,
            vec![
                egui::Event::PointerMoved(pos),
                egui::Event::PointerButton {
                    pos,
                    button: egui::PointerButton::Primary,
                    pressed,
                    modifiers: egui::Modifiers::NONE,
                },
            ],
        )
        .drop_without_applying_deltas();
    }
}

#[test]
fn replacement_definition_chooser_accepts_unused_definition_and_cancel_keeps_redo() {
    let (mut app, source, _, _, _) = replacement_fixture();
    let definition = app
        .document
        .block_definition_by_name("Original")
        .unwrap()
        .id();
    let unused = app
        .document
        .duplicate_block_definition(definition, "Unused2")
        .unwrap();
    app.document
        .duplicate_block_definition(definition, "Unused10")
        .unwrap();
    enter(&mut app, "ReplaceBlock");
    enter(&mut app, "SelectFromBlockDefinitionList");
    assert_eq!(app.viewport_object_filter(), None);
    let before = format!("{:?}", app.document);
    let context = egui::Context::default();
    replacement_chooser_frame(&mut app, &context, vec![]).drop_without_applying_deltas();
    let output = replacement_chooser_frame(&mut app, &context, vec![]);
    let labels = output
        .shapes
        .iter()
        .filter_map(|s| match &s.shape {
            egui::Shape::Text(t) => Some(t.galley.text()),
            _ => None,
        })
        .collect::<Vec<_>>();
    assert!(
        labels.iter().position(|s| *s == "Unused2").unwrap()
            < labels.iter().position(|s| *s == "Unused10").unwrap()
    );
    replacement_chooser_click(&mut app, &context, &output, "Unused2");
    assert_eq!(format!("{:?}", app.document), before);
    let output = replacement_chooser_frame(&mut app, &context, vec![]);
    replacement_chooser_click(&mut app, &context, &output, "Replace");
    assert!(!app.replacing_block());
    assert!(
        matches!(app.document.object(source).unwrap().geometry(), Geometry::BlockInstance(i) if i.reference().definition() == unused)
    );
    app.document.undo().unwrap();
    let before = format!("{:?}", app.document);
    enter(&mut app, "ReplaceBlock");
    enter(&mut app, "SelectFromBlockDefinitionList");
    let output = replacement_chooser_frame(&mut app, &context, vec![]);
    replacement_chooser_click(&mut app, &context, &output, "Cancel");
    assert!(!app.replacing_block());
    assert_eq!(format!("{:?}", app.document), before);
    app.document.redo().unwrap();
    assert!(
        matches!(app.document.object(source).unwrap().geometry(), Geometry::BlockInstance(i) if i.reference().definition() == unused)
    );
}

#[test]
fn replacement_chooser_reconciles_deleted_rows_and_rechecks_source_permissions() {
    let (mut app, source, _, _, _) = replacement_fixture();
    let original = app
        .document
        .block_definition_by_name("Original")
        .unwrap()
        .id();
    let unused = app
        .document
        .duplicate_block_definition(original, "Temporary")
        .unwrap();
    enter(&mut app, "ReplaceBlock");
    enter(&mut app, "SelectFromBlockDefinitionList");
    let context = egui::Context::default();
    replacement_chooser_frame(&mut app, &context, vec![]).drop_without_applying_deltas();
    let output = replacement_chooser_frame(&mut app, &context, vec![]);
    replacement_chooser_click(&mut app, &context, &output, "Temporary");
    app.document
        .delete_block_definition_and_instances(unused)
        .unwrap();
    let before = format!("{:?}", app.document);
    let output = replacement_chooser_frame(&mut app, &context, vec![]);
    replacement_chooser_click(&mut app, &context, &output, "Replace");
    assert!(app.replacing_block());
    assert_eq!(format!("{:?}", app.document), before);
    let output = replacement_chooser_frame(&mut app, &context, vec![]);
    replacement_chooser_click(&mut app, &context, &output, "Target = 10");
    app.document.set_objects_locked([source], true).unwrap();
    let before = format!("{:?}", app.document);
    let output = replacement_chooser_frame(&mut app, &context, vec![]);
    replacement_chooser_click(&mut app, &context, &output, "Replace");
    assert!(app.replacing_block());
    assert_eq!(format!("{:?}", app.document), before);
    app.cancel_interactive_command(false);
    assert_eq!(format!("{:?}", app.document), before);
}

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
fn unique_block_name_prompt_and_cancel_preserve_model_until_acceptance() {
    let mut app = test_app();
    let root = source(&mut app);
    app.document
        .select_objects_direct([root], SelectionMode::Replace)
        .unwrap();
    enter(&mut app, "Block 0,0,0 part");
    let root = app.document.objects().next().unwrap().id();
    app.document
        .select_objects_direct([root], SelectionMode::Replace)
        .unwrap();
    let before = format!("{:?}", app.document);
    enter(&mut app, "CreateUniqueBlock");
    assert_eq!(
        app.active_command,
        Some(InteractiveCommand::CreateUniqueBlock)
    );
    assert_eq!(format!("{:?}", app.document), before);
    app.cancel_interactive_command(false);
    assert_eq!(format!("{:?}", app.document), before);
    enter(&mut app, "CreateUniqueBlock");
    enter(&mut app, "\"Unique part\"");
    assert!(app.active_command.is_none());
    assert!(
        app.document
            .block_definition_by_name("Unique part")
            .is_some()
    );
    assert_eq!(app.document.undo_label(), Some("CreateUniqueBlock"));
}

#[test]
fn replacement_name_scope_prompt_and_cancel_preserve_sources_until_acceptance() {
    let mut app = test_app();
    for (name, x) in [("original", 1), ("target", 7)] {
        enter(&mut app, &format!("Point {x},0,0"));
        let id = app.document.objects().last().unwrap().id();
        app.document
            .select_objects_direct([id], SelectionMode::Replace)
            .unwrap();
        enter(&mut app, &format!("Block 0,0,0 {name}"));
    }
    let original = app.document.objects().next().unwrap().id();
    enter(&mut app, "Insert original 10,0,0");
    let chosen = app.document.objects().last().unwrap().id();
    app.document
        .select_objects_direct([chosen], SelectionMode::Replace)
        .unwrap();
    let before = format!("{:?}", app.document);
    enter(&mut app, "ReplaceBlock");
    assert_eq!(app.active_command, Some(InteractiveCommand::ReplaceBlock));
    enter(&mut app, "All");
    assert_eq!(format!("{:?}", app.document), before);
    app.cancel_interactive_command(false);
    assert_eq!(format!("{:?}", app.document), before);
    enter(&mut app, "ReplaceBlock");
    enter(&mut app, "All");
    enter(&mut app, "target");
    assert!(app.active_command.is_none());
    let target = app
        .document
        .block_definition_by_name("target")
        .unwrap()
        .id();
    assert!(
        matches!(app.document.object(original).unwrap().geometry(),Geometry::BlockInstance(i)if i.reference().definition()==target)
    );
    assert_eq!(app.document.undo_label(), Some("ReplaceBlock"));
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
