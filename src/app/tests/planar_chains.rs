use super::*;
use viboceros_oracle::planar_regions::{Fixture, snapshot_chains};
#[path = "../../../crates/viboceros-oracle/tests/support/planar_chains.rs"]
mod boundaries;
#[test]
fn planar_chains_application_preserves_native_regions_and_history() {
    let base = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tools/rhino_oracle");
    let tolerance = Tolerance::try_new(1e-6, 1e-12, 1e-6).unwrap();
    let mut gaps = Vec::new();
    for (op, native) in boundaries::rows(&base, super::oracle_observation::read) {
        let f: Fixture = serde_json::from_value(op.clone()).unwrap();
        let (doc, ids) = f.setup(tolerance).unwrap();
        let sources = doc
            .objects()
            .map(|o| o.geometry().nurbs_curve_representation().unwrap().unwrap())
            .collect::<Vec<_>>();
        let mut app = test_app();
        app.document = doc;
        let before = snapshot_chains(&app.document, &ids, &sources).unwrap();
        app.command_input = "PlanarSrf".into();
        app.run_command();
        assert!(
            app.object_prompt.is_none(),
            "{}: {:?}",
            op["id"],
            app.command_log
        );
        let success = app.document.undo_label() == Some("PlanarSrf");
        let after = snapshot_chains(&app.document, &ids, &sources).unwrap();
        if op["id"] == "chains_filled_outside_tangent" {
            assert!(!success);
            assert_eq!(before, after);
            gaps.push(op["id"].clone());
            continue;
        }
        let outputs = app
            .document
            .objects()
            .filter_map(|o| match o.geometry() {
                Geometry::Brep(b) => Some(b),
                _ => None,
            })
            .collect::<Vec<_>>();
        boundaries::assert_geometry(
            &outputs,
            &native["after"],
            tolerance,
            op["id"].as_str().unwrap(),
        );
        if success {
            app.command_input = "Undo".into();
            app.run_command();
        }
        let undo = snapshot_chains(&app.document, &ids, &sources).unwrap();
        if success {
            app.command_input = "Redo".into();
            app.run_command();
        }
        let redo = snapshot_chains(&app.document, &ids, &sources).unwrap();
        let actual = serde_json::json!({"success":success,"before":before,"after":after,"undo":undo,"redo":redo});
        assert_eq!(
            super::planar_regions_comparison::compare(
                &actual,
                &native,
                1e-9,
                op["id"].as_str().unwrap()
            )
            .unwrap_or_else(|e| panic!("{}: {e}", op["id"])),
            0
        );
    }
    assert_eq!(gaps, [serde_json::json!("chains_filled_outside_tangent")]);
}
#[test]
fn planar_chains_postselection_delete_option_and_escape_use_the_command_prompt() {
    for cancel in [false, true] {
        let mut app = test_app();
        for text in [
            "Line 0,0,0 10,0,0",
            "Line 10,0,0 10,10,0",
            "Line 10,10,0 0,10,0",
            "Line 0,10,0 0,0,0",
        ] {
            app.command_input = text.into();
            app.run_command();
        }
        let ids = app.document.objects().map(|o| o.id()).collect::<Vec<_>>();
        app.document.clear_selection();
        app.document.clear_history().unwrap();
        let before = app.document.objects().cloned().collect::<Vec<_>>();
        app.command_input = "PlanarSrf".into();
        app.run_command();
        assert!(app.object_prompt.is_some());
        app.command_input = "DeleteInput=Yes".into();
        app.run_command();
        app.select_prompt_objects(ids.clone(), SelectionMode::Replace);
        if cancel {
            app.cancel_current_prompt_or_selection();
            assert_eq!(app.document.objects().cloned().collect::<Vec<_>>(), before);
            assert!(app.document.undo_label().is_none());
        } else {
            app.command_input.clear();
            app.run_command();
            assert_eq!(app.document.objects().count(), 1);
            assert_eq!(app.document.undo_label(), Some("PlanarSrf"));
            app.document.undo().unwrap();
            assert_eq!(app.document.objects().cloned().collect::<Vec<_>>(), before);
        }
        assert!(app.object_prompt.is_none());
    }
}
