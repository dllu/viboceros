use super::*;

#[test]
fn inset_loose_native_completed_and_canceled_preferences_replay() {
    let native = common::cases_for("inset_loose_preferences").remove(0).1;
    let mut app = test_app();
    for (i, step) in native["steps"].as_array().unwrap().iter().enumerate() {
        let input = &step["input"];
        let expected = &step["value"];
        let (doc, id) = native_surface::fixture(expected);
        app.document = doc;
        app.document
            .select_objects_direct([id], SelectionMode::Replace)
            .unwrap();
        let defaults = app.commands.inset_surface_options();
        assert!(!defaults.loose);
        assert!(
            expected["history"]
                .as_str()
                .unwrap()
                .lines()
                .nth(1)
                .unwrap()
                .contains("Loose=Off")
        );
        let before = app.document.object(id).unwrap().geometry_snapshot().clone();
        enter(&mut app, "Inset");
        assert!(app.command_log.back().unwrap().contains("Loose=Off"));
        if let Some(loose) = input["loose"].as_bool() {
            enter(&mut app, if loose { "Loose=On" } else { "Loose=Off" });
        }
        if let Some(crease) = input["crease_corners"].as_bool() {
            enter(
                &mut app,
                if crease {
                    "CreaseCorners=Yes"
                } else {
                    "CreaseCorners=No"
                },
            );
        }
        let canceled = input["cancel"] == true;
        if canceled {
            enter(&mut app, "Cancel");
        } else {
            enter(&mut app, &input["distance"].as_f64().unwrap().to_string());
            let session = app.inset_session.as_ref().unwrap();
            assert!(!session.edges().is_empty());
            assert_eq!(
                app.document.object(id).unwrap().geometry_snapshot(),
                &before
            );
            enter(&mut app, "");
        }
        assert!(app.inset_session.is_none());
        assert!(!app.commands.inset_surface_options().loose);
        if canceled {
            assert_eq!(app.commands.inset_surface_options(), defaults);
            assert_eq!(
                app.document.object(id).unwrap().geometry_snapshot(),
                &before
            );
            assert!(!app.document.can_undo());
        } else {
            assert_eq!(
                app.commands.inset_surface_options().distance,
                input["distance"].as_f64().unwrap()
            );
        }
        let Geometry::Brep(result) = app.document.object(id).unwrap().geometry() else {
            panic!()
        };
        native_surface::assert_geometry_with_epsilon(
            result,
            &expected["after"][0]["brep"],
            app.document.tolerance(),
            if input["loose"] == true { 1e-10 } else { 2e-6 },
            &format!("preference step {i}"),
        );
        if !canceled {
            let after = app.document.object(id).unwrap().geometry_snapshot().clone();
            enter(&mut app, "SelNone");
            enter(&mut app, "Undo");
            assert_eq!(
                app.document.object(id).unwrap().geometry_snapshot(),
                &before
            );
            enter(&mut app, "Redo");
            assert_eq!(app.document.object(id).unwrap().geometry_snapshot(), &after);
        }
    }
}
