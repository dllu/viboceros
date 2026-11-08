use super::curve_fillet::{enter, inputs, mouse_picks, replay};
use super::plane_surface_native_support as native;
use super::*;

#[test]
fn open_zero_radius_getters_match_native_geometry_metadata_and_actual_history() {
    assert_eq!(
        replay(
            &[
                "fillet_open_zero_pilot",
                "fillet_open_zero_holdouts",
                "fillet_open_zero_overlap_final",
                "fillet_open_zero_bounds",
                "fillet_open_zero_orientations",
            ],
            "FILLET_OPEN_ZERO_UI"
        ),
        49
    );
}

#[test]
fn equal_geometry_mouse_preview_reuses_candidates_and_records_one_replacement_step() {
    let data = native::read("fillet_open_zero_pilot");
    let value = &data["results"]
        .as_array()
        .unwrap()
        .iter()
        .find(|r| r["id"] == "open_zero_dynamic_accept")
        .unwrap()["value"];
    let mut app = test_app();
    app.document = inputs::setup(value);
    let state = inputs::State::capture(&app.document);
    let picks = inputs::picks(&app.document, value);
    let original =
        picks.map(|(id, _)| app.document.object(id).unwrap().geometry_snapshot().clone());
    app.viewports[0] = Viewport::new(ViewKind::Top);
    enter(&mut app, "Fillet Radius=0 Trim=Yes Join=No Dynamic=Yes");
    mouse_picks(
        &mut app,
        picks.map(|(id, _)| id),
        [
            native::point(&value["prescribed_case"]["actions"][6]["pick"]),
            native::point(&value["prescribed_case"]["actions"][7]["pick"]),
        ],
    );
    let candidates = |app: &VibocerosApp| {
        app.curve_fillet_session
            .as_ref()
            .unwrap()
            .scene()
            .unwrap()
            .objects()
            .skip(3)
            .map(|o| o.geometry_snapshot().clone())
            .collect::<Vec<_>>()
    };
    let cached = candidates(&app);
    assert_eq!(cached.len(), 2);
    enter(&mut app, "0");
    assert!(
        cached
            .iter()
            .zip(candidates(&app))
            .all(|(a, b)| a.shares_storage_with(&b))
    );
    assert!(!app.document.can_undo());
    assert!(app.document.can_redo());
    for (i, (id, _)) in picks.iter().enumerate() {
        assert!(
            original[i].shares_storage_with(app.document.object(*id).unwrap().geometry_snapshot())
        );
    }
    enter(&mut app, "");
    for (i, (id, _)) in picks.iter().enumerate() {
        let object = app.document.object(*id).unwrap();
        assert_eq!(object.geometry(), &*original[i]);
        assert!(!original[i].shares_storage_with(object.geometry_snapshot()));
        assert_eq!(object.geometry(), &*cached[i]);
    }
    assert_eq!(app.document.undo_label(), Some("Fillet"));
    assert!(!app.document.can_redo());
    state.verify(&mut app.document, value);
    println!("FILLET_OPEN_ZERO_MOUSE_SOURCE_PICKS 2");
}
