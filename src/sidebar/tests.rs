use super::*;

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
