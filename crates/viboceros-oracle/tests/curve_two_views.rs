#![cfg(feature = "native-smlib")]
//! Experimental producer measurements. A bounded recorded gap is not parity.
use serde_json::Value;
#[path = "support/curve_boolean.rs"]
#[allow(dead_code)]
mod comparison;
#[path = "support/observation.rs"]
mod observation;
#[path = "support/curve_two_views.rs"]
mod queries;
fn sources_only(value: &Value) -> Value {
    let mut result = value.clone();
    for phase in ["before", "after", "undo", "redo"] {
        result[phase]["objects"]
            .as_array_mut()
            .unwrap()
            .retain(|o| o["source"].is_number());
    }
    result
}
fn ceiling(id: &str) -> Option<f64> {
    Some(match id {
        "two_views_cubic"
        | "two_views_reverse_second"
        | "two_views_domains"
        | "two_views_opposite"
        | "two_views_rotated"
        | "two_views_remote"
        | "two_views_auto"
        | "two_views_preselected" => 5.32e-5,
        "two_views_rational_shared" => 6.24e-5,
        "two_views_rational_distinct" => 6.44e-5,
        "two_views_different_clock" => 9.66e-5,
        "two_views_partial" => 1.046e-4,
        "two_views_turning" => 5.44e-5,
        "two_views_line_and_cubic" => 3.57e-5,
        "two_views_circles_unequal" => 3.62e-5,
        "two_views_oblique" => 4.24e-5,
        "two_views_temporary_first" => 1.34e-5,
        "two_views_temporary_both" | "two_views_temporary_reverse" => 1.45e-5,
        _ => return None,
    })
}
#[test]
fn experimental_two_view_producer_retains_native_gaps_and_strict_source_history() {
    let base = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tools/rhino_oracle");
    let mut query: Value =
        serde_json::from_slice(&std::fs::read(base.join("fixtures/curve_two_views.json")).unwrap())
            .unwrap();
    // Native cancellation captures are retained; there is no local getter yet.
    query["operations"]
        .as_array_mut()
        .unwrap()
        .retain(|o| o.get("cancel").is_none());
    assert_eq!(query["operations"].as_array().unwrap().len(), 23);
    let actual = serde_json::to_value(
        viboceros_oracle::run_request(&serde_json::from_value(query).unwrap()).unwrap(),
    )
    .unwrap();
    if let Ok(path) = std::env::var("VIBOCEROS_TWO_VIEW_DIAGNOSTIC_PATH") {
        std::fs::write(path, serde_json::to_vec(&actual).unwrap()).unwrap();
    }
    let native = observation::read(&base, "curve_two_views");
    let native = native["results"].as_array().unwrap();
    let mut qualified = 0;
    let mut count_gaps = 0;
    let mut completed = 0;
    for a in actual["results"].as_array().unwrap() {
        let id = a["id"].as_str().unwrap();
        let b = native.iter().find(|b| b["id"] == a["id"]).unwrap();
        let mut target = b["value"].clone();
        for phase in ["before", "after", "undo", "redo"] {
            for o in target[phase]["objects"].as_array_mut().unwrap() {
                o["kind"] = Value::String("curve".into());
            }
        }
        let source_check =
            comparison::compare(&sources_only(&a["value"]), &sources_only(&target)).unwrap();
        assert!(source_check.model_error <= 1e-6);
        let outputs = |v: &Value| {
            v["after"]["objects"]
                .as_array()
                .unwrap()
                .iter()
                .filter(|o| o["source"].is_null())
                .cloned()
                .collect::<Vec<_>>()
        };
        let local = outputs(&a["value"]);
        let expected = outputs(&target);
        if a["value"]["success"] == true {
            completed += 1;
        }
        for phase in ["after", "redo"] {
            for o in a["value"][phase]["objects"]
                .as_array()
                .unwrap()
                .iter()
                .filter(|o| o["source"].is_null())
            {
                for field in ["attributes", "geometry_user_text", "selected", "closed"] {
                    assert_eq!(o[field], expected[0][field], "{id}: {field}");
                }
            }
        }
        let count_gap = local.len() != expected.len();
        let upper = if count_gap {
            assert_eq!(id, "two_views_circles_equal");
            assert_eq!((local.len(), expected.len()), (2, 1));
            count_gaps += 1;
            None
        } else {
            let result = comparison::compare_with_curve_distance(
                &a["value"],
                &target,
                &queries::directed_upper_bound,
            )
            .unwrap();
            if result.model_error <= 1e-6 {
                qualified += 1;
            } else {
                assert!(
                    result.model_error <= ceiling(id).unwrap(),
                    "{id}: {}",
                    result.model_error
                );
            }
            Some(result.model_error)
        };
        println!(
            "TWO_VIEWS_CASE {}",
            serde_json::json!({"id":id,"success":a["value"]["success"],"sample_distance_upper_bound":upper,"geometry_qualified":upper.is_some_and(|v|v<=1e-6),"output_count_gap":count_gap,"actual_outputs":local.len(),"native_outputs":expected.len(),"local_error":a["value"]["local_error"]})
        );
    }
    assert_eq!((completed, qualified, count_gaps), (21, 3, 1));
}
