use super::curve_fillet::{enter, inputs};
use super::plane_surface_native_support as native;
use super::*;
const FAMILIES: [&str; 4] = [
    "chamfer_self_pilot",
    "chamfer_self_discovery",
    "chamfer_self_holdouts",
    "chamfer_self_spans",
];
#[test]
fn same_parent_getters_and_results_match_native() {
    assert_eq!(super::curve_chamfer::replay(&FAMILIES, "CHAMFER_SELF"), 46);
}
#[test]
fn repeated_mouse_picks_keep_one_parent_and_commit_one_transaction() {
    let data = native::read("chamfer_self_pilot");
    let v = &data["results"][1]["value"];
    let mut app = test_app();
    app.document = inputs::setup(v);
    let state = inputs::State::capture(&app.document);
    let picks = inputs::picks(&app.document, v);
    assert_eq!(picks[0].0, picks[1].0);
    app.viewports[0] = Viewport::new(ViewKind::Top);
    enter(&mut app, "Chamfer Distances=0.5,1 Trim=Yes Join=Yes");
    let context = egui::Context::default();
    let (_, rect) = super::curve_fillet::mouse_frame(&mut app, &context, vec![]);
    for (index, (id, p)) in picks.into_iter().enumerate() {
        let pos = app.viewports[0].project(p, rect).unwrap();
        super::curve_fillet::mouse_frame(&mut app, &context, vec![egui::Event::PointerMoved(pos)]);
        for pressed in [true, false] {
            let (output, _) = super::curve_fillet::mouse_frame(
                &mut app,
                &context,
                vec![egui::Event::PointerButton {
                    pos,
                    button: egui::PointerButton::Primary,
                    pressed,
                    modifiers: egui::Modifiers::NONE,
                }],
            );
            if !pressed {
                assert_eq!(output.selection_click.as_ref().unwrap().object_id, Some(id));
            }
            app.handle_viewport_action(output);
        }
        if index == 0 {
            assert_eq!(app.document.selected_object_count(), 1);
            assert!(!app.document.can_undo());
        }
    }
    assert!(app.curve_chamfer_session.is_none());
    assert_eq!(app.document.undo_label(), Some("Chamfer"));
    state.verify(&mut app.document, v);
    println!("CHAMFER_SELF_MOUSE_SOURCE_PICKS 2");
}
