#[path = "support/curve_boolean.rs"]
mod comparison;
#[path = "support/observation.rs"]
mod observation;
#[test]
fn curve_boolean_matches_native_regions_metadata_and_history() {
    let base = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tools/rhino_oracle");
    let mut failures = Vec::new();
    let mut maximum = 0f64;
    for (op, native) in comparison::rows(&base, observation::read) {
        let q = serde_json::json!({"protocol_version":1,"iterations":1,"tolerance":{"absolute":1e-6,"relative":1e-12,"angular":1e-6},"operations":[op]});
        match viboceros_oracle::run_request(&serde_json::from_value(q).unwrap()) {
            Ok(report) => {
                let actual = serde_json::to_value(report).unwrap()["results"][0]["value"].clone();
                match comparison::compare(&actual, &native) {
                    Ok(e) => {
                        maximum = maximum.max(e.model_error);
                        println!(
                            "CURVE_BOOLEAN_CASE {}",
                            serde_json::json!({"id":op["id"],"model_error":e.model_error,"area_error":e.area_error,"relative_area_error":e.relative_area_error,"topology_gap":e.topology_gap})
                        );
                        if let Err(error) = comparison::qualify(op["id"].as_str().unwrap(), e) {
                            failures.push(error);
                        }
                    }
                    Err(e) => failures.push(format!("{}: {e}", op["id"])),
                }
            }
            Err(e) => failures.push(format!("{}: {e:?}", op["id"])),
        }
    }
    assert!(failures.is_empty(), "{}", failures.join("\n"));
    println!("CurveBoolean sampled maximum model error: {maximum}");
}
