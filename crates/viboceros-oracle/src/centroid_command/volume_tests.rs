use super::*;

#[test]
fn volume_centroid_commands_match_while_raw_negative_mesh_api_centroids_remain_different() {
    let request: ProbeRequest = serde_json::from_str(include_str!(
        "../../../../tools/rhino_oracle/fixtures/volume_centroid.json"
    ))
    .unwrap();
    let observed: Value = serde_json::from_str(include_str!(
        "../../../../tools/rhino_oracle/observations/volume_centroid.json"
    ))
    .unwrap();
    let actual = run_request(&request).unwrap();
    assert_eq!(actual.results.len(), 26);
    let close = |a: &Value, b: &Value| (a.as_f64().unwrap() - b.as_f64().unwrap()).abs() <= 1e-9;
    let mut api_differences = Vec::new();
    for (a, b) in actual
        .results
        .iter()
        .zip(observed["results"].as_array().unwrap())
    {
        assert_eq!(a.id, b["id"]);
        let (x, y) = (&a.value, &b["value"]);
        assert_eq!(x["succeeded"], y["succeeded"], "{}", a.id);
        assert_eq!(x["selected"], y["selected"], "{}", a.id);
        let (points, expected) = (
            x["points"].as_array().unwrap(),
            y["points"].as_array().unwrap(),
        );
        assert_eq!(points.len(), expected.len(), "{}", a.id);
        for (p, q) in points.iter().zip(expected) {
            for field in ["selected", "current_layer", "groups", "name"] {
                assert_eq!(p[field], q[field]);
            }
            assert!(
                (0..3).all(|i| close(&p["point"][i], &q["point"][i])),
                "{}",
                a.id
            );
        }
        let masses = x["properties"].as_array().unwrap();
        let source = y["source_properties"].as_array().unwrap();
        assert_eq!(masses.len(), source.len());
        if !masses.iter().zip(source).all(|(p, q)| {
            if p.is_null() || q.is_null() {
                return p == q;
            }
            close(&p["volume"], &q["volume"])
                && (0..3).all(|i| close(&p["centroid"][i], &q["centroid"][i]))
        }) {
            api_differences.push(a.id.as_str());
        }
        // The raw API's signed first moments, unlike its negative-mesh
        // Centroid getter, reproduce the mathematical center.
        for (i, m) in masses.iter().enumerate() {
            if m.is_null() {
                continue;
            }
            let volume = source[i]["volume"].as_f64().unwrap();
            for axis in 0..3 {
                let from_moment = y["source_first_moments"][i][axis].as_f64().unwrap() / volume;
                assert!(
                    (from_moment - m["centroid"][axis].as_f64().unwrap()).abs() < 1e-9,
                    "{}",
                    a.id
                );
            }
        }
    }
    assert_eq!(
        api_differences,
        [
            "reversed-tetrahedron",
            "mixed-orientations",
            "equal-opposite-volumes",
            "post-equal-opposite-volumes",
            "opposed-disjoint-shells",
            "brep-paraboloid-capped-outward",
            "brep-paraboloid-capped-inward",
            "brep-paraboloid-rotated-translated",
            "warped-box-1-0",
            "warped-box-1-1",
            "warped-box-1-2",
            "warped-box-1-3"
        ]
    );
}

#[test]
fn volume_centroid_fixture_requires_one_iteration_and_valid_indices() {
    let mut request: ProbeRequest = serde_json::from_str(include_str!(
        "../../../../tools/rhino_oracle/fixtures/volume_centroid.json"
    ))
    .unwrap();
    request.iterations = 2;
    assert!(run_request(&request).is_err());
    request.iterations = 1;
    let Operation::VolumeCentroidCommand { fixture, .. } = &mut request.operations[0] else {
        panic!()
    };
    fixture.selected = Some(vec![0, 0]);
    assert!(run_request(&request).is_err());
}
