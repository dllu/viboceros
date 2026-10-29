//! Complete native image gates through the public section RPC on either backend.
use section_refit::definitions;
use serde_json::json;
use viboceros_geometry::SweepSection;
use viboceros_oracle::{ProbeRequest, run_request};
#[allow(dead_code)]
#[path = "../../viboceros-geometry/tests/support/section_refit.rs"]
mod section_refit;

fn replay(family: &str, expected: usize) {
    replay_cases(family, expected, 0..expected);
}

fn replay_cases(family: &str, expected: usize, cases: std::ops::Range<usize>) {
    section_refit::require_complete_family_with_cases(family, expected, cases, |op, sections| {
        let request: ProbeRequest = serde_json::from_value(json!({
            "protocol_version":1,"iterations":1,
            "tolerance":{"absolute":1e-7,"relative":1e-10,"angular":1e-10},
            "operations":[op],
        }))
        .unwrap();
        let before = request.clone();
        let response = run_request(&request).map_err(|error| error.to_string())?;
        assert_eq!(request, before);
        assert_eq!(response.results.len(), 1);
        assert_eq!(response.results[0].id, op["id"].as_str().unwrap());
        let value = &response.results[0].value;
        assert_eq!(value["parameters"], op["parameters"]);
        assert_eq!(value["before"], value["after"]);
        let curves = value["curves"].as_array().unwrap();
        assert_eq!(curves.len(), sections.len());
        for (native_input, source) in value["before"].as_array().unwrap().iter().zip(sections) {
            assert_eq!(definitions::curve(native_input), source.curve);
        }
        Ok(curves
            .iter()
            .zip(sections)
            .map(|(curve, source)| SweepSection {
                parameter: source.parameter,
                curve: definitions::curve(curve),
            })
            .collect())
    });
}

#[test]
fn large_tolerances_and_endpoint_frames_match_complete_native_curves() {
    for (family, count) in [
        ("sweep1_refit_large_tolerances", 50),
        ("sweep1_refit_large_holdouts", 16),
        ("sweep1_refit_endpoint_frames", 24),
        ("sweep1_refit_opposed_frames", 18),
        ("sweep1_refit_opposed_boundary", 6),
    ] {
        replay(family, count);
    }
}

#[test]
fn opposition_cosine_boundary_matches_complete_native_curves() {
    replay("sweep1_refit_opposed_cosines", 10);
}

#[test]
fn proposal_reentry_matches_complete_native_curves() {
    replay("sweep1_refit_proposal_reentry", 42);
}

#[test]
fn proposal_reentry_no_fit_preparation_matches_complete_native_curves() {
    replay("sweep1_refit_opposed_preparation", 14);
}

macro_rules! independent_cases {
    ($($name:ident = $index:literal;)*) => {
        const _: () = {
            let indices = [$($index,)*];
            assert!(indices.len() == 16);
            let mut i = 0;
            while i < indices.len() { assert!(indices[i] == i); i += 1; }
        };
        $(#[test] fn $name() {
            replay_cases("sweep1_refit_independent_profiles",16,$index..$index+1);
        })*
    }
}
independent_cases! {
    asymmetric_quadratic_fine_single = 0;
    asymmetric_quadratic_fine_pair = 1;
    asymmetric_quadratic_coarser_single = 2;
    asymmetric_quadratic_coarser_pair = 3;
    spatial_quadratic_fine_single = 4;
    spatial_quadratic_fine_pair = 5;
    spatial_quadratic_coarser_single = 6;
    spatial_quadratic_coarser_pair = 7;
    translated_circle_fine_single = 8;
    translated_circle_fine_pair = 9;
    translated_circle_coarser_single = 10;
    translated_circle_coarser_pair = 11;
    closed_cubic_fine_single = 12;
    closed_cubic_fine_pair = 13;
    closed_cubic_coarser_single = 14;
    closed_cubic_coarser_pair = 15;
}
