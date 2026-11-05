use serde_json::Value;
#[path = "support/observation.rs"]
mod observation;
#[path = "support/subd_area.rs"]
mod support;
#[test]
fn subd_area_and_centroid_match_native_appearance_continuity_selection_and_history() {
    let base = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tools/rhino_oracle");
    let mut count = 0;
    let mut retained = Vec::new();
    for family in ["subd_area_discovery", "subd_area_followups"] {
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
            assert_eq!(a["id"], b["id"]);
            assert_eq!(a["value"]["source_unchanged"], true);
            let diagnostics = &b["value"]["diagnostics"];
            for (a, b) in a["value"]["commands"]
                .as_array()
                .unwrap()
                .iter()
                .zip(b["value"]["commands"].as_array().unwrap())
            {
                eprintln!("{} {}", op["id"], a["command"]);
                count += 1;
                assert_eq!(a["success"], b["success"]);
                assert_eq!(a["source_unchanged"], true);
                let mixed =
                    (op["curve"] == true && a["command"] == "AreaCentroid").then_some(diagnostics);
                let mut strict = true;
                for key in ["before", "after", "undo", "redo"] {
                    strict &= support::state(&a[key], &b[key], mixed);
                }
                if !strict {
                    retained.push(op["id"].as_str().unwrap().to_owned());
                }
                if a["command"] == "Area" {
                    let value = a["area"].as_f64().unwrap();
                    assert!(
                        (value - support::area(b)).abs() < 1e-6,
                        "{} actual command area",
                        op["id"]
                    );
                    if !op["curve"].as_bool().unwrap_or(false) {
                        let reference = &diagnostics
                            .as_array()
                            .unwrap()
                            .iter()
                            .find(|r| r["mode"] == 3)
                            .unwrap()["area"];
                        assert!(
                            (value - reference.as_f64().unwrap()).abs() < 1e-8,
                            "{} tight public G1x area: delta {}",
                            op["id"],
                            value - reference.as_f64().unwrap()
                        );
                    }
                }
            }
        }
    }
    assert_eq!(count, 38);
    assert_eq!(retained, ["bent_grid_mixed", "cube_mixed"]);
}
