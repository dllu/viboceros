//! Command-first selection and ordinary command entry with shared native operands.
use super::direction_support as support;
use super::*;
use serde_json::json;
use viboceros_oracle::brep_direction::{Fixture, snapshot};
fn enter(app: &mut VibocerosApp, input: &str) {
    app.command_input = input.into();
    app.run_command();
}

#[test]
fn direction_input_matches_shared_native_geometry_and_history() {
    for (operation, native) in support::cases(false) {
        let f: Fixture = serde_json::from_value(operation.clone()).unwrap();
        let mut app = test_app();
        let tolerance = viboceros_geometry::Tolerance::try_new(1e-8, 1e-12, 1e-8).unwrap();
        app.document = viboceros_document::Document::new(tolerance);
        let source = f.source(tolerance).unwrap();
        let id = app
            .document
            .add_geometry_with_attributes(
                Geometry::Brep(source),
                viboceros_document::ObjectAttributes::on_layer(app.document.current_layer_id())
                    .with_name("Direction source"),
            )
            .unwrap();
        app.document
            .select_objects_direct([id], SelectionMode::Replace)
            .unwrap();
        app.commands
            .execute(&mut app.document, "Group DirectionSource")
            .unwrap();
        if !f.preselect {
            app.document.clear_selection();
        }
        app.document.clear_history().unwrap();
        let before = snapshot(&app.document, id).unwrap();
        enter(&mut app, &f.command().unwrap());
        if !f.preselect {
            assert!(
                app.object_prompt.is_some(),
                "{}: {:?}",
                operation["id"],
                app.command_log
            );
            app.apply_selection_click(SelectionClick {
                object_id: Some(id),
                mode: SelectionMode::Replace,
            });
            enter(&mut app, "");
        }
        assert_eq!(
            app.document.undo_label(),
            Some("Dir"),
            "{}: {:?}",
            operation["id"],
            app.command_log
        );
        let after = snapshot(&app.document, id).unwrap();
        enter(&mut app, "Undo");
        let undo = snapshot(&app.document, id).unwrap();
        enter(&mut app, "Redo");
        let redo = snapshot(&app.document, id).unwrap();
        support::compare(
            &json!({"before":before,"after":after,"undo":undo,"redo":redo}),
            &native,
        );
    }
}

#[test]
fn direction_face_batch_uses_its_typed_scope_and_cancel_keeps_redo() {
    let (operation, native) = support::cases(true)
        .into_iter()
        .find(|(op, _)| op["id"] == "dir-sdk-box-u_reverse-135")
        .unwrap();
    let mut f: Fixture = serde_json::from_value(operation).unwrap();
    f.preselect = false;
    f.sdk = false;
    let mut app = test_app();
    let id = app
        .document
        .add_geometry(Geometry::Brep(f.source(app.document.tolerance()).unwrap()))
        .unwrap();
    app.document.clear_history().unwrap();
    app.commands
        .execute(&mut app.document, "Point 99,99,99")
        .unwrap();
    app.document.undo().unwrap();
    let before = format!("{:?}", app.document);
    let input = f.command().unwrap();
    enter(&mut app, &input);
    app.apply_selection_click(SelectionClick {
        object_id: Some(id),
        mode: SelectionMode::Replace,
    });
    app.cancel_current_prompt_or_selection();
    assert_eq!(format!("{:?}", app.document), before);
    enter(&mut app, &input);
    app.apply_selection_click(SelectionClick {
        object_id: Some(id),
        mode: SelectionMode::Replace,
    });
    enter(&mut app, "");
    assert_eq!(
        app.document.undo_label(),
        Some("Dir"),
        "{:?}",
        app.command_log
    );
    let actual = snapshot(&app.document, id).unwrap();
    // Public face replacement qualifies this explicit batching extension's
    // geometry; its selection history is intentionally a different route.
    support::compare(&actual["geometry"], &native["after"]["geometry"]);
    enter(&mut app, "Undo");
    assert!(!app.document.is_selected(id));
}
