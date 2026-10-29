use super::support;
use serde_json::{Value, json};
use viboceros_geometry::{SubD, SubDMergeComponent};

fn definition(rows: &Value) -> &Value {
    &rows
        .as_array()
        .unwrap()
        .iter()
        .find(|r| r.get("subd").is_some())
        .unwrap()["subd"]
}
pub(super) fn records(family: &str) -> (Value, Value) {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tools/rhino_oracle");
    let r = serde_json::from_str(
        &std::fs::read_to_string(root.join(format!("observations/{family}.json"))).unwrap(),
    )
    .unwrap();
    let cases = serde_json::from_str(
        &std::fs::read_to_string(root.join("fixtures/subd_merge_allocation_recipes.json")).unwrap(),
    )
    .unwrap();
    (cases, r)
}
fn compare(s: &SubD, native: &Value, name: &str, differences: &mut Vec<String>) {
    support::differences_between(
        &support::actual_value(s),
        &support::native_value(native),
        name,
        differences,
    );
    assert!(
        s.face_properties()
            .iter()
            .all(|p| p.color.is_none() && p.material_channel == 0)
    );
}
pub(super) fn audit() {
    let mut report = Vec::new();
    let mut failures = Vec::new();
    for family in [
        "subd_merge_allocation",
        "subd_merge_allocation_finish",
        "subd_merge_allocation_live",
        "subd_merge_allocation_keyboard",
    ] {
        let (cases, record) = records(family);
        for row in record["results"].as_array().unwrap() {
            let name = row["id"].as_str().unwrap();
            let v = &row["value"];
            let source = support::read(&v["source_3dm"]);
            let selected = cases[name]["faces"]
                .as_array()
                .unwrap()
                .iter()
                .map(|i| SubDMergeComponent::Face(i.as_u64().unwrap() as usize))
                .collect::<Vec<_>>();
            let merged = source.merge_face_components(&selected).unwrap().subd;
            let mut differences = Vec::new();
            compare(
                &source,
                definition(&v["before"]),
                &format!("{name}/before"),
                &mut differences,
            );
            compare(
                &merged,
                definition(&v["merged"]),
                &format!("{name}/merged"),
                &mut differences,
            );
            let global = merged.subdivided(1).unwrap();
            for key in ["sdk_copy", "sdk_archive"] {
                compare(
                    &global,
                    &v[key]["after"],
                    &format!("{name}/{key}"),
                    &mut differences,
                );
            }
            let after = if let Some(id) = v["local_face"].as_u64() {
                let index = merged
                    .face_ids()
                    .iter()
                    .position(|&x| x as u64 == id)
                    .unwrap();
                merged.subdivided_faces(&[index]).unwrap()
            } else {
                global
            };
            compare(
                &after,
                definition(&v["after"]),
                &format!("{name}/after"),
                &mut differences,
            );
            for (phase, archive) in [("merged", "merged_3dm"), ("after", "after_3dm")] {
                let imported = support::read(&v[archive]);
                compare(
                    &imported,
                    definition(&v[phase]),
                    &format!("{name}/{phase}/native-archive"),
                    &mut differences,
                );
            }
            if !differences.is_empty() {
                failures.push((name.to_owned(), differences.clone()));
            }
            report.push(json!({"family":family,"id":name,"differences":differences,"merged":support::actual_value(&merged),"after":support::actual_value(&after)}));
        }
    }
    assert_eq!(report.len(), 52);
    if let Ok(path) = std::env::var("VIBOCEROS_SUBD_ALLOCATION_AUDIT_OUTPUT") {
        std::fs::write(path, serde_json::to_string_pretty(&report).unwrap()).unwrap();
    }
    assert!(
        failures.is_empty(),
        "Allocating SubD edits differ: {failures:?}"
    );
}
