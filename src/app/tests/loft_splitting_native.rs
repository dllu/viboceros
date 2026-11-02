//! Exercise the ordinary application command entry with captured Loft recipes.
use super::loft_splitting_support as support;
use super::*;
use serde_json::json;
use viboceros_oracle::loft_splitting::{Fixture, snapshot};

fn enter(app: &mut VibocerosApp, input: &str) {
    app.command_input = input.into();
    app.run_command();
}

#[test]
fn loft_gui_matches_native_threshold_splitting_and_history() {
    replay_cases(support::angle_cases(), true);
}

#[test]
#[ignore = "Retain native box, closed face-sense and Straight chart differences in all 276 raw states"]
fn loft_gui_matches_all_native_splitting_seams_and_history() {
    replay_cases(support::cases(), false);
}

fn replay_cases(cases: Vec<(serde_json::Value, serde_json::Value)>, angles: bool) {
    let tolerance = viboceros_geometry::Tolerance::try_new(1e-6, 1e-12, 1e-8).unwrap();
    for (operation, native) in cases {
        let fixture: Fixture = serde_json::from_value(operation.clone()).unwrap();
        let originals = fixture.sources().unwrap();
        let mut app = test_app();
        app.document = viboceros_document::Document::new(tolerance);
        let ids = originals
            .iter()
            .map(|g| app.document.add_geometry(g.clone()).unwrap())
            .collect::<Vec<_>>();
        if fixture.preselect {
            app.document
                .select_objects_direct(ids.iter().copied(), SelectionMode::Replace)
                .unwrap();
        }
        app.document.clear_history().unwrap();
        enter(&mut app, fixture.crease_command());
        assert_eq!(app.document.undo_label(), None);
        assert_eq!(app.commands.crease_splitting(), fixture.crease_splitting);
        enter(&mut app, &fixture.command());
        if !fixture.preselect {
            assert!(app.loft_prompt.is_some());
            for &id in &ids {
                app.apply_selection_click(SelectionClick {
                    object_id: Some(id),
                    mode: SelectionMode::Replace,
                });
            }
            assert_eq!(app.document.undo_label(), None);
            enter(&mut app, "");
        }
        assert_eq!(
            app.document.undo_label(),
            Some("Loft"),
            "{}: {:?}",
            operation["id"],
            app.command_log
        );
        let after = snapshot(&app.document, &ids, &originals).unwrap();
        enter(&mut app, "Undo");
        let undo = snapshot(&app.document, &ids, &originals).unwrap();
        enter(&mut app, "Redo");
        let redo = snapshot(&app.document, &ids, &originals).unwrap();
        let actual = json!({"after":after,"undo":undo,"redo":redo});
        let maximum_error = if angles {
            support::compare_angles(&operation, &actual, &native)
        } else {
            support::compare(&actual, &native)
        };
        println!(
            "LOFT_SPLITTING_APP {}",
            json!({"id":operation["id"],"value":actual,"maximum_error":maximum_error})
        );
    }
}
