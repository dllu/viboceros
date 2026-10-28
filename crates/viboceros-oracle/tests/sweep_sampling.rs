use serde_json::json;
use viboceros_oracle::{ProbeRequest, run_request};

#[test]
fn command_faces_sample_their_own_profile_domains() {
    let request: ProbeRequest = serde_json::from_value(json!({
        "protocol_version": 1,
        "iterations": 1,
        "tolerance": {"absolute": 1e-7, "relative": 1e-12, "angular": 1e-10},
        "operations": [{
            "op": "sweep1", "id": "split-profile-domains", "command": true,
            "rail": {"type": "line", "start": [0., 0., 0.], "end": [0., 0., 5.]},
            "sections": [{"type": "polyline", "vertices": [
                [0., 0., 0.], [1., 2., 0.], [3., 1., 0.]
            ]}],
            "parameters": [0.], "inspect_definition": true
        }]
    }))
    .unwrap();
    let response = run_request(&request).unwrap();
    let faces = response.results[0].value.as_array().unwrap();
    assert_eq!(faces.len(), 2);
    let segments = [([0., 0.], [1., 2.]), ([1., 2.], [3., 1.])];
    let mut visited = [false; 2];
    for face in faces {
        let samples = face["samples"].as_array().unwrap();
        assert_eq!(samples.len(), 81);
        let first = samples[0].as_array().unwrap();
        let index = segments
            .iter()
            .position(|(start, _)| {
                (0..2).all(|c| (first[c].as_f64().unwrap() - start[c]).abs() < 1e-9)
            })
            .unwrap();
        assert!(!visited[index]);
        visited[index] = true;
        let (start, end) = segments[index];
        for j in 0..=8 {
            let v = j as f64 / 8.;
            for i in 0..=8 {
                let expected = [
                    start[0] * (1. - v) + end[0] * v,
                    start[1] * (1. - v) + end[1] * v,
                    5. * i as f64 / 8.,
                ];
                for (c, expected) in expected.into_iter().enumerate() {
                    let actual = samples[j * 9 + i][c].as_f64().unwrap();
                    assert!((actual - expected).abs() < 1e-9, "{index}/{j}/{i}/{c}");
                }
            }
        }
    }
    assert!(visited.into_iter().all(|v| v));
}
