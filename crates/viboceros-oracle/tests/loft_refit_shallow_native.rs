//! Explicit shallow corners use the same complete-record qualification as Loft.
use serde_json::{Value, json};
use std::path::Path;
use viboceros_oracle::{ProbeRequest, run_request};
#[allow(dead_code)]
#[path = "support/loft_refit_admission.rs"]
mod admission;
use admission::comparison;

#[test]
fn shallow_corners_and_suppressed_fits_retain_native_definitions() {
    compare("loft_refit_shallow_angles", 36, (34, 2));
}

#[test]
fn minimum_angle_holdouts_match_native_regions_and_rational_clocks() {
    compare("loft_refit_shallow_holdouts", 48, (46, 2));
}

#[test]
fn measured_intrinsic_boundary_retains_scale_sensitive_native_references() {
    let result = admission::compare("loft_refit_corner_boundary", 0..22);
    assert_eq!(result.legacy, 14);
}

#[test]
fn explicit_angles_above_the_floor_control_corner_suppression() {
    compare("loft_refit_corner_suppression", 25, (25, 0));
}

#[test]
fn coarse_initial_cubics_can_smooth_explicitly_suppressed_larger_corners() {
    compare("loft_refit_suppressed_initial", 12, (12, 0));
}

#[test]
fn corrected_probe_records_expose_two_newly_supported_explicit_corners() {
    compare("loft_refit_matched_probe_angles", 10, (10, 0));
}

fn compare(family: &str, count: usize, counts: (usize, usize)) {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tools/rhino_oracle");
    let read = |folder| -> Value {
        serde_json::from_slice(
            &std::fs::read(root.join(format!("{folder}/{family}.json"))).unwrap(),
        )
        .unwrap()
    };
    let query = read("fixtures");
    let native = read("observations");
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
                let mut expected = row["value"].clone();
                for key in ["refitted", "refit_frames", "refit_probe_angle_tolerance"] {
                    expected.as_object_mut().unwrap().remove(key);
                }
                let difference = comparison::difference(value, &expected)
                    .unwrap_or_else(|e| panic!("{}: {e}", op["id"]));
                assert!(difference <= 1e-9, "{}: {difference}", op["id"]);
                assert_eq!(value["before"], value["after"]);
                maximum = maximum.max(difference);
                matched += 1;
            }
            Err(error) => {
                assert!(
                    error.to_string().contains("Loft Refit currently requires"),
                    "{}: {error}",
                    op["id"]
                );
                println!("SHALLOW_REFIT_PENDING {}", op["id"]);
                pending += 1;
            }
        }
    }
    println!("SHALLOW_REFIT {family} matched={matched} pending={pending} maximum={maximum}");
    assert_eq!((matched, pending), counts);
}
