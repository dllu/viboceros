use serde_json::Value;
use viboceros_oracle::{ProbeRequest, run_request};
#[path = "support/pt_off_selected.rs"]
mod support;
#[test]
fn selected_point_display_matches_all_41_native_states() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tools/rhino_oracle");
    let mut cases = 0;
    for (family, count) in [
        ("pt_off_selected_qualified", 31),
        ("pt_off_selected_lifecycle", 10),
    ] {
        let q: ProbeRequest = serde_json::from_str(
            &std::fs::read_to_string(root.join(format!("fixtures/{family}.json"))).unwrap(),
        )
        .unwrap();
        let expected: Value = serde_json::from_str(
            &std::fs::read_to_string(root.join(format!("observations/{family}.json"))).unwrap(),
        )
        .unwrap();
        let actual = serde_json::to_value(run_request(&q).unwrap()).unwrap();
        assert_eq!(actual["results"].as_array().unwrap().len(), count);
        for (a, b) in actual["results"]
            .as_array()
            .unwrap()
            .iter()
            .zip(expected["results"].as_array().unwrap())
        {
            assert_eq!(a["id"], b["id"]);
            support::compare(&a["value"], &b["value"], a["id"].as_str().unwrap());
            cases += 1;
        }
    }
    assert_eq!(cases, 41);
}

#[test]
fn command_start_history_matches_native_undo_labels_and_complete_grip_states() {
    use viboceros_command::CommandRegistry;
    use viboceros_document::Geometry;
    use viboceros_geometry::Point3;
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tools/rhino_oracle");
    let q: Value = serde_json::from_str(
        &std::fs::read_to_string(root.join("fixtures/pt_off_selected_command_history.json"))
            .unwrap(),
    )
    .unwrap();
    let r: Value = serde_json::from_str(
        &std::fs::read_to_string(root.join("observations/pt_off_selected_command_history.json"))
            .unwrap(),
    )
    .unwrap();
    assert_eq!(r["results"].as_array().unwrap().len(), 9);
    for (op, row) in q["operations"]
        .as_array()
        .unwrap()
        .iter()
        .zip(r["results"].as_array().unwrap())
    {
        let name = row["id"].as_str().unwrap();
        let native = &row["value"];
        let f = viboceros_oracle::pt_off_selected::Fixture {
            kind: "curve".into(),
            mode: "control".into(),
            pattern: op["pattern"]
                .as_str()
                .unwrap()
                .strip_prefix("command-undo-")
                .unwrap()
                .into(),
        };
        let (mut doc, ids, peer, _) = viboceros_oracle::pt_off_selected::setup(&f).unwrap();
        // Native controls have no older available Undo entry after the witness
        // setup. Compare that measured state, without treating the older anchor
        // as proof about preservation of arbitrary prior native model history.
        let anchor = doc
            .add_geometry(Geometry::Point(Point3::try_new(90., 90., 90.).unwrap()))
            .unwrap();
        doc.clear_history().unwrap();
        let marker = doc
            .add_geometry(Geometry::Point(Point3::try_new(99., 99., 99.).unwrap()))
            .unwrap();
        doc.undo().unwrap();
        doc.redo().unwrap();
        assert!(doc.object(marker).is_some());
        doc.undo().unwrap();
        support::compare(
            &viboceros_oracle::pt_off_selected::snapshot(&doc, &ids),
            &native["before"],
            name,
        );
        let commands = CommandRegistry::with_builtins();
        match op["control_action"].as_str().unwrap() {
            "none" => {}
            "PointsOff" => {
                commands.execute(&mut doc, "PointsOff").unwrap();
            }
            "PtOffSelected" => {
                let p = commands
                    .object_selection_prompt("PtOffSelected")
                    .unwrap()
                    .unwrap();
                if doc
                    .selected_objects()
                    .any(|o| p.filter.accepts_in_document(&doc, o))
                {
                    commands.execute(&mut doc, "PtOffSelected").unwrap();
                } else {
                    commands.begin_object_selection(&mut doc, &p).unwrap();
                }
            }
            _ => unreachable!(),
        }
        support::compare(
            &viboceros_oracle::pt_off_selected::snapshot(&doc, &ids),
            &native["after"],
            name,
        );
        assert_eq!(
            doc.is_selected(peer),
            native["peer_selected"].as_bool().unwrap()
        );
        doc.redo().unwrap();
        let redone = doc.object(marker).is_some();
        assert_eq!(
            redone,
            native["redo_preserved"].as_bool().unwrap(),
            "{name}"
        );
        if redone {
            doc.undo().unwrap();
        }
        let undone = doc.undo().unwrap();
        assert_eq!(
            undone.is_some(),
            native["undo_called"].as_bool().unwrap(),
            "{name}"
        );
        if undone.is_some() {
            assert_eq!(undone.as_deref(), Some("PtOffSelected"));
        }
        support::compare(
            &viboceros_oracle::pt_off_selected::snapshot(&doc, &ids),
            &native["undo_state"],
            name,
        );
        assert!(doc.object(anchor).is_some());
        assert!(doc.undo().unwrap().is_none());
        support::compare(
            &viboceros_oracle::pt_off_selected::snapshot(&doc, &ids),
            &native["second_undo_state"],
            name,
        );
    }
}
