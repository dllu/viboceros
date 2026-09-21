use super::*;
use crate::app::edge_commands::EdgePrompt;
use crate::viewport::EdgePick;

fn fixture(app: &mut VibocerosApp) -> EdgePick {
    let brep = viboceros_geometry::Brep::try_box(
        viboceros_command::CommandContext::default().construction_plane,
        [[0., 2.], [0., 3.], [0., 5.]],
        app.document.tolerance(),
    )
    .unwrap()
    .try_split_edges_at_parameters(&[(0, vec![0.25, 0.75, 1.75])], app.document.tolerance())
    .unwrap();
    let object = app.document.add_geometry(Geometry::Brep(brep)).unwrap();
    EdgePick { object, edge: 13 }
}

fn submit(app: &mut VibocerosApp, input: &str) {
    app.command_input = input.to_owned();
    app.run_command();
}

#[test]
fn edge_choice_edits_only_after_confirmation_and_one_undo_restores_the_source() {
    let mut app = test_app();
    let pick = fixture(&mut app);
    let before = app.document.object(pick.object).unwrap().clone();
    let history_before = app.document.undo_label().map(str::to_owned);
    app.document
        .select_objects_direct([pick.object], viboceros_document::SelectionMode::Replace)
        .unwrap();
    submit(&mut app, "MergeEdge");
    assert_eq!(app.document.selected_object_count(), 0);
    assert!(app.active_command.is_none());
    assert!(app.viewport_object_filter().is_none());
    assert!(app.handle_viewport_action(ViewportOutput {
        edge_click: Some(vec![pick]),
        ..Default::default()
    }));
    assert!(matches!(app.edge_prompt, Some(EdgePrompt::Choice(_))));
    assert_eq!(app.document.object(pick.object).unwrap(), &before);
    assert_eq!(app.document.undo_label(), history_before.as_deref());
    submit(&mut app, "Both");
    assert!(app.edge_prompt.is_none());
    assert_eq!(app.document.undo_label(), Some("MergeEdge"));
    let after = app.document.object(pick.object).unwrap().clone();
    let Geometry::Brep(brep) = after.geometry() else {
        panic!()
    };
    assert_eq!(brep.edges().len(), 13);
    submit(&mut app, "Undo");
    assert_eq!(app.document.object(pick.object).unwrap(), &before);
    assert_eq!(app.document.selected_object_count(), 0);
    submit(&mut app, "Redo");
    assert_eq!(app.document.object(pick.object).unwrap(), &after);
}

#[test]
fn ambiguity_invalid_choices_cancel_and_noops_cannot_commit_accidentally() {
    let mut app = test_app();
    let pick = fixture(&mut app);
    submit(&mut app, "Point 9,9,9");
    submit(&mut app, "Undo");
    let before = format!("{:?}", app.document);
    submit(&mut app, "MergeEdge");
    app.accept_edge_click(vec![EdgePick { edge: 1, ..pick }]);
    assert!(matches!(app.edge_prompt, Some(EdgePrompt::Pick(_))));
    app.accept_edge_click(vec![pick, EdgePick { edge: 12, ..pick }]);
    assert!(matches!(app.edge_prompt, Some(EdgePrompt::Ambiguous(..))));
    submit(&mut app, "0");
    submit(&mut app, "99");
    submit(&mut app, "1,2,3");
    assert!(matches!(app.edge_prompt, Some(EdgePrompt::Ambiguous(..))));
    submit(&mut app, "1");
    submit(&mut app, "Edge"); // Only EdgeA/EdgeB/Both/All are offered here.
    assert!(matches!(app.edge_prompt, Some(EdgePrompt::Choice(_))));
    submit(&mut app, "");
    assert!(app.edge_prompt.is_none());
    assert_eq!(format!("{:?}", app.document), before);
    submit(&mut app, "MergeEdge");
    app.accept_edge_click(vec![pick]);
    submit(&mut app, "Redo"); // A real new command cancels the menu first.
    assert!(app.edge_prompt.is_none());
    assert_eq!(app.document.objects().len(), 2);
}

#[test]
fn sidebar_changes_invalidate_pending_components_without_editing_hidden_geometry() {
    let mut app = test_app();
    let pick = fixture(&mut app);
    submit(&mut app, "MergeEdge");
    app.accept_edge_click(vec![pick]);
    app.document
        .set_objects_locked([pick.object], true)
        .unwrap();
    let before = format!("{:?}", app.document);
    submit(&mut app, "All");
    assert_eq!(format!("{:?}", app.document), before);
    assert!(matches!(app.edge_prompt, Some(EdgePrompt::Pick(_))));
}

#[test]
fn geometry_changes_while_an_ambiguity_menu_is_open_invalidate_its_indices() {
    let mut app = test_app();
    let pick = fixture(&mut app);
    submit(&mut app, "MergeEdge");
    app.accept_edge_click(vec![pick, EdgePick { edge: 12, ..pick }]);
    app.commands
        .execute(
            &mut app.document,
            &format!("MergeEdge {} 13 All", pick.object),
        )
        .unwrap();
    let before = format!("{:?}", app.document);
    submit(&mut app, "1");
    assert_eq!(format!("{:?}", app.document), before);
    assert!(matches!(app.edge_prompt, Some(EdgePrompt::Pick(_))));
}

#[test]
fn egui_buttons_highlight_ambiguous_edges_and_commit_the_chosen_merge() {
    let mut app = test_app();
    let pick = fixture(&mut app);
    let context = egui::Context::default();
    let frame = |app: &mut VibocerosApp, events| {
        context.run_ui(
            egui::RawInput {
                screen_rect: Some(egui::Rect::from_min_size(
                    egui::Pos2::ZERO,
                    egui::Vec2::new(800., 600.),
                )),
                events,
                ..Default::default()
            },
            |ui| app.show_edge_choices(ui),
        )
    };
    let label_center = |output: &egui::FullOutput, label: &str| {
        output
            .shapes
            .iter()
            .find_map(|shape| {
                if let egui::Shape::Text(text) = &shape.shape
                    && text.galley.job.text == label
                {
                    Some(text.pos + text.galley.rect.center().to_vec2())
                } else {
                    None
                }
            })
            .unwrap()
    };
    let button = |pos, pressed| egui::Event::PointerButton {
        pos,
        button: egui::PointerButton::Primary,
        pressed,
        modifiers: egui::Modifiers::NONE,
    };
    submit(&mut app, "MergeEdge");
    app.accept_edge_click(vec![pick, EdgePick { edge: 12, ..pick }]);
    let output = frame(&mut app, vec![]);
    let second = label_center(&output, "2");
    output.drop_without_applying_deltas();
    frame(&mut app, vec![egui::Event::PointerMoved(second)]).drop_without_applying_deltas();
    assert_eq!(
        app.edge_prompt.as_ref().unwrap().highlights(),
        vec![EdgePick { edge: 12, ..pick }]
    );
    frame(&mut app, vec![button(second, true)]).drop_without_applying_deltas();
    frame(&mut app, vec![button(second, false)]).drop_without_applying_deltas();
    let Some(EdgePrompt::Choice(selection)) = &app.edge_prompt else {
        panic!()
    };
    assert_eq!(selection.edge(), 12);
    let output = frame(&mut app, vec![]);
    let all = label_center(&output, "All");
    output.drop_without_applying_deltas();
    frame(
        &mut app,
        vec![egui::Event::PointerMoved(all), button(all, true)],
    )
    .drop_without_applying_deltas();
    frame(&mut app, vec![button(all, false)]).drop_without_applying_deltas();
    assert!(app.edge_prompt.is_none());
    assert_eq!(app.document.undo_label(), Some("MergeEdge"));
    let Geometry::Brep(brep) = app.document.object(pick.object).unwrap().geometry() else {
        panic!()
    };
    assert_eq!(brep.edges().len(), 12);
}
