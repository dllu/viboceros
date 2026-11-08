use super::curve_fillet::{enter, inputs, mouse_picks, replay};
use super::plane_surface_native_support as native;
use super::*;

#[test]
fn continuation_getters_match_every_native_result_and_history() {
    assert_eq!(
        replay(
            &[
                "fillet_continuation_pilot",
                "fillet_continuation_holdouts",
                "fillet_continuation_verification",
                "fillet_continuation_branches",
            ],
            "FILLET_CONTINUATION_UI"
        ),
        44
    );
}

#[test]
fn mouse_extension_preview_reuses_and_commits_the_cached_geometry_once() {
    let data = native::read("fillet_continuation_pilot");
    let value = &data["results"]
        .as_array()
        .unwrap()
        .iter()
        .find(|r| r["id"] == "continuation_tangent_arc_joined")
        .unwrap()["value"];
    let mut app = test_app();
    app.document = inputs::setup(value);
    let state = inputs::State::capture(&app.document);
    let picks = inputs::picks(&app.document, value);
    let parent = app
        .document
        .object(picks[0].0)
        .unwrap()
        .geometry_snapshot()
        .clone();
    app.viewports[0] = Viewport::new(ViewKind::Top);
    enter(
        &mut app,
        "Fillet Radius=0.1 Trim=Yes Join=Yes Dynamic=Yes ExtendArcsBy=Line ExtendOtherCurvesBy=Line",
    );
    mouse_picks(
        &mut app,
        picks.map(|(id, _)| id),
        [
            native::point(&value["prescribed_case"]["actions"][8]["pick"]),
            native::point(&value["prescribed_case"]["actions"][9]["pick"]),
        ],
    );
    let snapshot = |app: &VibocerosApp| {
        app.curve_fillet_session
            .as_ref()
            .unwrap()
            .scene()
            .unwrap()
            .objects()
            .last()
            .unwrap()
            .geometry_snapshot()
            .clone()
    };
    let cached = snapshot(&app);
    enter(&mut app, "w0,0,0");
    let point = Point3::try_new(0., 0.1, 0.).unwrap();
    app.hover_curve_fillet_radius(point);
    assert!(cached.shares_storage_with(&snapshot(&app)));
    app.handle_viewport_action(ViewportOutput {
        picked_point: Some(point),
        ..Default::default()
    });
    assert!(cached.shares_storage_with(&snapshot(&app)));
    assert!(!app.document.can_undo());
    enter(&mut app, "");
    assert!(app.curve_fillet_session.is_none());
    assert!(
        parent.shares_storage_with(app.document.object(picks[0].0).unwrap().geometry_snapshot())
    );
    assert!(app.document.objects().any(|o| o.geometry() == &*cached));
    state.verify(&mut app.document, value);
    println!("FILLET_CONTINUATION_MOUSE_SOURCE_PICKS 2");
}
