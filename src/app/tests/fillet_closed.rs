use super::curve_fillet::{enter, inputs, mouse_picks, replay};
use super::plane_surface_native_support as native;
use super::*;

#[test]
fn closed_sources_keep_parent_curves_and_match_native_dynamic_admission() {
    assert_eq!(
        replay(
            &["fillet_closed_pilot", "fillet_closed_final"],
            "FILLET_CLOSED_UI"
        ),
        16
    );
}

#[test]
fn closed_mouse_picks_preview_and_accept_the_same_geometry_once() {
    let data = native::read("fillet_closed_pilot");
    let value = &data["results"]
        .as_array()
        .unwrap()
        .iter()
        .find(|r| r["id"] == "circle_line_joined")
        .unwrap()["value"];
    let mut app = test_app();
    app.document = inputs::setup(value);
    let state = inputs::State::capture(&app.document);
    let picks = inputs::picks(&app.document, value);
    let circle = app
        .document
        .object(picks[0].0)
        .unwrap()
        .geometry_snapshot()
        .clone();
    app.viewports[0] = Viewport::new(ViewKind::Top);
    enter(&mut app, "Fillet Radius=0.5 Trim=Yes Join=Yes Dynamic=Yes");
    mouse_picks(
        &mut app,
        picks.map(|(id, _)| id),
        [
            native::point(&value["prescribed_case"]["actions"][6]["pick"]),
            native::point(&value["prescribed_case"]["actions"][7]["pick"]),
        ],
    );
    let scene = app.curve_fillet_session.as_ref().unwrap().scene().unwrap();
    let joined = scene.objects().last().unwrap().geometry().clone();
    assert!(!app.document.can_undo());
    enter(&mut app, "");
    assert!(
        circle.shares_storage_with(app.document.object(picks[0].0).unwrap().geometry_snapshot())
    );
    assert!(app.document.objects().any(|o| o.geometry() == &joined));
    state.verify(&mut app.document, value);
    println!("FILLET_CLOSED_MOUSE_SOURCE_PICKS 2");
}
