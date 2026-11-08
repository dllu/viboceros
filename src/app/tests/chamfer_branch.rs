use super::curve_fillet::{enter, inputs};
use super::plane_surface_native_support as native;
use super::*;
const FAMILIES: [&str; 4] = [
    "chamfer_branch_pilot",
    "chamfer_branch_holdouts",
    "chamfer_branch_seams",
    "chamfer_branch_verification",
];
#[test]
fn branch_getters_and_results_match_native() {
    assert_eq!(
        super::curve_chamfer::replay(&FAMILIES, "CHAMFER_BRANCH"),
        46
    );
}
#[test]
fn branch_mouse_picks_preserve_the_closed_parent_and_commit_once() {
    let data = native::read("chamfer_branch_pilot");
    let v = &data["results"][2]["value"];
    let mut app = test_app();
    app.document = inputs::setup(v);
    let state = inputs::State::capture(&app.document);
    let picks = inputs::picks(&app.document, v);
    assert_ne!(picks[0].0, picks[1].0);
    app.viewports[0] = Viewport::new(ViewKind::Top);
    enter(
        &mut app,
        "Chamfer Distances=0.5,0.2 Trim=Yes Join=Yes ExtendArcsBy=Line ExtendOtherCurvesBy=Line",
    );
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
    assert!(app.document.object(picks[0].0).is_some());
    assert!(app.document.object(picks[1].0).is_none());
    state.verify(&mut app.document, v);
    println!("CHAMFER_BRANCH_MOUSE_SOURCE_PICKS 2");
}
