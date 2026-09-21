use super::*;

#[test]
fn presplit_curved_boundaries_keep_certified_gaps_without_hiding_edge_cleanup_differences() {
    let request: ProbeRequest = serde_json::from_str(include_str!(
        "../../../../../tools/rhino_oracle/fixtures/join_trim_certificates.json"
    ))
    .unwrap();
    let expected: Value = serde_json::from_str(include_str!(
        "../../../../../tools/rhino_oracle/observations/join_trim_certificates.json"
    ))
    .unwrap();
    let native = run_request_audit(&request).unwrap();
    assert_eq!(native.outcomes.len(), 80);
    assert_eq!(expected["results"].as_array().unwrap().len(), 80);
    for (outcome, reference) in native
        .outcomes
        .iter()
        .zip(expected["results"].as_array().unwrap())
    {
        let result = match outcome {
            OperationOutcome::Success { result } => result,
            OperationOutcome::Failure { id, error } => panic!("{id}: {error:?}"),
        };
        let id = reference["id"].as_str().unwrap();
        assert_eq!(result.id, id);
        let (actual, expected) = (&result.value, &reference["value"]);
        // Independent 80-digit global-basis integration, splitting at knots,
        // gives these areas for a height-3 extrusion. The quadratic also has
        // the closed form 45*(sqrt(2)+asinh(1)). Large UV offsets must not change
        // native area accuracy to imitate Rhino's translated integration loss.
        let area = if id.starts_with("quadratic-") {
            "103.30142172266871333154341221352706744"
        } else if id.starts_with("rational-cubic-") {
            "93.03973219600359257481171327205076389"
        } else if id.starts_with("unclamped-") {
            "35.43003670080353622934934492613563630"
        } else {
            "126.25788197408796570759010047319074714"
        }
        .parse::<f64>()
        .unwrap();
        for object in actual["objects"].as_array().unwrap() {
            for face in object["brep"]["faces"].as_array().unwrap() {
                assert!((face[1].as_f64().unwrap() - area).abs() < 5e-13, "{id}");
            }
        }
        assert_eq!(actual["succeeded"], true);
        assert_eq!(expected["succeeded"], true);
        let (a, b) = (surfaces::outputs(actual), surfaces::outputs(expected));
        assert_eq!((a.len(), b.len()), (1, 1));
        let (a, b) = (&a[0]["brep"], &b[0]["brep"]);
        compare(&a["face_reversed"], &b["face_reversed"], id);
        assert_eq!(a["surfaces"].as_array().unwrap().len(), 2);
        assert_eq!(b["surfaces"].as_array().unwrap().len(), 2);
        for (a, b) in a["surfaces"]
            .as_array()
            .unwrap()
            .iter()
            .zip(b["surfaces"].as_array().unwrap())
        {
            verify_surface_record(a, b, id);
        }
        // Independent witness: both original boundary images lie in parallel
        // z=0 and z=0.0005 planes. The retained seam lies at their midpoint.
        // An uncertainty below the normal separation would be unsafe, even if
        // a differently parameterized reference happened to report it.
        let mut mates = 0;
        for (edge, tolerance) in a["edges"]
            .as_array()
            .unwrap()
            .iter()
            .zip(a["edge_tolerances"].as_array().unwrap())
        {
            if edge["uses"] != 2 {
                continue;
            }
            mates += 1;
            let bound = tolerance.as_f64().unwrap();
            assert!((0.00025..0.000250000001).contains(&bound), "{id}: {bound}");
            for p in edge["curve"]["definition"]["control_points"]
                .as_array()
                .unwrap()
            {
                assert!(
                    (p["point"][2].as_f64().unwrap() - 0.00025).abs() < 1e-18,
                    "{id}"
                );
            }
        }
        assert!(mates > 0);
        // Rhino coalesces a representation-dependent subset of the split
        // seams. These are all raw discrepancies, not normalized full matches.
        let rhino_edges = if id.starts_with("quadratic-tiny-") {
            8
        } else if id.starts_with("quadratic-") {
            7
        } else if id.starts_with("rational-cubic-") {
            9
        } else if id.starts_with("unclamped-uv0-") {
            10
        } else if id.starts_with("unclamped-uv1-") || id.starts_with("multispan-uv0-") {
            8
        } else {
            assert!(id.starts_with("multispan-uv1-"));
            9
        };
        assert_eq!(b["edges"].as_array().unwrap().len(), rhino_edges, "{id}");
    }
}

fn verify_surface_record(actual: &Value, expected: &Value, id: &str) {
    let mut active = actual.clone();
    if id.starts_with("unclamped-") {
        let key = if id.starts_with("unclamped-uv1-") {
            "knots_v"
        } else {
            "knots_u"
        };
        let n = active[key].as_array().unwrap().len();
        assert_eq!(n, 6);
        assert_eq!(expected[key][0], expected[key][1]);
        assert_eq!(expected[key][n - 1], expected[key][n - 2]);
        // OpenNURBS omits the redundant first/last full-vector knots. Only
        // this explicit representation check substitutes them; reports do not.
        active[key][0] = expected[key][0].clone();
        active[key][n - 1] = expected[key][n - 1].clone();
    }
    compare(&active, expected, id);
}
