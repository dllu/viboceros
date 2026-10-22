use super::*;

#[test]
fn viewport_group_pick_moves_locked_peer_without_following_its_other_group() {
    let mut app = test_app();
    let ids = (0..3)
        .map(|i| {
            app.document
                .add_geometry(Geometry::Point(point(i as f64, 0.0, 0.0)))
                .unwrap()
        })
        .collect::<Vec<_>>();
    app.document.add_group(None, [ids[0], ids[1]]).unwrap();
    app.document.add_group(None, [ids[1], ids[2]]).unwrap();
    app.document.set_objects_locked([ids[1]], true).unwrap();
    app.apply_selection_click(SelectionClick {
        object_id: Some(ids[0]),
        mode: viboceros_document::SelectionMode::Replace,
    });
    assert_eq!(
        app.document.selected_object_ids().collect::<Vec<_>>(),
        ids[..2]
    );
    app.execute_command("Move 0,0,0 0,1,0");
    for (i, id) in ids.iter().enumerate() {
        assert_eq!(
            app.document.object(*id).unwrap().geometry(),
            &Geometry::Point(point(i as f64, if i < 2 { 1.0 } else { 0.0 }, 0.0))
        );
    }
    assert!(
        app.document
            .object(ids[1])
            .unwrap()
            .attributes()
            .is_locked()
    );
    assert_eq!(app.document.undo_label(), Some("Move"));
    app.execute_command("Undo");
    assert_eq!(
        app.document.selected_object_ids().collect::<Vec<_>>(),
        ids[..2]
    );
    app.execute_command("Redo");
    assert_eq!(
        app.document.object(ids[1]).unwrap().geometry(),
        &Geometry::Point(point(1.0, 1.0, 0.0))
    );
}

#[test]
fn viewport_clicks_select_and_empty_clicks_clear() {
    let mut app = test_app();
    let object = app
        .document
        .add_geometry(Geometry::Point(point(1.0, 2.0, 0.0)))
        .unwrap();
    app.apply_selection_click(SelectionClick {
        object_id: Some(object),
        mode: viboceros_document::SelectionMode::Replace,
    });
    assert!(app.document.is_selected(object));

    app.apply_selection_click(SelectionClick {
        object_id: None,
        mode: viboceros_document::SelectionMode::Add,
    });
    assert!(app.document.is_selected(object));
    app.apply_selection_click(SelectionClick {
        object_id: None,
        mode: viboceros_document::SelectionMode::Replace,
    });
    assert_eq!(app.document.selected_object_count(), 0);
}

#[test]
fn selection_menu_defers_selection_until_a_choice_is_accepted() {
    let mut app = test_app();
    let first = app
        .document
        .add_geometry(Geometry::Point(point(0.0, 0.0, 0.0)))
        .unwrap();
    let second = app
        .document
        .add_geometry(Geometry::Point(point(0.0, 0.0, 0.0)))
        .unwrap();
    assert!(app.handle_viewport_action(ViewportOutput {
        selection_choice: Some(SelectionChoice {
            object_ids: vec![first, second],
            mode: viboceros_document::SelectionMode::Replace,
            pointer: egui::Pos2::new(40.0, 50.0),
            viewport: 2,
        }),
        ..Default::default()
    }));
    assert_eq!(app.document.selected_object_count(), 0);
    let menu = app.selection_menu.as_mut().unwrap();
    assert!(menu.is_original_pick(2, egui::Pos2::new(44.0, 50.0)));
    assert!(!menu.is_original_pick(1, egui::Pos2::new(40.0, 50.0)));
    menu.cycle();
    let click = menu.highlighted_click();
    assert_eq!(click.object_id, Some(second));
    app.selection_menu = None;
    app.apply_selection_click(click);
    assert_eq!(
        app.document.selected_object_ids().collect::<Vec<_>>(),
        vec![second]
    );
}

#[test]
fn layer_sidebar_crud_is_atomic_and_protects_nonempty_layers() {
    let mut app = test_app();
    let default = app.document.current_layer_id();
    app.sidebar.set_new_layer_name("Construction");
    app.apply_sidebar_action(SidebarAction::AddLayer {
        name: "Construction".to_owned(),
    });
    let construction = app.document.layer_by_name("Construction").unwrap().id();
    assert_eq!(app.document.current_layer_id(), construction);
    assert_eq!(
        app.document.layer(construction).unwrap().color(),
        suggested_layer_color(1)
    );
    assert!(app.sidebar.new_layer_name().is_empty());
    assert_eq!(app.document.undo_label(), Some("Add layer"));

    app.document.undo().unwrap();
    assert!(app.document.layer(construction).is_none());
    assert_eq!(app.document.current_layer_id(), default);
    app.document.redo().unwrap();
    assert_eq!(app.document.current_layer_id(), construction);

    let edited_color = ColorRgb::new(12, 34, 56);
    app.apply_sidebar_action(SidebarAction::EditLayer {
        id: construction,
        old_name: "Construction".to_owned(),
        name: "Reference".to_owned(),
        color: edited_color,
    });
    let edited = app.document.layer(construction).unwrap();
    assert_eq!(edited.name(), "Reference");
    assert_eq!(edited.color(), edited_color);
    assert_eq!(app.document.undo_label(), Some("Edit layer"));

    app.document.undo().unwrap();
    let original = app.document.layer(construction).unwrap();
    assert_eq!(original.name(), "Construction");
    assert_eq!(original.color(), suggested_layer_color(1));
    app.document.redo().unwrap();
    let edited = app.document.layer(construction).unwrap();
    assert_eq!(edited.name(), "Reference");
    assert_eq!(edited.color(), edited_color);

    app.document
        .add_geometry(Geometry::Point(point(1.0, 2.0, 0.0)))
        .unwrap();
    app.apply_sidebar_action(SidebarAction::SetCurrent {
        id: default,
        name: "Default".to_owned(),
    });
    app.apply_sidebar_action(SidebarAction::DeleteLayer {
        id: construction,
        name: "Reference".to_owned(),
    });
    assert!(app.document.layer(construction).is_some());
    assert!(app.command_log.back().unwrap().contains("contains objects"));

    app.apply_sidebar_action(SidebarAction::AddLayer {
        name: "Empty".to_owned(),
    });
    let empty = app.document.layer_by_name("Empty").unwrap().id();
    app.apply_sidebar_action(SidebarAction::SetCurrent {
        id: default,
        name: "Default".to_owned(),
    });
    app.apply_sidebar_action(SidebarAction::DeleteLayer {
        id: empty,
        name: "Empty".to_owned(),
    });
    assert!(app.document.layer(empty).is_none());
    assert_eq!(app.document.undo_label(), Some("Delete layer"));
    app.document.undo().unwrap();
    assert_eq!(app.document.layer(empty).unwrap().name(), "Empty");
}
