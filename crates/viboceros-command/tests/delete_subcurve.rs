//! Actual licensed commands replayed from independently captured native sources.
use serde_json::Value;
use viboceros_command::{CommandRegistry, open_3dm_with_views};
use viboceros_document::{ObjectId, SelectionMode};
#[path = "support/delete_subcurve.rs"]
mod comparison;
#[allow(dead_code)]
#[path = "support/simplify_curves.rs"]
mod native_curve_comparison;

#[test]
fn retained_portions_metadata_identity_selection_and_history_match_native() {
    let data: Value = serde_json::from_str(include_str!(
        "../../../tools/rhino_oracle/observations/delete_subcurve.json"
    ))
    .unwrap();
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../tools/rhino_oracle/fixtures/delete_subcurve");
    let mut failures = Vec::new();
    for row in data["results"].as_array().unwrap() {
        let name = row["id"].as_str().unwrap();
        let v = &row["value"];
        // Incomplete numeric invocations are exercised by the UI replay.
        if v["recipe"]["confirmation"] == false {
            assert_eq!(v["changed"], false);
            continue;
        }
        let outcome = std::panic::catch_unwind(|| {
            let mut doc = open_3dm_with_views(root.join(format!("{name}.3dm")).to_str().unwrap())
                .unwrap()
                .0;
            if let Some(abs) = v["document_tolerance"]["absolute"].as_f64() {
                assert_eq!(doc.tolerance().absolute(), abs);
            }
            let id = v["source_id"]
                .as_str()
                .unwrap()
                .parse::<ObjectId>()
                .unwrap();
            doc.select_objects_direct([id], SelectionMode::Replace)
                .unwrap();
            doc.clear_history().unwrap();
            let original = doc.object(id).unwrap().clone();
            let registry = CommandRegistry::with_builtins();
            let command = if let Some(length) = v["length"].as_f64() {
                format!(
                    "DeleteSubCrv Numeric={},{length},{}",
                    v["start_parameter"].as_f64().unwrap(),
                    v["end_parameter"].as_f64().unwrap()
                )
            } else {
                let point = |key: &str| {
                    serde_json::from_value::<[f64; 3]>(v[key].clone())
                        .unwrap()
                        .into_iter()
                        .map(|x| x.to_string())
                        .collect::<Vec<_>>()
                        .join(",")
                };
                format!(
                    "DeleteSubCrv {} {}",
                    point("start_point"),
                    point("end_point")
                )
            };
            registry
                .execute(&mut doc, &command)
                .unwrap_or_else(|e| panic!("{name}: {e}"));
            let mut mapping = std::collections::BTreeMap::from([(id.to_string(), id)]);
            let mut maximum = comparison::check(&doc, &v["after"], &original, name, &mut mapping);
            let after = doc.objects().cloned().collect::<Vec<_>>();
            if v["changed"] == true {
                assert!(
                    doc.object(id).is_none(),
                    "{name}: source identity must be replaced"
                );
                assert_eq!(doc.undo_label(), Some("DeleteSubCrv"));
                registry.execute(&mut doc, "Undo").unwrap();
                assert_eq!(
                    doc.objects().cloned().collect::<Vec<_>>(),
                    vec![original.clone()]
                );
                maximum = maximum.max(comparison::check(
                    &doc,
                    &v["undo"],
                    &original,
                    name,
                    &mut mapping,
                ));
                assert!(!doc.can_undo());
                registry.execute(&mut doc, "Redo").unwrap();
                assert_eq!(doc.objects().cloned().collect::<Vec<_>>(), after);
                maximum = maximum.max(comparison::check(
                    &doc,
                    &v["redo"],
                    &original,
                    name,
                    &mut mapping,
                ));
            } else {
                assert!(!doc.can_undo(), "{name}: no-op");
            }
            assert!(maximum <= 1e-6, "{name}: error {maximum}");
            eprintln!(
                "DELETE_SUBCURVE_AUDIT {}",
                serde_json::json!({"id":name,"maximum_error":maximum})
            );
        });
        if outcome.is_err() {
            failures.push(name.to_owned());
        }
    }
    assert!(failures.is_empty(), "native mismatches: {failures:?}");
}
