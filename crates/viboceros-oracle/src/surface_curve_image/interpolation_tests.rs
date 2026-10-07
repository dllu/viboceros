use super::pullback_tests::{operation, request};
use super::*;

#[test]
fn derivative_free_fits_replay_ten_native_nonlinear_singular_sources_with_complete_certificates() {
    let native: Value = serde_json::from_str(include_str!(
        "../../../../tools/rhino_oracle/observations/surface_pullback_interpolation.json"
    ))
    .unwrap();
    let before: Value = serde_json::from_str(include_str!(
        "../../../../tools/rhino_oracle/observations/surface_pullback_interpolation_before.json"
    ))
    .unwrap();
    let input: ProbeRequest = serde_json::from_str(include_str!(
        "../../../../tools/rhino_oracle/fixtures/surface_pullback_interpolation_certified.json"
    ))
    .unwrap();
    let rows = native["results"].as_array().unwrap();
    assert_eq!(rows.len(), 10);
    assert_eq!(before["outcomes"].as_array().unwrap().len(), 10);
    assert!(
        before["outcomes"]
            .as_array()
            .unwrap()
            .iter()
            .all(|r| r["status"] == "failure")
    );
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
        let id = &result.id;
        assert_eq!(id, row["id"].as_str().unwrap());
        assert_eq!(result.value["certified"], true);
        assert_eq!(result.value["fixed_endpoints"], Value::Null);
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
        let uv2 = NurbsCurve2::try_new_rational(
            uv.degree(),
            uv.control_points()
                .iter()
                .map(|p| {
                    WeightedPoint2::try_new(
                        Point2::try_new(p.point().x(), p.point().y()).unwrap(),
                        p.weight(),
                    )
                    .unwrap()
                })
                .collect(),
            uv.knots().to_vec(),
        )
        .unwrap();
        assert_eq!(
            surface
                .parameter_curve_deviation_bound(&uv2, &source, fixture.limit)
                .unwrap(),
            Some(bound),
            "{id}"
        );
        let uv_samples = uv.parameter_sampler().unwrap();
        let source_samples = source.parameter_sampler().unwrap();
        for sample in row["value"]["samples"].as_array().unwrap() {
            let f = sample["fraction"].as_f64().unwrap();
            let model = source_samples.evaluate(f).unwrap();
            let native_model: [f64; 3] =
                serde_json::from_value(sample["spatial_point"].clone()).unwrap();
            assert!(
                model.distance_to(point(native_model).unwrap()).unwrap() < 1e-11,
                "{id}"
            );
            let uv = uv_samples.evaluate(f).unwrap();
            assert!(
                surface
                    .evaluate(uv.x(), uv.y())
                    .unwrap()
                    .distance_to(model)
                    .unwrap()
                    <= bound + 1e-12,
                "{id}"
            );
            let reference_uv: [f64; 2] =
                serde_json::from_value(sample["reference_parameter_point"].clone()).unwrap();
            let reference_model: [f64; 3] =
                serde_json::from_value(sample["reference_surface_point"].clone()).unwrap();
            assert!(
                surface
                    .evaluate(reference_uv[0], reference_uv[1])
                    .unwrap()
                    .distance_to(point(reference_model).unwrap())
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
