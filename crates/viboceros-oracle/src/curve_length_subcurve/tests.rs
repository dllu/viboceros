use super::*;

#[test]
fn signed_length_protocol_replays_twenty_original_sdk_sources_and_all_stations() {
    let fixture: Value = serde_json::from_str(include_str!(
        "../../../../tools/rhino_oracle/fixtures/signed_length_subcurves.json"
    ))
    .unwrap();
    let native: Value = serde_json::from_str(include_str!(
        "../../../../tools/rhino_oracle/observations/signed_length_subcurves.json"
    ))
    .unwrap();
    let request: ProbeRequest = serde_json::from_value(fixture).unwrap();
    let local = run_request(&request).unwrap();
    assert_eq!(local.results.len(), 20);
    let tolerance = Tolerance::try_new(1e-9, 1e-12, 1e-10).unwrap();
    for (local, native) in local
        .results
        .iter()
        .zip(native["results"].as_array().unwrap())
    {
        assert_eq!(local.id, native["id"].as_str().unwrap());
        let n = &native["value"];
        assert_eq!(local.value["available"], n["available"], "{}", local.id);
        assert_eq!(local.value["source_unchanged"], true);
        assert_eq!(n["source_unchanged"], true);
        if n["available"] == false {
            continue;
        }
        let definition: NurbsCurveDefinition =
            serde_json::from_value(local.value["curve"]["definition"].clone()).unwrap();
        let curve = nurbs_curve_from_definition(&definition).unwrap();
        for sample in n["curve"]["samples"].as_array().unwrap() {
            let p = Point3::try_from(serde_json::from_value::<[Real; 3]>(sample.clone()).unwrap())
                .unwrap();
            let t = curve.closest_parameter(p, tolerance).unwrap();
            assert!(
                curve.evaluate(t).unwrap().distance_to(p).unwrap() < 1e-6,
                "{} {p:?}",
                local.id
            );
        }
        let stations = n["curve"]["samples"].as_array().unwrap();
        for (parameter, index) in [(*curve.domain().start(), 0), (*curve.domain().end(), 32)] {
            let p = Point3::try_from(
                serde_json::from_value::<[Real; 3]>(stations[index].clone()).unwrap(),
            )
            .unwrap();
            assert!(
                curve.evaluate(parameter).unwrap().distance_to(p).unwrap() < 1e-6,
                "{} endpoint",
                local.id
            );
        }
    }
}

#[test]
fn signed_length_protocol_rejects_anchors_outside_the_original_domain() {
    let fixture: Value = serde_json::from_str(include_str!(
        "../../../../tools/rhino_oracle/fixtures/signed_length_subcurves.json"
    ))
    .unwrap();
    for anchor in [19., 31.] {
        let mut op = fixture["operations"][0].clone();
        op["start"] = json!(anchor);
        let op: Operation = serde_json::from_value(op).unwrap();
        let Operation::CurveSubcurveArcLength {
            curve,
            start,
            length,
            ..
        } = op
        else {
            panic!()
        };
        assert!(run(&curve, start, length, Tolerance::DEFAULT, 1).is_err());
    }
}
