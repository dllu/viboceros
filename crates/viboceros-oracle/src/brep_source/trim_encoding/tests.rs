use super::*;

#[test]
fn endpoint_recipes_validate_shape_bounds_and_original_trim_representations() {
    let source = Brep::try_box(
        viboceros_command::CommandContext::default().construction_plane,
        [[-1., 1.]; 3],
        Tolerance::DEFAULT,
    )
    .unwrap();
    let valid = json!({"degree":2,"endpoint_indices":[0,1,1],"weights":[1,2,1],
        "knots":[-3,-3,-3,7,7,7]});
    let encoding: EndpointEncoding = serde_json::from_value(valid.clone()).unwrap();
    let encoded = encoding.apply(source.clone(), Tolerance::DEFAULT).unwrap();
    assert!(matches!(
        encoding.apply(encoded, Tolerance::DEFAULT),
        Err(ProbeError::FixtureInvariant(_))
    ));
    for (key, value) in [
        ("degree", json!(0)),
        ("degree", json!(9)),
        ("endpoint_indices", json!([])),
        ("endpoint_indices", json!([0])),
        ("endpoint_indices", json!([0, 2, 1])),
        ("endpoint_indices", json!([1, 1, 1])),
        ("endpoint_indices", json!([0, 1, 0])),
        ("weights", json!([1, 2])),
        ("knots", json!([0, 0, 1, 1])),
        ("endpoint_indices", json!(vec![0; 65])),
    ] {
        let mut input = valid.clone();
        input[key] = value;
        let invalid: EndpointEncoding = serde_json::from_value(input).unwrap();
        assert!(
            matches!(
                invalid.apply(source.clone(), Tolerance::DEFAULT),
                Err(ProbeError::FixtureInvariant(_))
            ),
            "{key}"
        );
    }
    for weights in [vec![1., 0., 1.], vec![1., -1., 1.]] {
        let mut invalid = encoding.clone();
        invalid.weights = weights;
        assert!(invalid.apply(source.clone(), Tolerance::DEFAULT).is_err());
    }
}
