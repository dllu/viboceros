use super::*;

fn bounds(points: impl Iterator<Item = [f64; 3]>) -> [[f64; 2]; 3] {
    let mut result = [[f64::INFINITY, f64::NEG_INFINITY]; 3];
    for p in points {
        for i in 0..3 {
            result[i][0] = result[i][0].min(p[i]);
            result[i][1] = result[i][1].max(p[i]);
        }
    }
    result
}

fn aligned(doc: &Document, actual: &Value, native: &Value) -> Value {
    let mut used = BTreeSet::new();
    let n = native.as_array().unwrap();
    let mut result = Vec::new();
    for (object, row) in doc.objects().zip(actual.as_array().unwrap()) {
        let Geometry::Brep(brep) = object.geometry() else {
            panic!()
        };
        let own = bounds(brep.vertices().iter().map(|v| v.point().to_array()));
        let candidates = n
            .iter()
            .enumerate()
            .filter(|(i, other)| {
                if used.contains(i)
                    || row["source"] != other["source"]
                    || row["name"] != other["name"]
                {
                    return false;
                }
                if !row["source"].is_null() {
                    return true;
                }
                let theirs = bounds(
                    other["vertices"]
                        .as_array()
                        .unwrap()
                        .iter()
                        .map(|p| serde_json::from_value(p.clone()).unwrap()),
                );
                (row["area"].as_f64().unwrap() - other["area"].as_f64().unwrap()).abs() < 1e-9
                    && (0..3).all(|i| (0..2).all(|j| (own[i][j] - theirs[i][j]).abs() < 1e-9))
            })
            .map(|(i, _)| i)
            .collect::<Vec<_>>();
        assert!(!candidates.is_empty(), "native output for {row}");
        let candidates = candidates
            .into_iter()
            .filter(|&i| row["faces"] == n[i]["faces"] && row["edges"] == n[i]["edges"])
            .collect::<Vec<_>>();
        assert!(!candidates.is_empty(), "native topology for {row}");
        used.insert(candidates[0]);
        result.push(n[candidates[0]].clone());
    }
    assert_eq!(used.len(), n.len());
    Value::Array(result)
}

#[test]
fn boolean_split_open_replays_shared_boundary_geometry_metadata_and_history() {
    let q: Value = serde_json::from_str(include_str!(
        "../../../../../tools/rhino_oracle/observations/boolean_split_open.json"
    ))
    .unwrap();
    replay(&q, 11);
}

#[test]
fn boolean_split_mixed_open_replays_coplanar_stage_boundary_ownership_and_history() {
    let q: Value = serde_json::from_str(include_str!(
        "../../../../../tools/rhino_oracle/observations/boolean_split_mixed_open.json"
    ))
    .unwrap();
    replay(&q, 20);
}

#[test]
fn boolean_split_topology_replays_compound_and_trimmed_inputs() {
    let q: Value = serde_json::from_str(include_str!(
        "../../../../../tools/rhino_oracle/observations/boolean_split_topology.json"
    ))
    .unwrap();
    replay(&q, 12);
}

fn replay(q: &Value, expected_success: usize) {
    let mut success = 0;
    for r in q["results"].as_array().unwrap() {
        let v = &r["value"];
        let case = v["case"].as_str().unwrap();
        let (mut doc, ids, layers, groups) = plane::setup(v);
        let command = format!(
            "BooleanSplit FirstSet={} SecondSet={} DeleteInput={}",
            ids[0],
            ids[1..]
                .iter()
                .map(ToString::to_string)
                .collect::<Vec<_>>()
                .join(","),
            if v["delete"] == true { "Yes" } else { "No" }
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
        let a = snapshot(&doc, &ids, &layers, &groups);
        let expected = aligned(&doc, &a, &v["command"]["after_script"]);
        compare(&a, &expected, case);
        if result.is_ok() {
            success += 1;
            for (object, expected) in doc.objects().zip(expected.as_array().unwrap()) {
                if !expected["source"].is_null() {
                    continue;
                }
                let Geometry::Brep(b) = object.geometry() else {
                    panic!()
                };
                let a = regions(b);
                let b =
                    serde_json::from_value::<Boundary>(expected["face_regions"].clone()).unwrap();
                witnesses(&a, &b, case);
                witnesses(&b, &a, case);
            }
            registry.execute(&mut doc, "Undo").unwrap();
            compare(
                &snapshot(&doc, &ids, &layers, &groups),
                &v["undo"]["after_script"],
                &format!("{case}/undo"),
            );
            registry.execute(&mut doc, "Redo").unwrap();
            let a = snapshot(&doc, &ids, &layers, &groups);
            compare(
                &a,
                &aligned(&doc, &a, &v["redo"]["after_script"]),
                &format!("{case}/redo"),
            );
        } else {
            assert!(!doc.can_undo());
        }
    }
    assert_eq!(success, expected_success);
}
