//! Replay the native corpus through the application's ordinary command dispatch.
use super::curve_path_support as support;
use super::*;
use serde_json::json;
use viboceros_oracle::curve_path_extrusion::{Fixture, snapshot};

fn enter(app: &mut VibocerosApp, input: &str) {
    app.command_input = input.into();
    app.run_command();
}

#[test]
fn curve_path_gui_matches_native_geometry_and_history() {
    let tolerance = viboceros_geometry::Tolerance::try_new(1e-6, 1e-12, 1e-8).unwrap();
    let mut count = 0;
    for (operation, native) in support::cases() {
        let fixture: Fixture = serde_json::from_value(operation.clone()).unwrap();
        let original = fixture.sources().unwrap();
        let mut app = test_app();
        app.document = viboceros_document::Document::new(tolerance);
        let profile = app.document.add_geometry(original[0].clone()).unwrap();
        let path = app
            .document
            .add_geometry_with_attributes(
                original[1].clone(),
                viboceros_document::ObjectAttributes::on_layer(app.document.current_layer_id())
                    .with_name("OraclePath"),
            )
            .unwrap();
        let ids = [profile, path];
        app.document
            .select_objects_direct([profile], SelectionMode::Replace)
            .unwrap();
        app.document.clear_history().unwrap();
        enter(&mut app, fixture.crease_command());
        assert_eq!(app.commands.crease_splitting(), fixture.crease_splitting);
        enter(&mut app, &fixture.command());
        let after = snapshot(&app.document, ids, &original).unwrap();
        if native["succeeded"] == false {
            assert_eq!(
                after,
                json!([{"source":0,"unchanged":true},{"source":1,"unchanged":true}])
            );
            assert_eq!(app.document.undo_label(), None);
            println!(
                "CURVE_PATH_APP_FAILURE {}",
                json!({"id":operation["id"],"actual":{"succeeded":false,"after":after,"undo":null,"redo":null},"native":native["after"]})
            );
            continue;
        }
        count += 1;
        assert_eq!(
            app.document.undo_label(),
            Some("ExtrudeCrvAlongCrv"),
            "{}: {:?}",
            operation["id"],
            app.command_log
        );
        enter(&mut app, "Undo");
        let undo = snapshot(&app.document, ids, &original).unwrap();
        enter(&mut app, "Redo");
        let redo = snapshot(&app.document, ids, &original).unwrap();
        let actual = json!({"succeeded":true,"after":after,"undo":undo,"redo":redo});
        let maximum_error = support::compare(&actual, &native);
        println!(
            "CURVE_PATH_APP {}",
            json!({"id":operation["id"],"value":actual,"maximum_error":maximum_error})
        );
    }
    assert_eq!(count, 264);
}
