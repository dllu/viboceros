use serde_json::Value;
#[path = "support/observation.rs"]
mod observation;
#[path = "support/subd_volume.rs"]
mod support;
#[test]
fn native_subd_volume_admission_appearance_warnings_mixed_references_and_history_replay() {
    let base = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tools/rhino_oracle");
    let mut count = 0;
    for family in [
        "subd_volume_discovery",
        "subd_volume_open",
        "subd_volume_admission",
    ] {
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
        assert_eq!(
            actual["results"].as_array().unwrap().len(),
            native["results"].as_array().unwrap().len()
        );
        for ((a, b), op) in actual["results"]
            .as_array()
            .unwrap()
            .iter()
            .zip(native["results"].as_array().unwrap())
            .zip(request["operations"].as_array().unwrap())
        {
            count += 1;
            assert_eq!(a["id"], b["id"]);
            eprintln!("{}", a["id"]);
            let (a, b) = (&a["value"], &b["value"]);
            assert_eq!(a["success"], b["success"]);
            assert_eq!(a["source_unchanged"], true);
            assert_eq!(b["source_unchanged"], true);
            support::definition(&a["constructed"], &b["constructed"]);
            support::definition(&a["stored"], &b["stored"]);
            if !b["replacement"].is_null() {
                support::definition(&a["replacement"], &b["replacement"]);
            }
            for key in ["before", "after", "undo", "redo"] {
                support::state(&a[key], &b[key]);
            }
            assert_eq!(
                a["warning"].as_bool().unwrap(),
                op.get("open_confirmation").is_some()
            );
            if let Some(v) = support::value(b) {
                let actual = a["volume"].as_f64().unwrap();
                assert!(
                    (actual - v).abs() < 1e-6,
                    "{} printed volume: {actual} versus {v}",
                    op["id"]
                );
                if op["mixed"] == false {
                    let reference = b["diagnostics"]
                        .as_array()
                        .unwrap()
                        .iter()
                        .find(|d| d["mode"] == 3)
                        .unwrap()["volume"]
                        .as_f64()
                        .unwrap();
                    assert!(
                        (actual - reference).abs() < 1e-8,
                        "{} tight volume: {}",
                        op["id"],
                        actual - reference
                    );
                }
            } else {
                assert!(a["volume"].is_null());
            }
            if op["command"] == "VolumeCentroid" && b["success"] == true && op["mixed"] == false {
                let reference = &b["diagnostics"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .find(|d| d["mode"] == 3)
                    .unwrap()["centroid"];
                let point = &a["after"]["points"][0];
                for i in 0..3 {
                    assert!(
                        (point[i].as_f64().unwrap() - reference[i].as_f64().unwrap()).abs() < 1e-8,
                        "{} tight centroid {point} versus {reference}",
                        op["id"]
                    );
                }
            }
        }
    }
    assert_eq!(count, 58);
}
