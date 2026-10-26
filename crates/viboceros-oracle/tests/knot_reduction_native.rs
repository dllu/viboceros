//! SDK range policy, source purity, and independently certified reductions.
use serde_json::{Value, json};
use std::path::Path;
use viboceros_oracle::{ProbeRequest, run_request};
#[allow(dead_code)]
#[path = "../../viboceros-geometry/tests/support/loft_rebuild.rs"]
mod comparison;

fn fixtures() -> (Value, Value) {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tools/rhino_oracle");
    let read = |folder| {
        serde_json::from_slice(
            &std::fs::read(root.join(format!("{folder}/knot_reduction.json"))).unwrap(),
        )
        .unwrap()
    };
    (read("fixtures"), read("observations"))
}

#[test]
fn native_ranges_match_except_for_certifiably_unsafe_polyline_removals() {
    let (query, native) = fixtures();
    let mut matched = 0;
    let mut conservative = 0;
    let mut bounded = 0;
    let mut unrestricted = 0;
    let mut maximum = 0_f64;
    assert_eq!(query["operations"].as_array().unwrap().len(), 140);
    assert_eq!(native["results"].as_array().unwrap().len(), 140);
    for (op, row) in query["operations"]
        .as_array()
        .unwrap()
        .iter()
        .zip(native["results"].as_array().unwrap())
    {
        assert_eq!(op["id"], row["id"]);
        let mut request = query.clone();
        request["operations"] = json!([op]);
        let actual = run_request(&serde_json::from_value::<ProbeRequest>(request).unwrap())
            .unwrap_or_else(|e| panic!("{}: {e}", op["id"]));
        let value = &actual.results[0].value;
        assert_eq!(value["before"], value["after"]);
        assert_eq!(row["value"]["before"], row["value"]["after"]);
        let limit = op["absolute_tolerance"].as_f64().unwrap();
        let source = comparison::definitions::curve(&value["before"]);
        let curve = comparison::definitions::curve(&value["curve"]);
        if limit > 0. {
            let bound = source
                .parameter_curve_deviation_bound(&curve, limit)
                .unwrap();
            assert!(bound.is_some_and(|b| b <= limit), "{}: {bound:?}", op["id"]);
            bounded += 1;
        } else {
            unrestricted += 1;
        }
        if op["id"]
            .as_str()
            .unwrap()
            .starts_with("knot-reduction-linear-")
            && limit > 0.
            && limit < 10.
            && op["maximum_multiplicity"].as_u64().unwrap() > 1
        {
            assert_eq!(row["value"]["removed"], 3);
            assert!(value["removed"].as_u64().unwrap() < 3);
            let unsafe_curve = comparison::definitions::curve(&row["value"]["curve"]);
            assert!(
                source
                    .parameter_curve_deviation_bound(&unsafe_curve, limit)
                    .unwrap()
                    .is_none()
            );
            conservative += 1;
        } else {
            let error = comparison::difference(value, &row["value"])
                .unwrap_or_else(|e| panic!("{}: {e}", op["id"]));
            assert!(error <= 1e-8, "{}: {error}", op["id"]);
            maximum = maximum.max(error);
            matched += 1;
        }
    }
    println!(
        "KNOT_REDUCTION matched={matched} conservative={conservative} bounded={bounded} unrestricted={unrestricted} maximum={maximum}"
    );
    assert_eq!(
        (matched, conservative, bounded, unrestricted),
        (134, 6, 112, 28)
    );
}

#[test]
fn invalid_arguments_fail_and_defaults_select_multiple_knots() {
    let (query, _) = fixtures();
    let base = query["operations"][0].clone();
    for changes in [
        json!({"absolute_tolerance":-1.}),
        json!({"minimum_multiplicity":2,"maximum_multiplicity":2}),
        json!({"minimum_multiplicity":4,"maximum_multiplicity":null}),
    ] {
        let mut op = base.clone();
        for (key, value) in changes.as_object().unwrap() {
            op[key] = value.clone();
        }
        let mut request = query.clone();
        request["operations"] = json!([op]);
        assert!(run_request(&serde_json::from_value::<ProbeRequest>(request).unwrap()).is_err());
    }
    let mut op = base;
    op.as_object_mut().unwrap().remove("minimum_multiplicity");
    op.as_object_mut().unwrap().remove("maximum_multiplicity");
    let mut request = query;
    request["operations"] = json!([op]);
    let actual = run_request(&serde_json::from_value::<ProbeRequest>(request).unwrap()).unwrap();
    assert_eq!(actual.results[0].value["removed"], 3);
}
