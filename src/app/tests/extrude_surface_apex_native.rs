//! Native command corpus replayed through source selection and the point getter.
use super::*;
#[path = "../../../crates/viboceros-oracle/tests/support/surface_apex.rs"]
mod support;

fn enter(app: &mut VibocerosApp, input: &str) {
    app.command_input = input.into();
    app.run_command();
}

#[test]
fn surface_apex_gui_matches_native_geometry_and_history() {
    let tolerance = viboceros_geometry::Tolerance::try_new(1e-6, 1e-12, 1e-8).unwrap();
    for (op, native) in support::cases() {
        let fixture: viboceros_oracle::SurfaceApexExtrusionFixture =
            serde_json::from_value(op.clone()).unwrap();
        let original = fixture.source_face(tolerance).unwrap();
        let mut app = test_app();
        app.document = viboceros_document::Document::new(tolerance);
        let source = app.document.add_geometry(original.clone()).unwrap();
        if op["preselect"] == true {
            app.document
                .select_objects_direct([source], SelectionMode::Replace)
                .unwrap();
        }
        app.document.clear_history().unwrap();
        enter(&mut app, "ExtrudeSrfToPoint");
        if op["preselect"] == false {
            app.document
                .select_objects_direct([source], SelectionMode::Replace)
                .unwrap();
            enter(&mut app, "");
        }
        enter(
            &mut app,
            &format!(
                "Solid={} DeleteInput={}",
                if fixture.solid { "Yes" } else { "No" },
                if op["delete_input"] == true {
                    "Yes"
                } else {
                    "No"
                }
            ),
        );
        let [x, y, z] = fixture.apex;
        enter(&mut app, &format!("{x},{y},{z}"));
        assert!(
            app.active_command.is_none(),
            "{}: {:?}",
            op["id"],
            app.active_command
        );
        let after = support::measure(&app.document, source, &original, &native["after"]);
        enter(&mut app, "Undo");
        let undo = support::measure(&app.document, source, &original, &native["undo"]);
        enter(&mut app, "Redo");
        let redo = support::measure(&app.document, source, &original, &native["redo"]);
        println!(
            "SURFACE_APEX_APP {}",
            serde_json::json!({"id":op["id"],"after":after,"undo":undo,"redo":redo})
        );
    }
}
