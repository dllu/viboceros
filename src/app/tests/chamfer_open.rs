use super::curve_chamfer::replay;
use super::curve_fillet::{enter, inputs, mouse_source_picks};
use super::plane_surface_native_support as native;
use super::*;

#[test]
fn open_contacts_match_native_geometry_metadata_and_actual_history() {
    assert_eq!(
        replay(
            &[
                "chamfer_open_pilot",
                "chamfer_open_holdouts",
                "chamfer_open_curves",
                "chamfer_open_choices",
                "chamfer_open_overlaps"
            ],
            "CHAMFER_OPEN"
        ),
        65
    );
}

#[test]
fn equal_replacements_from_mouse_picks_record_one_step_and_clear_old_redo() {
    let data = native::read("chamfer_open_pilot");
    let value = &data["results"]
        .as_array()
        .unwrap()
        .iter()
        .find(|r| r["id"] == "open_chamfer_zero_parts")
        .unwrap()["value"];
    let mut app = test_app();
    app.document = inputs::setup(value);
    let state = inputs::State::capture(&app.document);
    let picks = inputs::picks(&app.document, value);
    let original =
        picks.map(|(id, _)| app.document.object(id).unwrap().geometry_snapshot().clone());
    app.viewports[0] = Viewport::new(ViewKind::Top);
    enter(&mut app, "Chamfer Distances=0,0 Join=No Trim=Yes");
    assert!(!app.document.can_undo());
    assert!(app.document.can_redo());
    mouse_source_picks(
        &mut app,
        picks.map(|(id, _)| id),
        [
            Point3::try_new(2.7, 0., 0.).unwrap(),
            Point3::try_new(4., 1.7, 0.).unwrap(),
        ],
    );
    assert!(app.curve_chamfer_session.is_none());
    for (i, (id, _)) in picks.iter().enumerate() {
        let object = app.document.object(*id).unwrap();
        assert_eq!(object.geometry(), &*original[i]);
        assert!(!original[i].shares_storage_with(object.geometry_snapshot()));
    }
    assert_eq!(app.document.undo_label(), Some("Chamfer"));
    assert!(!app.document.can_redo());
    state.verify(&mut app.document, value);
    println!("CHAMFER_OPEN_MOUSE_SOURCE_PICKS 2");
}
