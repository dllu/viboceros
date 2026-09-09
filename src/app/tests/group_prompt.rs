use super::*;
use crate::app::group_prompt::GroupPrompt;
use viboceros_document::SelectionMode;

fn enter(app: &mut VibocerosApp, input: &str) {
    app.command_input = input.to_owned();
    app.run_command();
}

fn setup() -> (VibocerosApp, Vec<viboceros_document::ObjectId>) {
    let mut app = test_app();
    for input in ["Point 0,0,0", "Point 1,0,0", "Point 2,0,0"] {
        enter(&mut app, input);
    }
    let ids = app.document.objects().map(|o| o.id()).collect::<Vec<_>>();
    app.document
        .add_group(Some("Target assembly".into()), [ids[0]])
        .unwrap();
    (app, ids)
}

#[test]
fn bare_add_to_group_collects_sources_then_target_without_early_edits() {
    let (mut app, ids) = setup();
    let history = app.document.undo_label().map(str::to_owned);
    enter(&mut app, "AddToGroup");
    assert!(matches!(
        app.group_prompt,
        Some(GroupPrompt::Sources { target: None })
    ));
    enter(&mut app, "");
    assert!(matches!(
        app.group_prompt,
        Some(GroupPrompt::Sources { .. })
    ));
    for id in &ids[1..] {
        app.apply_selection_click(SelectionClick {
            object_id: Some(*id),
            mode: SelectionMode::Replace,
        });
    }
    assert_eq!(app.document.selected_object_count(), 2);
    app.apply_selection_window(SelectionWindow {
        object_ids: vec![ids[1]],
        mode: SelectionMode::Remove,
        crossing: false,
    });
    assert_eq!(
        app.document.selected_object_ids().collect::<Vec<_>>(),
        [ids[2]]
    );
    app.apply_selection_window(SelectionWindow {
        object_ids: vec![ids[1]],
        mode: SelectionMode::Replace,
        crossing: true,
    });
    assert_eq!(app.document.selected_object_count(), 2);
    assert_eq!(app.document.undo_label(), history.as_deref());
    enter(&mut app, "");
    assert_eq!(app.group_prompt, Some(GroupPrompt::Target));
    // Target-name entry must not accidentally change the source selection.
    app.apply_selection_click(SelectionClick {
        object_id: Some(ids[0]),
        mode: SelectionMode::Replace,
    });
    assert_eq!(
        app.document.selected_object_ids().collect::<Vec<_>>(),
        [ids[2], ids[1]]
    );
    enter(&mut app, "missing target");
    assert_eq!(app.group_prompt, Some(GroupPrompt::Target));
    assert_eq!(app.document.undo_label(), history.as_deref());
    enter(&mut app, "Target assembly");
    assert!(app.group_prompt.is_none());
    assert_eq!(app.document.selected_object_count(), 0);
    assert_eq!(
        app.document
            .group_by_name("Target assembly")
            .unwrap()
            .members()
            .len(),
        3
    );
    assert_eq!(app.document.undo_label(), Some("AddToGroup"));
    enter(&mut app, "Undo");
    assert_eq!(
        app.document
            .group_by_name("Target assembly")
            .unwrap()
            .members()
            .len(),
        1
    );
    enter(&mut app, "Redo");
    assert_eq!(
        app.document
            .group_by_name("Target assembly")
            .unwrap()
            .members()
            .len(),
        3
    );
}

#[test]
fn known_target_collects_sources_and_preselection_skips_source_prompt() {
    let (mut app, ids) = setup();
    enter(&mut app, "AddToGroup Target assembly");
    assert!(matches!(
        app.group_prompt,
        Some(GroupPrompt::Sources { target: Some(_) })
    ));
    app.select_group_prompt_objects([ids[1]], SelectionMode::Add);
    enter(&mut app, "");
    assert!(app.group_prompt.is_none());
    assert_eq!(
        app.document
            .group_by_name("Target assembly")
            .unwrap()
            .members()
            .len(),
        2
    );
    app.document
        .select_objects_direct([ids[2]], SelectionMode::Replace)
        .unwrap();
    enter(&mut app, "_AddToGroup");
    assert_eq!(app.group_prompt, Some(GroupPrompt::Target));
    enter(&mut app, "Target assembly");
    assert_eq!(
        app.document
            .group_by_name("Target assembly")
            .unwrap()
            .members()
            .len(),
        3
    );
}

#[test]
fn group_prompt_cancel_and_command_switch_do_not_create_memberships() {
    let (mut app, ids) = setup();
    for preselect in [false, true] {
        app.document.clear_selection();
        if preselect {
            app.document
                .select_objects_direct([ids[1]], SelectionMode::Replace)
                .unwrap();
        }
        let objects = app.document.objects().cloned().collect::<Vec<_>>();
        let history = app.document.undo_label().map(str::to_owned);
        enter(&mut app, "AddToGroup");
        app.cancel_interactive_command(true);
        assert!(app.group_prompt.is_none());
        assert_eq!(app.document.objects().cloned().collect::<Vec<_>>(), objects);
        assert_eq!(app.document.undo_label(), history.as_deref());
    }
    enter(&mut app, "AddToGroup");
    enter(&mut app, "Point 9,0,0");
    assert!(app.group_prompt.is_none());
    assert_eq!(app.document.objects().len(), 4);
    assert_eq!(
        app.document
            .group_by_name("Target assembly")
            .unwrap()
            .members()
            .len(),
        1
    );
}

#[test]
fn transparent_cplane_prompt_returns_to_group_target_entry() {
    let (mut app, ids) = setup();
    app.document
        .select_objects_direct([ids[1]], SelectionMode::Replace)
        .unwrap();
    enter(&mut app, "AddToGroup");
    enter(&mut app, "CPlane");
    assert!(app.plane_prompt.is_some());
    enter(&mut app, "");
    assert!(app.plane_prompt.is_none());
    assert_eq!(app.group_prompt, Some(GroupPrompt::Target));
    enter(&mut app, "Target assembly");
    assert_eq!(
        app.document
            .group_by_name("Target assembly")
            .unwrap()
            .members()
            .len(),
        2
    );
}

#[test]
fn group_sources_support_selectors_and_sidebar_actions_cancel_pending_input() {
    let (mut app, _) = setup();
    enter(&mut app, "AddToGroup");
    enter(&mut app, "SelAll");
    assert_eq!(app.document.selected_object_count(), 3);
    assert!(matches!(
        app.group_prompt,
        Some(GroupPrompt::Sources { .. })
    ));
    enter(&mut app, "SelNone");
    assert_eq!(app.document.selected_object_count(), 0);
    enter(&mut app, "SelAll");
    enter(&mut app, "");
    enter(&mut app, "SelNone");
    assert_eq!(app.document.selected_object_count(), 3);
    assert_eq!(app.group_prompt, Some(GroupPrompt::Target));
    app.apply_sidebar_action(SidebarAction::AddLayer {
        name: "new layer".into(),
    });
    assert!(app.group_prompt.is_none());
    assert_eq!(
        app.document
            .group_by_name("Target assembly")
            .unwrap()
            .members()
            .len(),
        1
    );
}
