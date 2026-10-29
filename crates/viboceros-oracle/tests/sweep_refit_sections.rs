//! Exercise the section-only RPC against frozen native compatible definitions.
use serde_json::{Value, json};
use viboceros_oracle::{ProbeRequest, run_request};
#[allow(dead_code)]
#[path = "../../viboceros-geometry/tests/support/loft_parameters.rs"]
mod definitions;

fn read(folder: &str) -> Value {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tools/rhino_oracle");
    serde_json::from_slice(
        &std::fs::read(root.join(format!("{folder}/sweep1_refit_sections.json"))).unwrap(),
    )
    .unwrap()
}

#[test]
fn coarse_and_disabled_refits_preserve_authored_parameters_inputs_and_complete_curves() {
    let query = read("fixtures");
    let observed = read("observations");
    assert_eq!(query["operations"].as_array().unwrap().len(), 19);
    let mut cases = 0;
    let mut curves = 0;
    for (operation, row) in query["operations"]
        .as_array()
        .unwrap()
        .iter()
        .zip(observed["results"].as_array().unwrap())
    {
        assert_eq!(operation["id"], row["id"]);
        let tolerance = operation["refit_tolerance"].as_f64().unwrap();
        // The fine cases retain their separate required direct-curve failure
        // gate. This checks RPC behavior on previously qualified geometry and
        // the new disabled-fitting controls, including threshold equality.
        if tolerance > 2. * f64::EPSILON.sqrt() && tolerance < 0.001 {
            continue;
        }
        let mut single = query.clone();
        single["operations"] = json!([operation]);
        let request: ProbeRequest = serde_json::from_value(single).unwrap();
        let response = run_request(&request).unwrap();
        let actual = &response.results[0].value;
        let native = &row["value"];
        assert_eq!(actual["parameters"], operation["parameters"]);
        assert_eq!(actual["parameters"], native["parameters"]);
        assert_eq!(actual["before"], actual["after"]);
        assert_eq!(native["before"], native["after"]);
        let a = actual["curves"].as_array().unwrap();
        let b = native["curves"].as_array().unwrap();
        assert_eq!(a.len(), operation["sections"].as_array().unwrap().len());
        assert_eq!(a.len(), b.len());
        for (a, b) in a.iter().zip(b) {
            let bound = definitions::curve(a)
                .parameter_curve_deviation_bound(&definitions::curve(b), 1e-7)
                .unwrap()
                .unwrap_or_else(|| panic!("{} complete curve", operation["id"]));
            assert!(bound <= 1e-7);
            curves += 1;
        }
        cases += 1;
    }
    assert_eq!((cases, curves), (11, 17));
    println!("SECTION_REFIT_RPC cases={cases} complete_curves={curves}");
}

#[test]
fn invalid_section_station_metadata_stops_before_curve_construction() {
    let mut query = read("fixtures");
    let mut operation = query["operations"][0].clone();
    operation["sections"][0]["degree"] = json!(0);
    operation["parameters"] = json!([]);
    query["operations"] = json!([operation]);
    let request: ProbeRequest = serde_json::from_value(query).unwrap();
    let error = run_request(&request).unwrap_err().to_string();
    assert!(
        error.contains("section fitting count, parameters or tolerance"),
        "{error}"
    );
}
