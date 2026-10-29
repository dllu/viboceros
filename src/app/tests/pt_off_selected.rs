use super::*;
use serde_json::Value;
use viboceros_oracle::pt_off_selected::{Fixture, setup, snapshot};
#[path = "../../../crates/viboceros-oracle/tests/support/pt_off_selected.rs"]
mod support;
fn enter(app: &mut VibocerosApp, input: &str) {
    app.command_input = input.into();
    app.run_command();
}
#[test]
fn pt_off_selected_application_matches_all_41_native_getters_and_states() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tools/rhino_oracle");
    for family in ["pt_off_selected_qualified", "pt_off_selected_lifecycle"] {
        let q: Value = serde_json::from_str(
            &std::fs::read_to_string(root.join(format!("fixtures/{family}.json"))).unwrap(),
        )
        .unwrap();
        let expected: Value = serde_json::from_str(
            &std::fs::read_to_string(root.join(format!("observations/{family}.json"))).unwrap(),
        )
        .unwrap();
        for (op, row) in q["operations"]
            .as_array()
            .unwrap()
            .iter()
            .zip(expected["results"].as_array().unwrap())
        {
            let f = Fixture {
                kind: op["kind"].as_str().unwrap().into(),
                mode: op["mode"].as_str().unwrap().into(),
                pattern: op["pattern"].as_str().unwrap().into(),
            };
            let name = row["id"].as_str().unwrap();
            let native = &row["value"];
            let mut app = test_app();
            let (doc, ids, peer, marker) = setup(&f).unwrap();
            app.document = doc;
            let originals = ids.map(|id| app.document.object(id).unwrap().clone());
            let undo = app.document.undo_label().map(str::to_owned);
            support::compare(&snapshot(&app.document, &ids), &native["before"], name);
            enter(&mut app, "PtOffSelected");
            assert_eq!(
                app.object_prompt.is_some(),
                native["getter_opened"].as_bool().unwrap(),
                "{name}"
            );
            assert!(!app.document.can_redo(), "{name}");
            if app.object_prompt.is_some() {
                assert_eq!(
                    app.object_prompt.as_ref().unwrap().description.filter,
                    viboceros_command::ObjectSelectionFilter::PointDisplayOwners
                );
                // Ineligible peers cannot satisfy the owner getter.
                app.apply_selection_click(SelectionClick {
                    object_id: Some(peer),
                    mode: SelectionMode::Add,
                });
                if f.pattern.starts_with("postselect") || f.pattern.starts_with("postcancel") {
                    app.apply_selection_click(SelectionClick {
                        object_id: Some(ids[0]),
                        mode: SelectionMode::Replace,
                    });
                }
                enter(
                    &mut app,
                    if f.pattern.starts_with("postselect") {
                        ""
                    } else {
                        "Cancel"
                    },
                );
            }
            assert!(app.object_prompt.is_none(), "{name}: {:?}", app.command_log);
            support::compare(&snapshot(&app.document, &ids), &native["after"], name);
            assert_eq!(
                app.document.is_selected(peer),
                native["peer_selected"].as_bool().unwrap(),
                "{name}"
            );
            assert_eq!(app.document.undo_label(), Some("PtOffSelected"), "{name}");
            for (id, old) in ids.iter().zip(originals) {
                let now = app.document.object(*id).unwrap();
                assert!(
                    old.geometry_snapshot()
                        .shares_storage_with(now.geometry_snapshot())
                );
                assert_eq!(old.group_ids(), now.group_ids());
            }
            enter(&mut app, "Redo");
            assert_eq!(
                app.document.object(marker).is_some(),
                native["redo_preserved"].as_bool().unwrap(),
                "{name}"
            );
            enter(&mut app, "Undo");
            assert_eq!(
                app.document.undo_label(),
                undo.as_deref(),
                "{name}: exactly one command-start entry"
            );
            support::compare(&snapshot(&app.document, &ids), &native["after"], name);
            enter(&mut app, "Redo");
            support::compare(&snapshot(&app.document, &ids), &native["after"], name);
        }
    }
}

#[test]
fn pt_off_selected_selall_filters_active_displays_and_preserves_unrelated_grips() {
    let mut app = test_app();
    let f = Fixture {
        kind: "curve".into(),
        mode: "control".into(),
        pattern: "postselect-grip".into(),
    };
    let (doc, ids, peer, _) = setup(&f).unwrap();
    app.document = doc;
    app.document.disable_control_points_for_objects([ids[1]]);
    app.document
        .select_objects_direct([peer], SelectionMode::Add)
        .unwrap();
    enter(&mut app, "PtOffSelected");
    enter(&mut app, "SelAll");
    assert!(app.document.is_selected(ids[0]));
    assert!(!app.document.is_selected(ids[1]));
    enter(&mut app, "");
    assert!(app.document.control_point_locations(ids[0]).is_none());
    assert!(app.document.is_selected(peer));
    assert!(!app.document.is_selected(ids[0]));
}
