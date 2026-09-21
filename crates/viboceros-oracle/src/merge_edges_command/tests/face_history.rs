use super::*;

#[test]
fn singular_seam_and_multidirectional_replacement_preserves_full_history() {
    let request: ProbeRequest = serde_json::from_str(include_str!(
        "../../../../../tools/rhino_oracle/fixtures/merge_edges_face_history.json"
    ))
    .unwrap();
    let observed: Value = serde_json::from_str(include_str!(
        "../../../../../tools/rhino_oracle/observations/merge_edges_face_history.json"
    ))
    .unwrap();
    let actual = run_request(&request).unwrap();
    assert_eq!(actual.results.len(), 24);
    assert_eq!(observed["results"].as_array().unwrap().len(), 24);
    let mut relabeled = 0;
    for (a, b) in actual
        .results
        .iter()
        .zip(observed["results"].as_array().unwrap())
    {
        assert_eq!(a.id, b["id"]);
        assert_eq!(a.value["succeeded"], true);
        assert_eq!(a.value["history_tested"], true);
        let mut value = a.value.clone();
        if a.id.starts_with("two_directions-") {
            relabeled += 1;
            // These two raw records differ in component numbering, not loci.
            // Require a bijection and remap *every* vertex/edge reference and
            // uncertainty before comparing all remaining fields unchanged.
            // The published raw report still counts them as differences.
            for state in ["after", "redo"] {
                let actual = &mut value[state][0]["geometry"]["brep"];
                let expected = &b["value"][state][0]["geometry"]["brep"];
                relabel(actual, expected);
            }
        }
        close(&value, &b["value"], &a.id);
        close(&a.value["after"], &a.value["redo"], &a.id);
        for state in ["after", "undo", "redo"] {
            let selected = a.value[state]
                .as_array()
                .unwrap()
                .iter()
                .filter(|o| o["selected"] == true)
                .count();
            assert_eq!(
                selected,
                if a.id.ends_with("pre1") {
                    if a.id.starts_with("mixed-") { 3 } else { 1 }
                } else {
                    0
                }
            );
        }
    }
    assert_eq!(relabeled, 2);
}

fn relabel(actual: &mut Value, expected: &Value) {
    let vertices = actual["vertices"].as_array().unwrap();
    let target = expected["vertices"].as_array().unwrap();
    assert_eq!(vertices.len(), target.len());
    let vertex_map = vertices
        .iter()
        .map(|p| {
            let candidates = target
                .iter()
                .enumerate()
                .filter(|(_, q)| {
                    p.as_array()
                        .unwrap()
                        .iter()
                        .zip(q.as_array().unwrap())
                        .all(|(a, b)| (a.as_f64().unwrap() - b.as_f64().unwrap()).abs() < 1e-12)
                })
                .map(|(i, _)| i)
                .collect::<Vec<_>>();
            assert_eq!(candidates.len(), 1);
            candidates[0]
        })
        .collect::<Vec<_>>();
    assert_eq!(
        vertex_map.iter().collect::<BTreeSet<_>>().len(),
        vertices.len()
    );
    let edges = actual["edges"].as_array().unwrap();
    let target = expected["edges"].as_array().unwrap();
    assert_eq!(edges.len(), target.len());
    let edge_map = edges
        .iter()
        .map(|edge| {
            let pair = edge["vertices"]
                .as_array()
                .unwrap()
                .iter()
                .map(|v| vertex_map[v.as_u64().unwrap() as usize])
                .collect::<Vec<_>>();
            let candidates = target
                .iter()
                .enumerate()
                .filter(|(_, e)| e["vertices"] == json!(pair))
                .map(|(i, _)| i)
                .collect::<Vec<_>>();
            assert_eq!(candidates.len(), 1);
            candidates[0]
        })
        .collect::<Vec<_>>();
    assert_eq!(edge_map.iter().collect::<BTreeSet<_>>().len(), edges.len());
    for edge in actual["edges"].as_array_mut().unwrap() {
        for v in edge["vertices"].as_array_mut().unwrap() {
            *v = json!(vertex_map[v.as_u64().unwrap() as usize]);
        }
    }
    for (key, map) in [
        ("vertices", &vertex_map),
        ("vertex_tolerances", &vertex_map),
        ("edges", &edge_map),
        ("edge_tolerances", &edge_map),
    ] {
        let old = std::mem::take(actual[key].as_array_mut().unwrap());
        let mut reordered = vec![Value::Null; old.len()];
        for (v, &target) in old.into_iter().zip(map) {
            reordered[target] = v;
        }
        actual[key] = json!(reordered);
    }
    type LoopKey = (String, Vec<(Option<usize>, bool)>);
    let mut faces: Vec<(Vec<LoopKey>, f64)> =
        serde_json::from_value(actual["faces"].clone()).unwrap();
    for (rings, _) in &mut faces {
        for (_, uses) in rings.iter_mut() {
            for (edge, _) in uses.iter_mut() {
                *edge = edge.map(|e| edge_map[e]);
            }
            uses.sort();
        }
        rings.sort();
    }
    faces.sort_by(|a, b| a.0.cmp(&b.0));
    actual["faces"] = json!(faces);
}
