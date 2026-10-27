//! Actual SDK and document admission, retaining strict legacy comparisons.
use serde_json::{Value, json};
use viboceros_oracle::{ProbeRequest, run_request};
#[allow(dead_code)]
#[path = "../../../viboceros-geometry/tests/support/loft_rebuild.rs"]
pub(super) mod comparison;
#[path = "loft_refit_surface_bound.rs"]
mod surface_bound;

pub fn read(family: &str, folder: &str) -> Value {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tools/rhino_oracle");
    serde_json::from_slice(&std::fs::read(root.join(format!("{folder}/{family}.json"))).unwrap())
        .unwrap()
}
#[path = "loft_refit_legacy.rs"]
mod legacy_cases;
pub use legacy_cases::initially_admitted as legacy;

fn surfaces(value: &Value) -> &Value {
    if value.is_array() {
        value
    } else if value.get("surfaces").is_some() {
        &value["surfaces"]
    } else {
        &value["outputs"][0]["surfaces"]
    }
}
fn metadata(value: &Value) -> Value {
    if value.is_array() {
        return json!({});
    }
    let mut result = value.clone();
    for key in [
        "surfaces",
        "refitted",
        "refit_adjustment",
        "refit_sources",
        "refit_probe_angle_tolerance",
        "refit_frames",
        "refit_clock",
        "refit_sets",
        "refit_beziers",
        "scalar_clock",
        "input_planes",
        "simplified_inputs",
    ] {
        result.as_object_mut().unwrap().remove(key);
    }
    if let Some(outputs) = result.get_mut("outputs").and_then(Value::as_array_mut) {
        for output in outputs {
            output.as_object_mut().unwrap().remove("surfaces");
        }
    }
    result
}
#[derive(Debug, Default)]
pub struct Stats {
    pub strict: usize,
    pub geometric: usize,
    pub declined: usize,
    pub legacy: usize,
    pub maximum_bound: f64,
}

fn expected_decline(family: &str, id: &Value) -> bool {
    use std::{collections::BTreeSet, sync::OnceLock};
    static MISSING: OnceLock<BTreeSet<(String, String)>> = OnceLock::new();
    let missing = MISSING.get_or_init(|| {
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../tools/rhino_oracle/fixtures/loft_refit_preparation_declines.json");
        let saved: Value = serde_json::from_slice(&std::fs::read(path).unwrap()).unwrap();
        saved["cases"]
            .as_array()
            .unwrap()
            .iter()
            .map(|r| {
                (
                    r["family"].as_str().unwrap().to_owned(),
                    r["id"].as_str().unwrap().to_owned(),
                )
            })
            .collect()
    });
    missing.contains(&(family.to_owned(), id.as_str().unwrap().to_owned()))
}

pub fn compare(family: &str, range: std::ops::Range<usize>) -> Stats {
    let query = read(family, "fixtures");
    let native = read(family, "observations");
    let operations = query["operations"].as_array().unwrap();
    let rows = native["results"].as_array().unwrap();
    assert_eq!(operations.len(), rows.len());
    assert!(range.end <= rows.len());
    let mut stats = Stats::default();
    for (op, row) in operations
        .iter()
        .zip(rows)
        .skip(range.start)
        .take(range.len())
    {
        assert_eq!(op["id"], row["id"]);
        let mut request = query.clone();
        request["operations"] = json!([op]);
        if op["op"] != "loft" {
            assert_eq!(op["op"], "nurbs_curve_closest_point");
            let actual =
                run_request(&serde_json::from_value::<ProbeRequest>(request).unwrap()).unwrap();
            let difference =
                comparison::difference(&actual.results[0].value, &row["value"]).unwrap();
            assert!(difference <= 1e-9, "{}: {difference}", op["id"]);
            stats.strict += 1;
            stats.legacy += 1;
            continue;
        }
        let prior = legacy(family, &op["id"]);
        match run_request(&serde_json::from_value::<ProbeRequest>(request).unwrap()) {
            Ok(actual) => {
                let value = &actual.results[0].value;
                if let Some(folder) = std::env::var_os("VIBOCEROS_REFIT_RESULTS") {
                    let folder = std::path::PathBuf::from(folder);
                    std::fs::create_dir_all(&folder).unwrap();
                    std::fs::write(
                        folder.join(format!("{}.json", op["id"].as_str().unwrap())),
                        serde_json::to_vec(
                            &json!({"operation":op,"actual":value,"native":row["value"]}),
                        )
                        .unwrap(),
                    )
                    .unwrap();
                }
                let expected = &row["value"];
                if value.get("before").is_some() {
                    assert_eq!(
                        value["before"], value["after"],
                        "{}: source mutation",
                        op["id"]
                    );
                }
                let metadata_error = comparison::difference(&metadata(value), &metadata(expected))
                    .unwrap_or_else(|e| panic!("{}: metadata {e}", op["id"]));
                assert!(
                    metadata_error <= 1e-9,
                    "{}: metadata difference {metadata_error}",
                    op["id"]
                );
                let difference = comparison::difference(surfaces(value), surfaces(expected));
                let epsilon = if (family == "loft_refit"
                    && op["refit_tolerance"].as_f64().unwrap() > 0.
                    && !op["id"].as_str().unwrap().contains("-polygon-")
                    && !op["id"].as_str().unwrap().contains("-line-"))
                    || family.contains("ellipses")
                    || family.contains("adjustment_local")
                    || family.contains("adjustment_commands")
                    || family == "loft_refit_subdivision"
                {
                    1e-8
                } else {
                    1e-9
                };
                if prior {
                    let d = difference
                        .as_ref()
                        .unwrap_or_else(|e| panic!("{}: legacy {e}", op["id"]));
                    assert!(*d <= epsilon, "{}: legacy {d}", op["id"]);
                    stats.legacy += 1;
                }
                let strict = difference.as_ref().is_ok_and(|d| *d <= 1e-9);
                if strict {
                    stats.strict += 1;
                } else {
                    stats.geometric += 1;
                }
                if !prior || !strict {
                    let tolerance = op["refit_tolerance"].as_f64().unwrap();
                    let bound =
                        surface_bound::bound(surfaces(value), surfaces(expected), tolerance)
                            .unwrap_or_else(|e| {
                                panic!("{}: {difference:?}; Native geometric bound: {e}", op["id"])
                            });
                    stats.maximum_bound = stats.maximum_bound.max(bound);
                    if !strict {
                        println!(
                            "ADAPTIVE_COMMAND_DIFFERENCE {} strict={difference:?} continuous_native_bound={bound} tolerance={tolerance}",
                            op["id"]
                        );
                    }
                }
            }
            Err(error) => {
                assert!(
                    expected_decline(family, &op["id"]),
                    "{}: regressed admitted record: {error}",
                    op["id"]
                );
                assert!(
                    !prior || surfaces(&row["value"]).as_array().unwrap().is_empty(),
                    "{}: lost legacy preparation: {error}",
                    op["id"]
                );
                stats.declined += 1;
                println!("ADAPTIVE_COMMAND_DECLINED {} {error}", op["id"]);
            }
        }
    }
    assert_eq!(stats.strict + stats.geometric + stats.declined, range.len());
    println!("ADAPTIVE_COMMAND {family} {range:?} {stats:?}");
    stats
}
