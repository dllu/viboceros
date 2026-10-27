//! Frozen initial-preparation admission before adaptive command integration.
use serde_json::Value;

pub fn initially_admitted(family: &str, id: &Value) -> bool {
    use std::{collections::BTreeMap, sync::OnceLock};
    static CASES: OnceLock<BTreeMap<String, BTreeMap<String, bool>>> = OnceLock::new();
    let cases = CASES.get_or_init(|| {
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../tools/rhino_oracle/fixtures/loft_refit_preparation_baseline.json");
        let saved: Value = serde_json::from_slice(&std::fs::read(path).unwrap()).unwrap();
        let mut cases = BTreeMap::<String, BTreeMap<String, bool>>::new();
        for row in saved["cases"].as_array().unwrap() {
            assert!(
                cases
                    .entry(row["family"].as_str().unwrap().to_owned())
                    .or_default()
                    .insert(
                        row["id"].as_str().unwrap().to_owned(),
                        row["initial_preparation"].as_bool().unwrap()
                    )
                    .is_none()
            );
        }
        cases
    });
    *cases
        .get(family)
        .and_then(|group| group.get(id.as_str().unwrap()))
        .unwrap_or_else(|| panic!("missing legacy preparation case {family} {id}"))
}
