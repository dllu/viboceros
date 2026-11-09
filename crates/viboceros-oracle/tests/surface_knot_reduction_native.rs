//! Whole-source tensor bounds and owned SDK comparison, including declined edits.
use serde_json::{Value, json};
use viboceros_geometry::*;
use viboceros_oracle::{ProbeRequest, run_request};
#[path = "../../viboceros-command/tests/support/merge_surfaces.rs"]
#[allow(dead_code)]
mod captured;

fn data() -> Vec<(Value, Value)> {
    let root =
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tools/rhino_oracle/fixtures");
    ["surface_knot_reduction", "surface_knot_reduction_holdouts"]
        .into_iter()
        .map(|family| {
            (
                serde_json::from_slice(
                    &std::fs::read(root.join(format!("{family}.json"))).unwrap(),
                )
                .unwrap(),
                captured::read(family),
            )
        })
        .collect()
}

#[test]
fn surface_reductions_match_or_explicitly_decline_under_a_complete_original_bound() {
    let mut matched = 0;
    let mut declined = 0;
    let mut count = 0;
    for (query, native) in data() {
        let operations = query["operations"].as_array().unwrap();
        assert_eq!(
            operations.len(),
            native["results"].as_array().unwrap().len()
        );
        for (op, row) in operations.iter().zip(native["results"].as_array().unwrap()) {
            assert_eq!(op["id"], row["id"]);
            let request: ProbeRequest =
                serde_json::from_value(json!({"protocol_version":1,"iterations":1,
                "tolerance":query["tolerance"],"operations":[op]}))
                .unwrap();
            let result = run_request(&request).unwrap_or_else(|e| panic!("{}: {e}", op["id"]));
            let actual = &result.results[0].value;
            let expected = &row["value"];
            assert_eq!(actual["before"], actual["after"]);
            assert_eq!(expected["before"], expected["after"]);
            let source = captured::surface(&actual["before"]);
            let surface = captured::surface(&actual["surface"]);
            let target = captured::surface(&expected["surface"]);
            let limit = op["absolute_tolerance"].as_f64().unwrap();
            let source_bound = if limit > 0. {
                Some(
                    source
                        .parameter_surface_deviation_bound(&surface, limit)
                        .unwrap()
                        .unwrap_or_else(|| panic!("{}: original tensor bound", op["id"])),
                )
            } else {
                None
            };
            let agrees = actual["removed"] == expected["removed"];
            let bound = if agrees {
                assert_eq!(actual["surface"]["degree"], expected["surface"]["degree"]);
                assert_eq!(
                    actual["surface"]["control_count"],
                    expected["surface"]["control_count"]
                );
                for key in ["knots_u", "knots_v"] {
                    let a = actual["surface"][key].as_array().unwrap();
                    let b = expected["surface"][key].as_array().unwrap();
                    assert_eq!(a.len(), b.len());
                    for (a, b) in a.iter().zip(b) {
                        assert!((a.as_f64().unwrap() - b.as_f64().unwrap()).abs() <= 1e-12);
                    }
                }
                matched += 1;
                Some(
                    surface
                        .parameter_surface_deviation_bound(&target, 1e-9)
                        .unwrap()
                        .unwrap_or_else(|| panic!("{}: native tensor comparison", op["id"])),
                )
            } else {
                assert!(limit > 0.);
                assert!(
                    actual["removed"].as_u64().unwrap() < expected["removed"].as_u64().unwrap()
                );
                // This is a stricter parameter correspondence. An inconclusive
                // bound does not prove geometric disagreement with the SDK.
                assert!(
                    source
                        .parameter_surface_deviation_bound(&target, limit)
                        .unwrap()
                        .is_none()
                );
                declined += 1;
                None
            };
            let brep = Brep::try_rectangular_surface_face_with_surface_edge_domains(
                surface.clone(),
                surface.domain_u(),
                surface.domain_v(),
                false,
                captured::model_tolerance(),
            )
            .unwrap();
            let archive = captured::archive(&brep);
            let archive_bound = surface
                .parameter_surface_deviation_bound(archive.faces()[0].surface(), 1e-9)
                .unwrap()
                .unwrap();
            println!(
                "SURFACE_REDUCTION {}",
                json!({"id":op["id"],"native_match":agrees,"maximum_native_bound":bound,
                "source_bound":source_bound,"maximum_archive_bound":archive_bound,"removed":actual["removed"],"native_removed":expected["removed"]})
            );
            count += 1;
        }
    }
    assert_eq!(count, 88);
    assert_eq!((matched, declined), (64, 24));
    println!(
        "SURFACE_REDUCTION_TOTAL {}",
        json!({"matched":matched,"declined":declined,"cases":count})
    );
}

#[test]
fn defaults_and_invalid_ranges_use_the_same_public_dispatch() {
    let (mut query, _) = data().remove(0);
    let mut operation = query["operations"][0].clone();
    for field in ["minimum_multiplicity", "maximum_multiplicity", "direction"] {
        operation.as_object_mut().unwrap().remove(field);
    }
    query["operations"] = json!([operation]);
    let result =
        run_request(&serde_json::from_value::<ProbeRequest>(query.clone()).unwrap()).unwrap();
    assert_eq!(result.results[0].value["removed"], 6);
    for changes in [
        json!({"absolute_tolerance":-1.}),
        json!({"minimum_multiplicity":4,"maximum_multiplicity":4}),
        json!({"direction":"invalid"}),
    ] {
        let mut bad = query.clone();
        for (key, value) in changes.as_object().unwrap() {
            bad["operations"][0][key] = value.clone();
        }
        let result = serde_json::from_value::<ProbeRequest>(bad)
            .map_err(|_| ())
            .and_then(|request| run_request(&request).map_err(|_| ()));
        assert!(result.is_err());
    }
}
