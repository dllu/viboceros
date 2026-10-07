use super::*;

fn request() -> Value {
    serde_json::from_str(include_str!(
        "../../../../tools/rhino_oracle/fixtures/surface_pushup_certified.json"
    ))
    .unwrap()
}

#[test]
fn constructs_certified_images_of_all_thirteen_native_uv_sources() {
    let capture: Value = serde_json::from_str(include_str!(
        "../../../../tools/rhino_oracle/observations/surface_pushup_certified.json"
    ))
    .unwrap();
    let request = request();
    assert_eq!(
        capture["results"].as_array().unwrap().len(),
        request["operations"].as_array().unwrap().len()
    );
    for (row, operation) in capture["results"]
        .as_array()
        .unwrap()
        .iter()
        .zip(request["operations"].as_array().unwrap())
    {
        let operation: Operation = serde_json::from_value(operation.clone()).unwrap();
        assert_eq!(operation.id(), row["id"].as_str().unwrap());
        let Operation::SurfacePushupCertified { fixture, .. } = operation else {
            panic!()
        };
        let (value, _) = pushup(&fixture, Tolerance::default(), 1).unwrap();
        assert_eq!(value["certified"], true, "{}", row["id"]);
        assert!(value["bound"].as_f64().unwrap() <= 1e-6);
        let spatial: NurbsCurveDefinition =
            serde_json::from_value(value["spatial_curve"].clone()).unwrap();
        let model = nurbs_curve_from_definition(&spatial).unwrap();
        let sampler = model.parameter_sampler().unwrap();
        for sample in row["value"]["samples"].as_array().unwrap() {
            let native = Point3::try_from(
                serde_json::from_value::<[f64; 3]>(sample["image"].clone()).unwrap(),
            )
            .unwrap();
            assert!(
                sampler
                    .evaluate(sample["fraction"].as_f64().unwrap())
                    .unwrap()
                    .distance_to(native)
                    .unwrap()
                    <= 1e-6 + 1e-11,
                "{}",
                row["id"]
            );
        }
        let check = SurfaceCurveDeviationFixture {
            surface: fixture.surface,
            parameter_curve: fixture.parameter_curve,
            spatial_curve: spatial,
            limit: 1e-6,
        };
        assert_eq!(run(&check, 1).unwrap().0["certified"], true);
    }
}

#[test]
fn pushup_protocol_rejects_nonzero_z_and_invalid_limits() {
    let mut fixture: SurfacePushupFixture =
        serde_json::from_value(request()["operations"][0].clone()).unwrap();
    fixture.parameter_curve.control_points[0].point[2] = 1.;
    assert!(matches!(
        pushup(&fixture, Tolerance::default(), 1),
        Err(ProbeError::FixtureInvariant(_))
    ));
    fixture.parameter_curve.control_points[0].point[2] = 0.;
    fixture.limit = -1.;
    assert!(pushup(&fixture, Tolerance::default(), 1).is_err());
}
