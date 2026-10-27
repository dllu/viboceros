//! Curved Refit must not silently run an ordinary Loft.
use serde_json::{Value, json};
use std::path::Path;
use viboceros_oracle::{ProbeRequest, run_request};
#[allow(dead_code)]
#[path = "../../viboceros-geometry/tests/support/loft_rebuild.rs"]
mod comparison;
#[path = "support/loft_refit_legacy.rs"]
mod legacy;

fn fixture() -> (Value, Value) {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tools/rhino_oracle");
    let read = |folder| {
        serde_json::from_slice(
            &std::fs::read(root.join(format!("{folder}/loft_refit.json"))).unwrap(),
        )
        .unwrap()
    };
    (read("fixtures"), read("observations"))
}

#[test]
fn zero_refit_matches_all_native_ordinary_lofts_and_preserves_sources() {
    let (query, native) = fixture();
    let mut count = 0;
    let mut maximum = 0_f64;
    for (op, row) in query["operations"]
        .as_array()
        .unwrap()
        .iter()
        .zip(native["results"].as_array().unwrap())
    {
        if op["refit_tolerance"].as_f64() != Some(0.) {
            continue;
        }
        assert_eq!(op["id"], row["id"]);
        let mut request = query.clone();
        request["operations"] = json!([op]);
        let request: ProbeRequest = serde_json::from_value(request).unwrap();
        let actual = run_request(&request).unwrap().results[0].value.clone();
        let difference = comparison::difference(&actual, &row["value"])
            .unwrap_or_else(|e| panic!("{}: {e}", op["id"]));
        assert!(difference <= 1e-9, "{}: {difference}", op["id"]);
        assert_eq!(actual["before"], actual["after"]);
        count += 1;
        maximum = maximum.max(difference);
    }
    assert_eq!(count, 25);
    println!("LOFT_REFIT_ZERO matched={count} maximum={maximum}");
}

#[test]
fn initial_curved_sdk_and_document_fits_retain_native_records() {
    let (query, native) = fixture();
    let native_commands: Value = serde_json::from_slice(
        &std::fs::read(
            Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("../../tools/rhino_oracle/observations/loft_refit_adjustment_commands.json"),
        )
        .unwrap(),
    )
    .unwrap();
    let mut matched = 0;
    let mut pending = 0;
    let mut maximum = 0_f64;
    for (op, row) in query["operations"]
        .as_array()
        .unwrap()
        .iter()
        .zip(native["results"].as_array().unwrap())
    {
        if op["refit_tolerance"].as_f64().unwrap() <= 0.
            || op["id"].as_str().unwrap().starts_with("loft-refit-line-")
            || op["id"]
                .as_str()
                .unwrap()
                .starts_with("loft-refit-polygon-")
        {
            continue;
        }
        // The captured command reference below is the initial quarter fit.
        // Adaptive cases use their own complete SDK/document records instead.
        if !legacy::initially_admitted("loft_refit", &op["id"]) {
            pending += 2;
            continue;
        }
        for command in [false, true] {
            let mut op = op.clone();
            op["command"] = json!(command);
            let mut request = query.clone();
            request["operations"] = json!([op]);
            let request: ProbeRequest = serde_json::from_value(request).unwrap();
            match run_request(&request) {
                Ok(actual) => {
                    let expected = if command {
                        let id = format!(
                            "loft-refit-adjustment-command-quarter-{}",
                            op["style"].as_str().unwrap()
                        );
                        &native_commands["results"]
                            .as_array()
                            .unwrap()
                            .iter()
                            .find(|r| r["id"] == id)
                            .unwrap()["value"]
                    } else {
                        &row["value"]
                    };
                    let mut expected = expected.clone();
                    expected.as_object_mut().unwrap().remove("refitted");
                    let value = &actual.results[0].value;
                    let error = comparison::difference(value, &expected).unwrap();
                    // Newly supported corrections use the recorded 1e-8 native
                    // parity reserve; acceptance uses the continuous certificate.
                    assert!(error <= 1e-8, "{} command={command}: {error}", op["id"]);
                    if !command {
                        assert_eq!(value["before"], value["after"]);
                    }
                    maximum = maximum.max(error);
                    matched += 1;
                }
                Err(error) => {
                    assert!(
                        error.to_string().contains("Loft Refit currently requires"),
                        "{error}"
                    );
                    pending += 1;
                }
            }
        }
    }
    assert_eq!((matched, pending), (10, 80));
    println!("LOFT_REFIT_CURVED matched={matched} pending={pending} maximum={maximum}");
}

#[test]
fn invalid_and_conflicting_simplification_is_rejected_before_geometry() {
    let (query, _) = fixture();
    for (refit, rebuild, reason) in [
        (-1., 0, "nonnegative finite"),
        (0., 4, "mutually exclusive"),
        (0.1, 4, "mutually exclusive"),
    ] {
        let mut op = query["operations"][0].clone();
        op["curves"] = json!([]); // Invalid geometry must never be reached.
        op["refit_tolerance"] = json!(refit);
        op["rebuild_point_count"] = json!(rebuild);
        let mut request = query.clone();
        request["operations"] = json!([op]);
        let request: ProbeRequest = serde_json::from_value(request).unwrap();
        let error = run_request(&request).unwrap_err().to_string();
        assert!(error.contains(reason), "{error}");
    }
    for angle in [-1., 4.] {
        let mut op = query["operations"][0].clone();
        op["curves"] = json!([]);
        op["refit_tolerance"] = json!(0.001);
        op["refit_angle_tolerance"] = json!(angle);
        let mut request = query.clone();
        request["operations"] = json!([op]);
        let error = run_request(&serde_json::from_value::<ProbeRequest>(request).unwrap())
            .unwrap_err()
            .to_string();
        assert!(error.contains("angle tolerance"), "{error}");
    }
}
