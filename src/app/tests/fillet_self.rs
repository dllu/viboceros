use super::curve_fillet::{enter, inputs, mouse_picks, replay};
use super::plane_surface_native_support as native;
use super::*;

#[test]
fn same_parent_edits_noops_failure_and_cancellation_match_native() {
    assert_eq!(
        replay(
            &[
                "fillet_self_pilot",
                "fillet_self_discovery",
                "fillet_self_holdouts",
                "fillet_self_branches",
                "fillet_self_limits"
            ],
            "FILLET_SELF_UI"
        ),
        38
    );
}

#[test]
fn same_parent_mouse_picks_keep_one_selected_object_and_commit_one_edit() {
    let data = native::read("fillet_self_discovery");
    let value = &data["results"]
        .as_array()
        .unwrap()
        .iter()
        .find(|r| r["id"] == "self_open_joined")
        .unwrap()["value"];
    let mut app = test_app();
    app.document = inputs::setup(value);
    let state = inputs::State::capture(&app.document);
    let picks = inputs::picks(&app.document, value);
    assert_eq!(picks[0].0, picks[1].0);
    app.viewports[0] = Viewport::new(ViewKind::Top);
    enter(&mut app, "Fillet Radius=0.5 Trim=Yes Join=Yes Dynamic=Yes");
    mouse_picks(&mut app, picks.map(|(id, _)| id), picks.map(|(_, p)| p));
    assert_eq!(app.document.selected_object_count(), 1);
    let scene = app.curve_fillet_session.as_ref().unwrap().scene().unwrap();
    let candidate = scene.objects().last().unwrap().geometry().clone();
    assert!(!app.document.can_undo());
    enter(&mut app, "");
    assert_eq!(
        app.document.object(picks[0].0).unwrap().geometry(),
        &candidate
    );
    state.verify(&mut app.document, value);
    println!("FILLET_SELF_MOUSE_SOURCE_PICKS 2");
}
