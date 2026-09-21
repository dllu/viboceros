use super::*;

#[test]
fn interior_isocurve_join_certifies_all_boundaries_and_preserves_independent_area_accuracy() {
    let fixture =
        include_str!("../../../../../tools/rhino_oracle/fixtures/join_isocurve_certificates.json");
    let request: ProbeRequest = serde_json::from_str(fixture).unwrap();
    let definitions: Value = serde_json::from_str(fixture).unwrap();
    let reference: Value = serde_json::from_str(include_str!(
        "../../../../../tools/rhino_oracle/observations/join_isocurve_local.json"
    ))
    .unwrap();
    let native = run_request_audit(&request).unwrap();
    assert_eq!(native.outcomes.len(), 64);
    assert_eq!(reference["results"].as_array().unwrap().len(), 32);
    let mut compared = 0;
    for (outcome, input) in native
        .outcomes
        .iter()
        .zip(definitions["operations"].as_array().unwrap())
    {
        let result = match outcome {
            OperationOutcome::Success { result } => result,
            OperationOutcome::Failure { id, error } => panic!("{id}: {error:?}"),
        };
        let id = result.id.as_str();
        assert_eq!(input["id"], id);
        let value = &result.value;
        assert_eq!(value["succeeded"], true);
        let outputs = surfaces::outputs(value);
        assert_eq!(outputs.len(), 1);
        let brep = &outputs[0]["brep"];
        assert_eq!(brep["surfaces"].as_array().unwrap().len(), 2);
        for i in 0..2 {
            let definition: NurbsSurfaceDefinition =
                serde_json::from_value(input["sources"][i]["brep"]["source"]["surface"].clone())
                    .unwrap();
            let original = nurbs_surface_from_definition(&definition).unwrap();
            assert_eq!(
                brep["surfaces"][i],
                nurbs_surface_definition_value(&original),
                "{id}"
            );
        }
        let mut mates = 0;
        let half_gap = 1. / 4096.;
        for (edge, tolerance) in brep["edges"]
            .as_array()
            .unwrap()
            .iter()
            .zip(brep["edge_tolerances"].as_array().unwrap())
        {
            let tolerance = tolerance.as_f64().unwrap();
            assert!(
                (0. ..half_gap + 1e-12).contains(&tolerance),
                "{id}: {tolerance}"
            );
            if edge["uses"] != 2 {
                continue;
            }
            mates += 1;
            // These independently constructed boundary images lie on z=0 and
            // z=1/2048. Their common seam must not understate the half-gap.
            assert!(tolerance >= half_gap, "{id}: {tolerance}");
            for p in edge["curve"]["definition"]["control_points"]
                .as_array()
                .unwrap()
            {
                assert!(
                    (p["point"][2].as_f64().unwrap() - half_gap).abs() < 1e-17,
                    "{id}"
                );
            }
        }
        assert_eq!(mates, 1);
        for tolerance in brep["vertex_tolerances"].as_array().unwrap() {
            assert!(
                tolerance.as_f64().unwrap() <= 1.001 * half_gap + 1e-12,
                "{id}"
            );
        }
        // Independent global tensor-basis Gaussian quadrature at orders 32 and
        // 64 agrees past twenty decimal places. Reflection, UV transposition,
        // and exact UV translation preserve these areas for originals/outputs.
        let area = if id.starts_with("extrusion-clamped-") {
            "42.11468947316492072265396448013234483"
        } else if id.starts_with("extrusion-unclamped-") {
            "15.50433131173025622142992181122237353"
        } else if id.starts_with("tensor-clamped-") {
            "48.94204697822932356613645356434878512"
        } else {
            assert!(id.starts_with("tensor-unclamped-"));
            "14.24645295123724288144204849365714259"
        }
        .parse::<f64>()
        .unwrap();
        for object in value["objects"].as_array().unwrap() {
            for face in object["brep"]["faces"].as_array().unwrap() {
                assert!(
                    (face[1].as_f64().unwrap() - area).abs() < 5e-13,
                    "{id}: {}",
                    face[1]
                );
            }
        }
        if let Some(expected) = reference["results"]
            .as_array()
            .unwrap()
            .iter()
            .find(|r| r["id"] == id)
        {
            compare_geometry_except_recorded_integrals_and_outer_knots(
                value,
                &expected["value"],
                id,
            );
            compared += 1;
        } else {
            // The full translated batch exceeded its Rhino observation limit.
            // Missing records are not matches, mismatches, or native errors.
            assert!(id.contains("-origin1e+12-"));
        }
    }
    assert_eq!(compared, 32);
}

fn compare_geometry_except_recorded_integrals_and_outer_knots(
    actual: &Value,
    expected: &Value,
    id: &str,
) {
    let mut geometry = actual.clone();
    let (a, b) = (
        geometry["objects"].as_array_mut().unwrap(),
        expected["objects"].as_array().unwrap(),
    );
    assert_eq!(a.len(), b.len());
    for (a, b) in a.iter_mut().zip(b) {
        let (af, bf) = (
            a["brep"]["faces"].as_array_mut().unwrap(),
            b["brep"]["faces"].as_array().unwrap(),
        );
        assert_eq!(af.len(), bf.len());
        for (a, b) in af.iter_mut().zip(bf) {
            // Native areas were checked against the independent reference
            // above. This isolated geometry check is NOT a full raw match.
            a[1] = b[1].clone();
        }
        if id.contains("-unclamped-") {
            let key = if id.contains("-uv0-") {
                "knots_u"
            } else {
                "knots_v"
            };
            let (a, b) = (
                a["brep"]["surfaces"].as_array_mut().unwrap(),
                b["brep"]["surfaces"].as_array().unwrap(),
            );
            assert_eq!(a.len(), b.len());
            for (a, b) in a.iter_mut().zip(b) {
                assert_eq!(a[key].as_array().unwrap().len(), 6);
                assert_eq!(b[key][0], b[key][1]);
                assert_eq!(b[key][5], b[key][4]);
                a[key][0] = b[key][0].clone();
                a[key][5] = b[key][5].clone();
            }
        }
    }
    // Full replay preserves and reports every integral/knot difference.
    compare(&geometry, expected, id);
}
