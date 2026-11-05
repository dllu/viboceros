use serde_json::Value;
#[path = "support/observation.rs"]
mod observation;

#[path = "support/subd_direction.rs"]
mod support;
use support::compare;

#[test]
fn native_subd_flip_and_dir_selection_topology_metadata_cancellation_and_redo() {
    let base = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tools/rhino_oracle");
    let mut count = 0;
    for family in ["subd_direction_discovery", "subd_direction_controls"] {
        let request: Value = serde_json::from_slice(
            &std::fs::read(base.join(format!("fixtures/{family}.json"))).unwrap(),
        )
        .unwrap();
        let native = observation::read(&base, family);
        let actual = serde_json::to_value(
            viboceros_oracle::run_request(&serde_json::from_value(request.clone()).unwrap())
                .unwrap(),
        )
        .unwrap();
        for ((a, b), op) in actual["results"]
            .as_array()
            .unwrap()
            .iter()
            .zip(native["results"].as_array().unwrap())
            .zip(request["operations"].as_array().unwrap())
        {
            count += 1;
            assert_eq!(a["id"], b["id"]);
            let path = a["id"].as_str().unwrap();
            let (a, b) = (&a["value"], &b["value"]);
            assert_eq!(a["changed"], b["changed"], "{path}");
            // An accepted SubD-only Dir ends with native Nothing, even after
            // changing geometry. Raw events and success remain frozen evidence.
            assert_eq!(a["completed"], op["cancel"] == false, "{path}");
            if op["command"] == "Dir" && op["mixed"] == false {
                assert_eq!(b["success"], false, "{path}");
            } else if op["cancel"] == false {
                assert_eq!(b["success"], true, "{path}");
            }
            for phase in ["before", "after", "undo", "redo"] {
                compare(&a[phase], &b[phase], &format!("{path}/{phase}"));
            }
        }
    }
    assert_eq!(count, 82);
}
