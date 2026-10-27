//! A certified joining proposal is distinct from native adaptive acceptance.
use serde_json::json;
use viboceros_geometry::try_propose_joined_linear_refit;
use viboceros_oracle::{ProbeRequest, run_request};
#[path = "support/loft_refit_proposals.rs"]
mod proposal_comparison;
use proposal_comparison::{difference, read};

#[test]
fn retimed_hermite_proposals_reproduce_native_c0_and_c1_scale_boundaries() {
    proposals("loft_refit_join_scales", 20, 20);
    proposals("loft_refit_join_curvature", 11, 11);
}

#[test]
fn uneven_region_counterexamples_keep_command_acceptance_unqualified() {
    proposals("loft_refit_unequal_sampling", 24, 13);
}

fn proposals(family: &str, count: usize, expected_matches: usize) {
    proposal_comparison::compare(
        family,
        count,
        (expected_matches, count - expected_matches, 0),
        "JOIN_PROPOSAL",
        try_propose_joined_linear_refit,
    );
}

#[test]
fn native_fitting_floor_preserves_complete_surface_records() {
    for (family, count, expected_counts) in [
        ("loft_refit_tiny_tolerances", 24, (22, 2)),
        ("loft_refit_tolerance_floor", 18, (12, 6)),
    ] {
        let query = read(family, "fixtures");
        let native = read(family, "observations");
        assert_eq!(query["operations"].as_array().unwrap().len(), count);
        assert_eq!(native["results"].as_array().unwrap().len(), count);
        let (mut matched, mut pending, mut maximum) = (0, 0, 0_f64);
        for (op, row) in query["operations"]
            .as_array()
            .unwrap()
            .iter()
            .zip(native["results"].as_array().unwrap())
        {
            assert_eq!(op["id"], row["id"]);
            let mut request = query.clone();
            request["operations"] = json!([op]);
            match run_request(&serde_json::from_value::<ProbeRequest>(request).unwrap()) {
                Ok(actual) => {
                    let value = &actual.results[0].value;
                    assert_eq!(value["before"], value["after"]);
                    let d = difference(value, &row["value"])
                        .unwrap_or_else(|e| panic!("{}: {e}", op["id"]));
                    assert!(d <= 1e-9, "{}: {d}", op["id"]);
                    maximum = maximum.max(d);
                    matched += 1;
                }
                Err(error) => {
                    assert!(op["refit_tolerance"].as_f64().unwrap() > 2. * f64::EPSILON.sqrt());
                    assert!(
                        error.to_string().contains("Loft Refit currently requires"),
                        "{}: {error}",
                        op["id"]
                    );
                    pending += 1;
                }
            }
        }
        println!("REFIT_FLOOR {family} matched={matched} pending={pending} maximum={maximum}");
        assert_eq!((matched, pending), expected_counts);
    }
}
