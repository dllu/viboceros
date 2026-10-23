//! Fixed edge-spin workflows shared by command and frontend qualification.
use serde_json::Value;
use viboceros_command::subd_spin_edge::{Direction, SpinSession, Target};
use viboceros_document::{Document, Geometry, ObjectId};
pub fn rows() -> Vec<(&'static str, Value)> {
    let mut result = Vec::new();
    for (family, data) in [
        (
            "subd_spin_edge",
            include_str!("../../../../tools/rhino_oracle/observations/subd_spin_edge.json"),
        ),
        (
            "subd_spin_edge_live_v3",
            include_str!("../../../../tools/rhino_oracle/observations/subd_spin_edge_live_v3.json"),
        ),
        (
            "subd_spin_edge_holdouts",
            include_str!(
                "../../../../tools/rhino_oracle/observations/subd_spin_edge_holdouts.json"
            ),
        ),
        (
            "subd_spin_edge_live_v5",
            include_str!("../../../../tools/rhino_oracle/observations/subd_spin_edge_live_v5.json"),
        ),
    ] {
        let capture: Value = serde_json::from_str(data).unwrap();
        result.extend(
            capture["results"]
                .as_array()
                .unwrap()
                .iter()
                .filter(|r| {
                    let v = &r["value"];
                    r["id"] != "spin_live_v5_undo_both"
                        && !v["history"].as_str().unwrap().contains("Unknown command")
                        && v["after"]
                            .as_array()
                            .unwrap()
                            .iter()
                            .all(|r| r.get("subd").is_none_or(|s| s["valid"] == true))
                        && (v["input"]["live"] == true
                            || v["input"].get("command_arguments").is_none()
                                && v["input"]["cancel"] != true)
                })
                .cloned()
                .map(|r| (family, r)),
        );
    }
    result
}
pub fn initial_targets(ids: &[ObjectId], value: &Value) -> Vec<Target> {
    value["before"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|r| r["source"].is_number())
        .flat_map(|row| {
            let object = ids[row["source"].as_u64().unwrap() as usize];
            row["selected_edges"]
                .as_array()
                .unwrap()
                .iter()
                .map(move |i| Target {
                    object,
                    edge: i.as_u64().unwrap() as usize,
                })
        })
        .collect()
}
pub fn live_target(doc: &Document, id: ObjectId) -> Target {
    let Geometry::SubD(s) = doc.object(id).unwrap().geometry() else {
        panic!()
    };
    Target {
        object: id,
        edge: s
            .edges()
            .iter()
            .position(|&e| e == [1, 4] || e == [4, 1])
            .unwrap(),
    }
}
pub fn replay(session: &mut SpinSession, doc: &Document, ids: &[ObjectId], value: &Value) {
    if value["input"]["live"] == true {
        for action in value["input"]["actions"].as_array().unwrap() {
            if action.is_object() {
                session
                    .spin(
                        doc,
                        live_target(doc, ids[action["source"].as_u64().unwrap_or(0) as usize]),
                    )
                    .unwrap();
            } else {
                match action.as_str().unwrap() {
                    "Direction" => {
                        session.direction = if session.direction == Direction::Clockwise {
                            Direction::CounterClockwise
                        } else {
                            Direction::Clockwise
                        }
                    }
                    "SpinEdgeAgain" => {
                        session.spin_again(doc).unwrap();
                    }
                    "Undo" => {
                        assert!(session.undo_spin());
                    }
                    "Enter" | "Cancel" => {}
                    _ => panic!("unexpected fixed action"),
                }
            }
        }
    } else {
        let targets = initial_targets(ids, value);
        if targets.len() == 1 {
            session.spin(doc, targets[0]).unwrap();
        }
    }
}
