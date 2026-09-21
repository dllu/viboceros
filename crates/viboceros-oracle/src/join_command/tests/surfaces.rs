use super::*;

fn replay(request: &str, observation: &str, count: usize) -> (ProbeResponse, Value) {
    let request: ProbeRequest = serde_json::from_str(request).unwrap();
    let actual = run_request(&request).unwrap();
    let expected: Value = serde_json::from_str(observation).unwrap();
    assert_eq!(actual.results.len(), count);
    assert_eq!(expected["results"].as_array().unwrap().len(), count);
    for (a, b) in actual
        .results
        .iter()
        .zip(expected["results"].as_array().unwrap())
    {
        assert_eq!(a.id, b["id"]);
    }
    (actual, expected)
}

#[test]
fn surface_commands_replay_all_raw_geometry_and_document_fields() {
    let (actual, expected) = replay(
        include_str!("../../../../../tools/rhino_oracle/fixtures/join_surfaces.json"),
        include_str!("../../../../../tools/rhino_oracle/observations/join_surfaces.json"),
        66,
    );
    for (a, b) in actual
        .results
        .iter()
        .zip(expected["results"].as_array().unwrap())
    {
        compare(&a.value, &b["value"], &a.id);
    }
}

fn outputs(value: &Value) -> Vec<&Value> {
    value["objects"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|o| o["source"].is_null())
        .collect()
}

#[test]
fn partial_duplicate_and_gap_policies_keep_their_raw_rhino_differences() {
    let (actual, expected) = replay(
        include_str!("../../../../../tools/rhino_oracle/fixtures/join_surface_differences.json"),
        include_str!(
            "../../../../../tools/rhino_oracle/observations/join_surface_differences.json"
        ),
        26,
    );
    for (a, b) in actual
        .results
        .iter()
        .zip(expected["results"].as_array().unwrap())
    {
        let b = &b["value"];
        let (native, rhino) = (outputs(&a.value), outputs(b));
        // Retained JoinCopy originals remain exactly the shared input, even
        // when output topology, geometry, or acceptance differs.
        let originals = |v: &Value| {
            v["objects"]
                .as_array()
                .unwrap()
                .iter()
                .filter(|o| !o["source"].is_null())
                .cloned()
                .collect::<Vec<_>>()
        };
        if a.id.contains("JoinCopy") {
            let (original_a, original_b) = (originals(&a.value), originals(b));
            if a.id == "gap-0.0021-JoinCopy-post" {
                assert_eq!((original_a.len(), original_b.len()), (2, 2));
                for (i, (n, r)) in original_a.iter().zip(&original_b).enumerate() {
                    for field in ["source", "name", "layer", "color", "groups", "brep"] {
                        compare(&n[field], &r[field], &a.id);
                    }
                    assert_eq!(n["selected"], json!(i == 0));
                    assert_eq!(r["selected"], true);
                }
            } else {
                compare(&json!(original_a), &json!(original_b), &a.id);
            }
        }
        if a.id.starts_with("partial-edge-") {
            assert_eq!((native.len(), rhino.len()), (1, 1));
            let (n, r) = (&native[0]["brep"], &rhino[0]["brep"]);
            for field in [
                "area",
                "volume",
                "surfaces",
                "faces",
                "face_reversed",
                "edge_tolerances",
                "vertex_tolerances",
            ] {
                compare(&n[field], &r[field], &format!("{}/{field}", a.id));
            }
            // Same boundary locus, but different split-vertex order and
            // retained spatial/UV parameter domains. Do not normalize these.
            assert_eq!(n["edges"][5]["curve"]["domain"], json!([0.5, 1.5]));
            assert_eq!(r["edges"][5]["curve"]["domain"], json!([0., 1.]));
            assert_eq!(n["vertices"][4], r["vertices"][5]);
            assert_eq!(n["vertices"][5], r["vertices"][4]);
            assert_ne!(n["trim_curves"], r["trim_curves"]);
        } else if a.id.starts_with("triple-boundary-") {
            assert_eq!((native.len(), rhino.len()), (1, 2));
            assert_eq!(native[0]["brep"]["surfaces"].as_array().unwrap().len(), 3);
            assert_eq!(rhino[0]["brep"]["surfaces"].as_array().unwrap().len(), 1);
            assert_eq!(rhino[1]["brep"]["surfaces"].as_array().unwrap().len(), 2);
            assert_eq!(rhino[1]["brep"]["volume"], json!(0.));
        } else {
            assert!(a.id.starts_with("gap-"));
            if a.id.starts_with("gap-0.0021-") {
                assert!(native.is_empty());
                assert_eq!(rhino.len(), 2);
                assert_eq!(a.value["succeeded"], false);
                assert_eq!(b["succeeded"], true);
            } else if a.id.starts_with("gap-0.002-") && a.id.ends_with("-pre") {
                assert_eq!((native.len(), rhino.len()), (1, 2));
            } else {
                assert_eq!((native.len(), rhino.len()), (1, 1));
                let (n, r) = (&native[0]["brep"], &rhino[0]["brep"]);
                for field in ["surfaces", "trim_curves"] {
                    compare(&n[field], &r[field], &a.id);
                }
                assert_ne!(n["vertices"], r["vertices"]);
                assert_ne!(n["edges"], r["edges"]);
                assert_ne!(n["edge_tolerances"], r["edge_tolerances"]);
            }
        }
    }
}
