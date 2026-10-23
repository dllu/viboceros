//! Replay physical native edge-spin command workflows through app selection.
use super::native::fixture as native;
use super::*;
use serde_json::json;
#[allow(dead_code)]
#[path = "../../../../crates/viboceros-command/tests/support/subd_spin_edge.rs"]
mod fixtures;
fn interior_edge(app: &VibocerosApp, id: ObjectId) -> usize {
    let Geometry::SubD(s) = app.document.object(id).unwrap().geometry() else {
        panic!()
    };
    s.edge_face_counts().iter().position(|&n| n == 2).unwrap()
}
#[test]
fn direction_values_stay_in_the_spin_session_and_undo_preserves_one_history_step() {
    for option in ["Direction Clockwise", "Direction=Clockwise"] {
        let (mut app, id) = setup();
        let original = app.document.object(id).unwrap().geometry().clone();
        enter(&mut app, "SubDSpinEdge");
        enter(&mut app, option);
        assert!(app.picking_spin_edge(), "{:?}", app.command_log);
        let edge = interior_edge(&app, id);
        select(&mut app, id, ComponentSelectionKind::SubDEdge, edge, false);
        assert_eq!(app.document.object(id).unwrap().geometry(), &original);
        enter(&mut app, "Undo");
        assert!(app.picking_spin_edge());
        enter(&mut app, "Cancel");
        assert_eq!(app.document.undo_label(), Some("SubDSpinEdge"));
        assert_eq!(app.document.object(id).unwrap().geometry(), &original);
    }
}
#[test]
fn a_new_command_finishes_accepted_spins_before_running() {
    let (mut app, id) = setup();
    let original = app.document.object(id).unwrap().geometry().clone();
    enter(&mut app, "SubDSpinEdge");
    let edge = interior_edge(&app, id);
    select(&mut app, id, ComponentSelectionKind::SubDEdge, edge, false);
    enter(&mut app, "Point 8,9,10");
    assert!(app.crease_prompt.is_none());
    assert_eq!(app.document.undo_label(), Some("Point"));
    enter(&mut app, "Undo");
    assert_eq!(app.document.undo_label(), Some("SubDSpinEdge"));
    assert_ne!(app.document.object(id).unwrap().geometry(), &original);
    enter(&mut app, "Undo");
    assert_eq!(app.document.object(id).unwrap().geometry(), &original);
}
#[test]
fn edge_spin_getter_matches_native_preselection_direction_repeat_undo_and_escape() {
    let rows = fixtures::rows();
    assert_eq!(rows.len(), 31);
    let mut failures = Vec::new();
    for (family, row) in rows {
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            let v = &row["value"];
            let (doc, ids) = native::setup_from_family(&row, family);
            let mut app = test_app();
            app.document = doc;
            let mut maximum = native::check(&app.document, &v["before"], true);
            if v["input"]["live"] == true {
                enter(&mut app, "SubDSpinEdge");
                assert!(app.picking_spin_edge());
                for action in v["input"]["actions"].as_array().unwrap() {
                    if action.is_object() {
                        let target = fixtures::live_target(
                            &app.document,
                            ids[action["source"].as_u64().unwrap_or(0) as usize],
                        );
                        select(
                            &mut app,
                            target.object,
                            ComponentSelectionKind::SubDEdge,
                            target.edge,
                            false,
                        );
                    } else {
                        enter(&mut app, action.as_str().unwrap());
                    }
                }
            } else {
                for t in fixtures::initial_targets(&ids, v) {
                    select(
                        &mut app,
                        t.object,
                        ComponentSelectionKind::SubDEdge,
                        t.edge,
                        true,
                    );
                }
                enter(&mut app, "SubDSpinEdge");
                if app.picking_spin_edge() {
                    enter(&mut app, "");
                }
            }
            assert!(app.crease_prompt.is_none(), "{:?}", app.command_log);
            maximum = maximum.max(native::check(&app.document, &v["after"], true));
            let expected = v["after"]
                .as_array()
                .unwrap()
                .iter()
                .filter(|s| s["source"].is_number())
                .flat_map(|s| {
                    let object = ids[s["source"].as_u64().unwrap() as usize];
                    s["selected_edges"]
                        .as_array()
                        .unwrap()
                        .iter()
                        .map(move |i| (object, i.as_u64().unwrap() as usize))
                })
                .collect::<std::collections::BTreeSet<_>>();
            let actual = app
                .component_selection
                .checked_picks(&app.document)
                .unwrap()
                .into_iter()
                .map(|p| (p.object, p.index))
                .collect::<std::collections::BTreeSet<_>>();
            assert_eq!(actual, expected, "component selection");
            app.component_selection.clear();
            app.document.clear_selection();
            maximum = maximum.max(native::check(&app.document, &v["after_script"], true));
            enter(&mut app, "Undo");
            maximum = maximum.max(native::check(&app.document, &v["undo"], true));
            enter(&mut app, "Redo");
            maximum = maximum.max(native::check(&app.document, &v["redo"], true));
            println!(
                "SUBD_SPIN_GETTER {}",
                json!({"id":row["id"],"maximum_error":maximum,"identity":true,"metadata":true,"history":true,"selection":true})
            );
        }));
        if result.is_err() {
            failures.push(row["id"].as_str().unwrap().to_owned());
        }
    }
    assert!(
        failures.is_empty(),
        "SubD edge spin getter differences: {failures:?}"
    );
}
