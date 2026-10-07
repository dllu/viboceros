use super::pullback_tests::{operation, request};
use super::*;

#[test]
fn previously_failing_closed_native_isocurves_now_certify_without_endpoint_constraints() {
    let capture: Value = serde_json::from_str(include_str!(
        "../../../../tools/rhino_oracle/observations/surface_pullback_endpoints.json"
    ))
    .unwrap();
    let rows = capture["results"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|row| !row["id"].as_str().unwrap().contains("planar"))
        .collect::<Vec<_>>();
    assert_eq!(rows.len(), 6);
    let operations = rows
        .iter()
        .map(|row| {
            let mut op = operation(row);
            op.as_object_mut().unwrap().remove("endpoints");
            op
        })
        .collect();
    let response = run_request(&request(operations)).unwrap();
    for (row, result) in rows.iter().zip(response.results) {
        assert_eq!(result.value["fixed_endpoints"], Value::Null);
        assert_eq!(result.value["certified"], true);
        assert_eq!(result.value["bound"], 0.);
        let controls = result.value["parameter_curve"]["control_points"]
            .as_array()
            .unwrap();
        assert_eq!(controls.len(), 2);
        assert_eq!(controls[0]["point"][0], row["value"]["endpoints"][0][0]);
        assert_eq!(controls[1]["point"][0], row["value"]["endpoints"][1][0]);
    }
}

#[test]
fn automatic_pullbacks_replay_full_native_chart_and_singularity_sources_with_continuous_bounds() {
    let capture: Value = serde_json::from_str(include_str!(
        "../../../../tools/rhino_oracle/observations/surface_pullback_linear.json"
    ))
    .unwrap();
    let input: ProbeRequest = serde_json::from_str(include_str!(
        "../../../../tools/rhino_oracle/fixtures/surface_pullback_linear_certified.json"
    ))
    .unwrap();
    let rows = capture["results"].as_array().unwrap();
    assert_eq!(rows.len(), 24);
    let derived = request(
        rows.iter()
            .map(|row| {
                let mut op = operation(row);
                op.as_object_mut().unwrap().remove("endpoints");
                op
            })
            .collect(),
    );
    assert_eq!(input.operations, derived.operations);
    let response = run_request(&input).unwrap();
    for ((row, op), result) in rows.iter().zip(input.operations).zip(response.results) {
        let Operation::SurfacePullbackCertified { fixture, .. } = op else {
            panic!("wrong operation")
        };
        assert!(fixture.endpoints.is_none());
        assert_eq!(result.id, row["id"].as_str().unwrap());
        let id = &result.id;
        assert_eq!(result.value["fixed_endpoints"], Value::Null);
        assert_eq!(result.value["certified"], true);
        let bound = result.value["bound"].as_f64().unwrap();
        assert!(
            bound.is_finite() && bound >= 0. && bound <= fixture.limit,
            "{id}"
        );
        assert!(
            bound < 1e-12,
            "{id}: exact linear knot crossings should retain a tight bound: {bound}"
        );
        let definition = serde_json::from_value(result.value["parameter_curve"].clone()).unwrap();
        let uv = nurbs_curve_from_definition(&definition).unwrap();
        let surface = nurbs_surface_from_definition(&fixture.surface).unwrap();
        let spatial = nurbs_curve_from_definition(&fixture.spatial_curve).unwrap();
        assert_eq!(uv.degree(), 1, "{id}");
        assert_eq!(uv.control_points().len(), 2, "{id}");
        assert_eq!(uv.domain(), spatial.domain(), "{id}");
        let uv2 = NurbsCurve2::try_new(
            1,
            uv.control_points()
                .iter()
                .map(|p| Point2::try_new(p.point().x(), p.point().y()).unwrap())
                .collect(),
            uv.knots().to_vec(),
        )
        .unwrap();
        assert_eq!(
            surface
                .parameter_curve_deviation_bound(&uv2, &spatial, fixture.limit)
                .unwrap(),
            Some(bound),
            "{id}"
        );
        for sample in row["value"]["samples"].as_array().unwrap() {
            let fraction = sample["fraction"].as_f64().unwrap();
            let native_model: [f64; 3] =
                serde_json::from_value(sample["spatial_point"].clone()).unwrap();
            let model = spatial
                .evaluate(spatial.parameter_at(fraction).unwrap())
                .unwrap();
            assert!(
                model.distance_to(point(native_model).unwrap()).unwrap() < 1e-11,
                "{id}"
            );
            let parameter = uv.evaluate(uv.parameter_at(fraction).unwrap()).unwrap();
            let image = surface.evaluate(parameter.x(), parameter.y()).unwrap();
            assert!(image.distance_to(model).unwrap() <= bound + 1e-12, "{id}");
            let reference_uv: [f64; 2] =
                serde_json::from_value(sample["reference_parameter_point"].clone()).unwrap();
            let reference_image: [f64; 3] =
                serde_json::from_value(sample["reference_surface_point"].clone()).unwrap();
            assert!(
                surface
                    .evaluate(reference_uv[0], reference_uv[1])
                    .unwrap()
                    .distance_to(point(reference_image).unwrap())
                    .unwrap()
                    < 1e-11,
                "{id}"
            );
            if row["value"]["native_pullback_succeeded"].as_bool().unwrap() {
                let native_uv: [f64; 2] =
                    serde_json::from_value(sample["native_parameter_point"].clone()).unwrap();
                let native_image: [f64; 3] =
                    serde_json::from_value(sample["native_surface_point"].clone()).unwrap();
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
}
