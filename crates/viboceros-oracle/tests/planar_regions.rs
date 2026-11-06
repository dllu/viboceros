#[path = "support/planar_regions.rs"]
mod comparison;
#[path = "support/observation.rs"]
mod observation;
use serde_json::Value;
#[test]
fn planar_regions_match_native_boundaries_sources_history_and_record_area_evidence() {
    let base = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tools/rhino_oracle");
    let mut failures = Vec::new();
    let mut area_gaps = Vec::new();
    for (family, count, epsilon) in [
        ("planar_regions_pilot", 49, 1e-5),
        ("planar_regions_holdouts", 69, 1e-9),
        ("planar_regions_confirmations", 32, 1e-9),
        ("planar_regions_planes", 8, 1e-9),
        ("planar_crossings", 48, 1e-9),
        ("planar_crossings_details", 8, 1e-9),
        ("planar_crossings_holdouts", 33, 1e-9),
    ] {
        let request: Value = serde_json::from_slice(
            &std::fs::read(base.join(format!("fixtures/{family}.json"))).unwrap(),
        )
        .unwrap();
        let native = observation::read(&base, family);
        assert_eq!(request["operations"].as_array().unwrap().len(), count);
        assert_eq!(native["results"].as_array().unwrap().len(), count);
        for (op, row) in request["operations"]
            .as_array()
            .unwrap()
            .iter()
            .zip(native["results"].as_array().unwrap())
        {
            let q = serde_json::json!({"protocol_version":1,"iterations":1,"tolerance":request["tolerance"],"operations":[op]});
            let report = viboceros_oracle::run_request(&serde_json::from_value(q).unwrap())
                .unwrap_or_else(|e| panic!("{}: {e:?}", op["id"]));
            let actual = serde_json::to_value(report).unwrap()["results"][0]["value"].clone();
            match comparison::compare(&actual, &row["value"], epsilon, op["id"].as_str().unwrap()) {
                Ok(0) => {}
                Ok(count) => area_gaps.push((op["id"].as_str().unwrap().to_owned(), count)),
                Err(e) => failures.push(format!("{}: {e}", op["id"])),
            }
        }
    }
    assert!(failures.is_empty(), "{}", failures.join("\n"));
    assert_eq!(area_gaps, vec![("crossing_circle10_small".to_owned(), 2)]);
}

#[test]
fn original_planar_surface_region_properties_match_in_both_builds() {
    let base = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tools/rhino_oracle");
    let query: Value = serde_json::from_slice(
        &std::fs::read(base.join("fixtures/planar_surface_native.json")).unwrap(),
    )
    .unwrap();
    let expected = observation::read(&base, "planar_surface_native");
    let actual = serde_json::to_value(
        viboceros_oracle::run_request(&serde_json::from_value(query).unwrap()).unwrap(),
    )
    .unwrap();
    assert_eq!(actual["results"].as_array().unwrap().len(), 6);
    for (a, b) in actual["results"]
        .as_array()
        .unwrap()
        .iter()
        .zip(expected["results"].as_array().unwrap())
    {
        assert_eq!(a["id"], b["id"]);
        let x = &a["value"];
        let y = &b["value"];
        assert_eq!(x["retained_sources"], y["retained_sources"]);
        let x = x["regions"].as_array().unwrap();
        let y = y["regions"].as_array().unwrap();
        assert_eq!(x.len(), y.len());
        for (x, y) in x.iter().zip(y) {
            assert_eq!(x["faces"], y["faces"]);
            assert_eq!(x["loops"], y["loops"]);
            assert!((x["area"].as_f64().unwrap() - y["area"].as_f64().unwrap()).abs() < 1e-10);
            for key in ["min", "max"] {
                for (x, y) in x[key]
                    .as_array()
                    .unwrap()
                    .iter()
                    .zip(y[key].as_array().unwrap())
                {
                    assert!((x.as_f64().unwrap() - y.as_f64().unwrap()).abs() < 1e-10);
                }
            }
        }
    }
}
