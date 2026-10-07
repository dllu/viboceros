use super::*;

fn capture() -> Value {
    serde_json::from_str(include_str!(
        "../../../../tools/rhino_oracle/observations/surface_pullback_endpoints.json"
    ))
    .unwrap()
}

pub(super) fn operation(row: &Value) -> Value {
    let v = &row["value"];
    let s = &v["surface"];
    json!({
        "op":"surface_pullback_certified", "id":row["id"],
        "surface":{
            "degree_u":s["degree"][0], "degree_v":s["degree"][1],
            "control_point_count_u":s["control_count"][0],
            "control_point_count_v":s["control_count"][1],
            "control_points":s["control_points"],
            "knots_u":s["knots_u"], "knots_v":s["knots_v"],
            "domain_u":s["domain_u"], "domain_v":s["domain_v"]
        },
        "spatial_curve":v["spatial_curve"], "limit":v["limit"],
        "endpoints":v["endpoints"]
    })
}

pub(super) fn request(operations: Vec<Value>) -> ProbeRequest {
    serde_json::from_value(json!({
        "protocol_version":PROTOCOL_VERSION, "iterations":1,
        "operations":operations
    }))
    .unwrap()
}

#[test]
fn python_pullback_replays_eight_native_sources_and_fixed_endpoint_witnesses() {
    let capture = capture();
    let rows = capture["results"].as_array().unwrap();
    assert_eq!(rows.len(), 8);
    let request = request(rows.iter().map(operation).collect());
    let saved_request: ProbeRequest = serde_json::from_str(include_str!(
        "../../../../tools/rhino_oracle/fixtures/surface_pullback_certified.json"
    ))
    .unwrap();
    assert_eq!(request.operations, saved_request.operations);
    let response = run_request(&request).unwrap();
    assert_eq!(response.results.len(), 8);
    for ((row, op), result) in rows.iter().zip(&request.operations).zip(response.results) {
        let Operation::SurfacePullbackCertified { fixture, .. } = op else {
            panic!("wrong operation");
        };
        assert_eq!(result.id, row["id"].as_str().unwrap());
        let id = &result.id;
        assert_eq!(result.value["certified"], true, "{id}");
        assert_eq!(result.value["fixed_endpoints"], row["value"]["endpoints"]);
        assert_eq!(result.value["correspondence"], "normalized_curve_domains");
        let bound = result.value["bound"].as_f64().unwrap();
        assert!(
            bound.is_finite() && bound >= 0. && bound <= fixture.limit,
            "{id}"
        );
        let definition = serde_json::from_value(result.value["parameter_curve"].clone()).unwrap();
        let uv = nurbs_curve_from_definition(&definition).unwrap();
        let source = nurbs_curve_from_definition(&fixture.spatial_curve).unwrap();
        let surface = nurbs_surface_from_definition(&fixture.surface).unwrap();
        assert_eq!(uv.domain(), source.domain(), "{id}");
        let endpoints = fixture.endpoints.unwrap();
        assert_eq!(
            uv.evaluate(*uv.domain().start()).unwrap().to_array(),
            [endpoints[0][0], endpoints[0][1], 0.]
        );
        assert_eq!(
            uv.evaluate(*uv.domain().end()).unwrap().to_array(),
            [endpoints[1][0], endpoints[1][1], 0.]
        );
        for sample in row["value"]["samples"].as_array().unwrap() {
            let fraction = sample["fraction"].as_f64().unwrap();
            let model = source
                .evaluate(source.parameter_at(fraction).unwrap())
                .unwrap();
            let native: [f64; 3] = serde_json::from_value(sample["spatial_point"].clone()).unwrap();
            assert!(
                model.distance_to(point(native).unwrap()).unwrap() < 1e-11,
                "{id}"
            );
            let parameter = uv.evaluate(uv.parameter_at(fraction).unwrap()).unwrap();
            let image = surface.evaluate(parameter.x(), parameter.y()).unwrap();
            assert!(image.distance_to(model).unwrap() <= bound + 1e-12, "{id}");
            // Independently evaluate the native pullback station on the retained
            // tensor surface. Its speed differs from the local contract on the
            // planar recipes, so do not equate the two UV parameterizations.
            let native_uv: [f64; 2] =
                serde_json::from_value(sample["parameter_point"].clone()).unwrap();
            let native_image: [f64; 3] =
                serde_json::from_value(sample["surface_point"].clone()).unwrap();
            assert!(
                surface
                    .evaluate(native_uv[0], native_uv[1])
                    .unwrap()
                    .distance_to(point(native_image).unwrap())
                    .unwrap()
                    < 1e-11,
                "{id}"
            );
        }
        for sample in row["value"]["endpoint_samples"].as_array().unwrap() {
            let native_uv: [f64; 2] =
                serde_json::from_value(sample["parameter_point"].clone()).unwrap();
            let native_image: [f64; 3] =
                serde_json::from_value(sample["surface_point"].clone()).unwrap();
            assert!(
                surface
                    .evaluate(native_uv[0], native_uv[1])
                    .unwrap()
                    .distance_to(point(native_image).unwrap())
                    .unwrap()
                    < 1e-11,
                "{id}"
            );
        }
    }
}

#[test]
fn python_pullback_allows_omitted_endpoints_and_rejects_invalid_constraints() {
    let capture = capture();
    let row = &capture["results"][0];
    let mut op = operation(row);
    op.as_object_mut().unwrap().remove("endpoints");
    let response = run_request(&request(vec![op.clone()])).unwrap();
    assert_eq!(response.results[0].value["certified"], true);
    assert_eq!(response.results[0].value["fixed_endpoints"], Value::Null);
    for limit in [0., -1.] {
        let mut invalid = op.clone();
        invalid["limit"] = json!(limit);
        assert!(run_request(&request(vec![invalid])).is_err());
    }
    for endpoints in [json!([[0., 0.], [1.1, 0.5]]), json!([[0., 0.], [0.5, 0.5]])] {
        let mut invalid = op.clone();
        invalid["endpoints"] = endpoints;
        assert!(run_request(&request(vec![invalid])).is_err());
    }
    for endpoints in [json!([[0., 0.]]), json!([[0., 0., 0.], [1., 0.5, 0.]])] {
        let mut invalid = op.clone();
        invalid["endpoints"] = endpoints;
        assert!(serde_json::from_value::<Operation>(invalid).is_err());
    }
    let Operation::SurfacePullbackCertified { mut fixture, .. } =
        serde_json::from_value(operation(row)).unwrap()
    else {
        panic!("wrong operation")
    };
    for limit in [f64::NAN, f64::INFINITY] {
        fixture.limit = limit;
        assert!(pullback(&fixture, Tolerance::DEFAULT, 1).is_err());
    }
}
