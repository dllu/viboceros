use super::*;
use crate::boolean_union::tests::{Boundary, compare, regions, setup, snapshot, witnesses};
use serde_json::Value;

pub(crate) fn align(actual: &Value, native: &Value) -> (Value, bool) {
    let mut used = BTreeSet::new();
    let n = native.as_array().unwrap();
    let mut ordered = true;
    let rows = actual
        .as_array()
        .unwrap()
        .iter()
        .enumerate()
        .map(|(i, row)| {
            let candidates = n
                .iter()
                .enumerate()
                .filter(|(j, o)| {
                    !used.contains(j)
                        && row["source"] == o["source"]
                        && row["attribute_text"] == o["attribute_text"]
                        && (0..3).all(|k| {
                            (row["centroid"][k].as_f64().unwrap()
                                - o["centroid"][k].as_f64().unwrap())
                            .abs()
                                < 1e-10
                        })
                })
                .map(|(j, _)| j)
                .collect::<Vec<_>>();
            assert_eq!(candidates.len(), 1, "unique native piece for {row}");
            let j = candidates[0];
            used.insert(j);
            ordered &= i == j;
            n[j].clone()
        })
        .collect::<Vec<_>>();
    assert_eq!(used.len(), n.len());
    (Value::Array(rows), ordered)
}

#[test]
fn boolean_split_replays_native_geometry_metadata_selection_and_history() {
    let q: Value = serde_json::from_str(include_str!(
        "../../../../tools/rhino_oracle/observations/boolean_split_command.json"
    ))
    .unwrap();
    let mut order_differences = Vec::new();
    let mut count = 0;
    for r in q["results"].as_array().unwrap() {
        let v = &r["value"];
        let case = v["case"].as_str().unwrap();
        if case.contains("cancel") {
            continue;
        }
        count += 1;
        let (mut doc, ids, layers, groups) = setup(&v["before"]);
        doc.clear_history().unwrap();
        let indices = |field: &str| {
            v[field]
                .as_array()
                .unwrap()
                .iter()
                .map(|x| x.as_u64().unwrap() as usize)
                .collect::<Vec<_>>()
        };
        let first = indices("first");
        let second = indices("second");
        doc.select_objects_direct(
            first.iter().chain(&second).map(|&i| ids[i]),
            SelectionMode::Replace,
        )
        .unwrap();
        let set = |indices: &[usize]| {
            indices
                .iter()
                .map(|&i| ids[i].to_string())
                .collect::<Vec<_>>()
                .join(",")
        };
        let command = format!(
            "BooleanSplit DeleteInput={} FirstSet={} SecondSet={}",
            if v["delete"] == true { "Yes" } else { "No" },
            set(&first),
            set(&second)
        );
        let registry = CommandRegistry::with_builtins();
        let result = if v["pre"] == true {
            registry.execute(&mut doc, &command)
        } else {
            registry.execute_postselected(&mut doc, &command, Default::default())
        };
        assert_eq!(
            result.is_ok(),
            v["command"]["success"] == true,
            "{case}: {result:?}"
        );
        let actual = snapshot(&doc, &ids, &layers, &groups);
        let (expected, ordered) = align(&actual, &v["command"]["after_script"]);
        if !ordered {
            order_differences.push(case);
        }
        compare(&actual, &expected, case);
        for (object, expected) in doc.objects().zip(expected.as_array().unwrap()) {
            if !expected["source"].is_null() {
                continue;
            }
            let Geometry::Brep(brep) = object.geometry() else {
                panic!("split")
            };
            let a = regions(brep);
            let b = serde_json::from_value::<Boundary>(expected["face_regions"].clone()).unwrap();
            witnesses(&a, &b, case);
            witnesses(&b, &a, case);
        }
        if result.is_ok() {
            registry.execute(&mut doc, "Undo").unwrap();
            let a = snapshot(&doc, &ids, &layers, &groups);
            compare(&a, &v["undo"]["after"], &format!("{case}/undo"));
            registry.execute(&mut doc, "Redo").unwrap();
            let a = snapshot(&doc, &ids, &layers, &groups);
            compare(
                &a,
                &align(&a, &v["redo"]["after"]).0,
                &format!("{case}/redo"),
            );
        } else {
            assert!(!doc.can_undo(), "{case}");
        }
    }
    assert_eq!(count, 25);
    println!("BooleanSplit output ordering differs: {order_differences:?}");
}

#[test]
fn boolean_split_rejects_invalid_sets_and_unsupported_geometry_atomically() {
    let registry = CommandRegistry::with_builtins();
    let mut doc = Document::default();
    registry.execute(&mut doc, "Box 0,0 2,2 2").unwrap();
    let id = doc.objects().next().unwrap().id();
    doc.clear_history().unwrap();
    let before = doc.objects().cloned().collect::<Vec<_>>();
    for arguments in [
        "DeleteInput=Maybe".to_string(),
        format!("SecondSet={id}"),
        format!("FirstSet={id},{id} SecondSet={id}"),
        format!("FirstSet={id} SecondSet={id} DeleteInput=Yes DeleteInput=No"),
    ] {
        assert!(
            registry
                .execute(&mut doc, &format!("BooleanSplit {arguments}"))
                .is_err()
        );
        assert_eq!(doc.objects().cloned().collect::<Vec<_>>(), before);
        assert!(!doc.can_undo());
    }
    registry.execute(&mut doc, "Sphere 1,1 1").unwrap();
    let sphere = doc.objects().last().unwrap().id();
    doc.clear_history().unwrap();
    let before = doc.objects().cloned().collect::<Vec<_>>();
    assert!(
        registry
            .execute(
                &mut doc,
                &format!("BooleanSplit FirstSet={id} SecondSet={sphere}")
            )
            .is_err()
    );
    assert_eq!(doc.objects().cloned().collect::<Vec<_>>(), before);
    assert!(!doc.can_undo());
}
