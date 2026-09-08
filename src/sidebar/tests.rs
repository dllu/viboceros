use super::*;

#[test]
fn editor_conflicts_preserve_drafts_and_clear_when_resolved_or_undone() {
    let id = Document::default().current_layer_id();
    let mut editor = LayerEditor::new(id, "Original".into(), ColorRgb::BLACK);
    editor.name = "My name".into();
    editor.color = [1, 2, 3];
    for _ in 0..2 {
        editor.refresh("Their name", ColorRgb::new(4, 5, 6));
        assert!(editor.conflicted);
        assert!(!editor.has_valid_changes());
        assert_eq!(editor.name, "My name");
        assert_eq!(editor.color, [1, 2, 3]);
    }
    editor.refresh("Original", ColorRgb::BLACK);
    assert!(
        !editor.conflicted,
        "undoing outside changes removes the conflict"
    );
    assert!(editor.has_valid_changes());
    editor.refresh("Their name", ColorRgb::new(4, 5, 6));
    editor.name = "Their name".into();
    editor.refresh("Their name", ColorRgb::new(4, 5, 6));
    assert!(
        editor.conflicted,
        "the unresolved color conflict still blocks Apply"
    );
    editor.color = [4, 5, 6];
    editor.refresh("Their name", ColorRgb::new(4, 5, 6));
    assert!(!editor.conflicted);
    assert!(
        !editor.has_valid_changes(),
        "identical external and draft values are a no-op"
    );
}

#[test]
fn editor_refresh_adopts_only_untouched_fields() {
    let id = Document::default().current_layer_id();
    let mut editor = LayerEditor::new(id, "Original".into(), ColorRgb::BLACK);
    editor.name = "My name".into();
    editor.refresh("Original", ColorRgb::new(4, 5, 6));
    assert_eq!(editor.name, "My name");
    assert_eq!(editor.color, [4, 5, 6]);
    assert_eq!(editor.original_color, [4, 5, 6]);
    assert!(editor.has_valid_changes());
    assert!(!editor.conflicted);
}

#[test]
fn deleting_an_edited_layer_closes_its_draft_even_if_undo_restores_the_id() {
    let mut document = Document::default();
    let id = document.add_layer("Temporary", ColorRgb::BLACK).unwrap();
    let mut sidebar = DocumentSidebar {
        layer_editor: Some(LayerEditor::new(id, "Temporary".into(), ColorRgb::BLACK)),
        ..Default::default()
    };
    sidebar.layer_editor.as_mut().unwrap().name = "Old draft".into();
    document.delete_layer(id).unwrap();
    for restore in [false, true] {
        if restore {
            document.undo().unwrap();
        }
        egui::Context::default()
            .run_ui(Default::default(), |ui| {
                assert!(sidebar.show(ui, &document).is_empty());
            })
            .drop_without_applying_deltas();
        assert!(sidebar.layer_editor.is_none());
    }
}

#[test]
fn open_editor_merges_external_changes_without_losing_its_draft() {
    let mut document = Document::default();
    let id = document.add_layer("Original", ColorRgb::BLACK).unwrap();
    let mut sidebar = DocumentSidebar::default();
    let mut editor = LayerEditor::new(id, "Original".into(), ColorRgb::BLACK);
    editor.color = [1, 2, 3];
    sidebar.layer_editor = Some(editor);
    document.rename_layer(id, "Renamed externally").unwrap();
    egui::Context::default()
        .run_ui(Default::default(), |ui| {
            assert!(sidebar.show(ui, &document).is_empty());
        })
        .drop_without_applying_deltas();
    let editor = sidebar.layer_editor.as_ref().unwrap();
    assert_eq!(editor.name, "Renamed externally");
    assert_eq!(editor.color, [1, 2, 3]);
    assert!(editor.has_valid_changes());
}

#[test]
fn new_layer_text_focus_survives_a_layer_insert() {
    let mut document = Document::default();
    let context = egui::Context::default();
    let mut sidebar = DocumentSidebar::default();
    let mut frame = |document: &Document, events| {
        context.run_ui(
            egui::RawInput {
                screen_rect: Some(egui::Rect::from_min_size(
                    egui::Pos2::ZERO,
                    egui::vec2(800.0, 600.0),
                )),
                events,
                ..Default::default()
            },
            |ui| {
                assert!(sidebar.show(ui, document).is_empty());
            },
        )
    };
    let output = frame(&document, vec![]);
    let position = output
        .shapes
        .iter()
        .find_map(|clipped| {
            let egui::epaint::Shape::Text(text) = &clipped.shape else {
                return None;
            };
            (text.galley.text() == "Layer name")
                .then_some(text.galley.rect.translate(text.pos.to_vec2()).center())
        })
        .unwrap();
    output.drop_without_applying_deltas();
    for pressed in [true, false] {
        frame(
            &document,
            vec![
                egui::Event::PointerMoved(position),
                egui::Event::PointerButton {
                    pos: position,
                    button: egui::PointerButton::Primary,
                    pressed,
                    modifiers: Default::default(),
                },
            ],
        )
        .drop_without_applying_deltas();
    }
    document.add_layer("Inserted", ColorRgb::BLACK).unwrap();
    frame(&document, vec![egui::Event::Text("Design".into())]).drop_without_applying_deltas();
    assert_eq!(sidebar.new_layer_name, "Design");
}

#[test]
fn deleting_a_pressed_row_does_not_click_its_replacement() {
    for layers in [false, true] {
        let mut document = Document::default();
        let mut layer_ids = Vec::new();
        let mut group_ids = Vec::new();
        for name in ["First", "Second", "Third"] {
            if layers {
                layer_ids.push(document.add_layer(name, ColorRgb::BLACK).unwrap());
            } else {
                group_ids.push(document.add_empty_group(Some(name.into())).unwrap());
            }
        }
        let context = egui::Context::default();
        let mut sidebar = DocumentSidebar::default();
        let mut frame = |document: &Document, events| {
            let mut actions = Vec::new();
            let output = context.run_ui(
                egui::RawInput {
                    screen_rect: Some(egui::Rect::from_min_size(
                        egui::Pos2::ZERO,
                        egui::vec2(800.0, 600.0),
                    )),
                    events,
                    ..Default::default()
                },
                |ui| {
                    actions = sidebar.show(ui, document);
                },
            );
            (output, actions)
        };
        let (output, _) = frame(&document, vec![]);
        let texts = output
            .shapes
            .iter()
            .filter_map(|clipped| {
                let egui::epaint::Shape::Text(text) = &clipped.shape else {
                    return None;
                };
                Some((
                    text.galley.text(),
                    text.galley.rect.translate(text.pos.to_vec2()),
                ))
            })
            .collect::<Vec<_>>();
        let row = texts
            .iter()
            .find(|(text, _)| *text == if layers { "Second" } else { "Second · 0" })
            .unwrap()
            .1;
        let position = texts
            .iter()
            .find(|(text, rect)| *text == "×" && (rect.center().y - row.center().y).abs() < 3.0)
            .unwrap()
            .1
            .center();
        output.drop_without_applying_deltas();
        let click = |pressed| {
            vec![
                egui::Event::PointerMoved(position),
                egui::Event::PointerButton {
                    pos: position,
                    button: egui::PointerButton::Primary,
                    pressed,
                    modifiers: Default::default(),
                },
            ]
        };
        let (output, actions) = frame(&document, click(true));
        output.drop_without_applying_deltas();
        assert!(actions.is_empty());
        if layers {
            document.delete_layer(layer_ids[1]).unwrap();
        } else {
            document.remove_group(group_ids[1]).unwrap();
        }
        let (output, actions) = frame(&document, click(false));
        output.drop_without_applying_deltas();
        assert!(
            actions.is_empty(),
            "release must not delete the shifted replacement row (layers={layers})"
        );
    }
}

#[test]
fn long_names_leave_layer_and_group_actions_visible() {
    let mut document = Document::default();
    let layer = document
        .add_layer("LongLayerName".repeat(40), ColorRgb::BLACK)
        .unwrap();
    document
        .add_empty_group(Some("LongGroupName".repeat(40)))
        .unwrap();
    let context = egui::Context::default();
    let mut sidebar = DocumentSidebar::default();
    let mut edit_position = None;
    for _ in 0..2 {
        let output = context.run_ui(
            egui::RawInput {
                screen_rect: Some(egui::Rect::from_min_size(
                    egui::Pos2::ZERO,
                    egui::vec2(600.0, 400.0),
                )),
                ..Default::default()
            },
            |ui| {
                assert!(sidebar.show(ui, &document).is_empty());
            },
        );
        let count = |needle| {
            output
                .shapes
                .iter()
                .filter(|clipped| {
                    let egui::epaint::Shape::Text(text) = &clipped.shape else {
                        return false;
                    };
                    text.galley.text() == needle
                        && clipped
                            .clip_rect
                            .contains_rect(text.galley.rect.translate(text.pos.to_vec2()))
                })
                .count()
        };
        let edits = count("Edit");
        let removals = count("×");
        edit_position = output
            .shapes
            .iter()
            .filter_map(|clipped| {
                let egui::epaint::Shape::Text(text) = &clipped.shape else {
                    return None;
                };
                (text.galley.text() == "Edit")
                    .then_some(text.galley.rect.translate(text.pos.to_vec2()).center())
            })
            .nth(1);
        output.drop_without_applying_deltas();
        assert_eq!(edits, 2, "both layer editors must remain reachable");
        assert_eq!(
            removals, 3,
            "layer and group controls must remain reachable"
        );
    }
    let position = edit_position.unwrap();
    for pressed in [true, false] {
        context
            .run_ui(
                egui::RawInput {
                    screen_rect: Some(egui::Rect::from_min_size(
                        egui::Pos2::ZERO,
                        egui::vec2(600.0, 400.0),
                    )),
                    events: vec![
                        egui::Event::PointerMoved(position),
                        egui::Event::PointerButton {
                            pos: position,
                            button: egui::PointerButton::Primary,
                            pressed,
                            modifiers: Default::default(),
                        },
                    ],
                    ..Default::default()
                },
                |ui| {
                    assert!(sidebar.show(ui, &document).is_empty());
                },
            )
            .drop_without_applying_deltas();
    }
    let editor = sidebar
        .layer_editor
        .as_ref()
        .expect("long-name layer editor opens");
    assert_eq!(editor.id, layer);
    assert_eq!(editor.name, "LongLayerName".repeat(40));
}

fn visible_text(output: &egui::FullOutput, needle: &str) -> bool {
    output.shapes.iter().any(|clipped| {
        if let egui::epaint::Shape::Text(text) = &clipped.shape {
            text.galley.text() == needle
                && clipped
                    .clip_rect
                    .intersects(text.galley.rect.translate(text.pos.to_vec2()))
        } else {
            false
        }
    })
}

#[test]
fn long_document_lists_scroll_to_the_group_controls() {
    let mut document = Document::default();
    for i in 0..80 {
        document
            .add_layer(format!("Layer {i:02}"), ColorRgb::BLACK)
            .unwrap();
        document
            .add_empty_group(Some(format!("Group {i:02}")))
            .unwrap();
    }
    let context = egui::Context::default();
    let mut sidebar = DocumentSidebar::default();
    let mut frame = |events| {
        let mut actions = Vec::new();
        let output = context.run_ui(
            egui::RawInput {
                screen_rect: Some(egui::Rect::from_min_size(
                    egui::Pos2::ZERO,
                    egui::vec2(800.0, 600.0),
                )),
                events,
                ..Default::default()
            },
            |ui| {
                actions = sidebar.show(ui, &document);
            },
        );
        (output, actions)
    };
    let (first, actions) = frame(vec![]);
    assert!(actions.is_empty());
    assert!(!visible_text(&first, "Create a group with: Group [name]"));
    first.drop_without_applying_deltas();
    let mut reached = false;
    let mut last_button = None;
    for _ in 0..20 {
        let (output, actions) = frame(vec![
            egui::Event::PointerMoved(egui::pos2(700.0, 300.0)),
            egui::Event::MouseWheel {
                unit: egui::MouseWheelUnit::Point,
                delta: egui::vec2(0.0, -1000.0),
                phase: egui::TouchPhase::Move,
                modifiers: Default::default(),
            },
        ]);
        assert!(actions.is_empty());
        reached |= visible_text(&output, "Create a group with: Group [name]");
        let texts = output
            .shapes
            .iter()
            .filter_map(|clipped| {
                let egui::epaint::Shape::Text(text) = &clipped.shape else {
                    return None;
                };
                let rect = text.galley.rect.translate(text.pos.to_vec2());
                clipped
                    .clip_rect
                    .contains_rect(rect)
                    .then_some((text.galley.text(), rect))
            })
            .collect::<Vec<_>>();
        if let Some((_, group)) = texts.iter().find(|(text, _)| *text == "Group 79 · 0") {
            last_button = texts
                .iter()
                .find(|(text, rect)| {
                    *text == "×" && (rect.center().y - group.center().y).abs() < 3.0
                })
                .map(|(_, rect)| rect.center());
        }
        output.drop_without_applying_deltas();
    }
    assert!(
        reached,
        "the group controls must be reachable by scrolling inside the sidebar"
    );
    let position = last_button.expect("last group's button must be fully visible");
    let click = |pressed| {
        vec![
            egui::Event::PointerMoved(position),
            egui::Event::PointerButton {
                pos: position,
                button: egui::PointerButton::Primary,
                pressed,
                modifiers: Default::default(),
            },
        ]
    };
    let (output, actions) = frame(click(true));
    assert!(actions.is_empty());
    output.drop_without_applying_deltas();
    let (output, actions) = frame(click(false));
    assert_eq!(actions.len(), 1);
    assert!(
        matches!(&actions[0], SidebarAction::RemoveGroup { id, name }
        if *id == document.groups().last().unwrap().id() && name == "Group 79")
    );
    output.drop_without_applying_deltas();
}
