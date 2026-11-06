#[path = "support/curve_two_views_audit.rs"]
mod exact;
#[path = "support/observation.rs"]
mod observation;
#[test]
fn bernstein_exclusion_handles_interior_hits_and_separated_curves() {
    let controls = [[0., 0., 0.], [1., 0., 0.], [2., 0., 0.], [3., 0., 0.]];
    assert!(exact::outside_cubic_ball(controls, [1.5, 1., 0.], 0.5).is_some());
    assert!(exact::outside_cubic_ball(controls, [1.5, 0., 0.], 0.5).is_none());
    assert!(exact::outside_cubic_ball(controls, controls[0], 1e-6).is_none());
}
#[test]
fn native_reference_points_are_provably_outside_the_exact_reconstruction_gate() {
    let base = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tools/rhino_oracle");
    let request: serde_json::Value = serde_json::from_slice(
        &std::fs::read(base.join("fixtures/curve_two_views_audit.json")).unwrap(),
    )
    .unwrap();
    let native = observation::read(&base, "curve_two_views_audit");
    assert_eq!(native["results"].as_array().unwrap().len(), 5);
    for (op, row) in request["operations"]
        .as_array()
        .unwrap()
        .iter()
        .zip(native["results"].as_array().unwrap())
    {
        assert_eq!(op["id"], row["id"]);
        let a = &op["curves"][0];
        let b = &op["curves"][1];
        assert_eq!(
            op["directions"],
            serde_json::json!([[0., 0., 1.], [0., 1., 0.]])
        );
        for curve in [a, b] {
            assert_eq!(curve["degree"], 3);
            assert_eq!(curve["control_points"].as_array().unwrap().len(), 4);
            assert!(curve.get("domain").is_none());
            assert_eq!(
                curve["knots"],
                serde_json::json!([0., 0., 0., 0., 1., 1., 1., 1.])
            );
            assert!(
                curve["control_points"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .all(|p| p["weight"].as_f64() == Some(1.))
            );
        }
        for i in 0..4 {
            assert_eq!(
                a["control_points"][i]["point"][0],
                b["control_points"][i]["point"][0]
            );
            assert_eq!(a["control_points"][i]["point"][2], 0.);
            assert_eq!(b["control_points"][i]["point"][1], 0.);
            if i > 0 {
                assert!(
                    a["control_points"][i]["point"][0].as_f64().unwrap()
                        > a["control_points"][i - 1]["point"][0].as_f64().unwrap()
                );
            }
        }
        let controls = std::array::from_fn(|i| {
            [
                a["control_points"][i]["point"][0].as_f64().unwrap(),
                a["control_points"][i]["point"][1].as_f64().unwrap(),
                b["control_points"][i]["point"][2].as_f64().unwrap(),
            ]
        });
        let p: [f64; 3] =
            serde_json::from_value(row["value"]["audit"]["worst"]["output"].clone()).unwrap();
        let certificate = exact::outside_cubic_ball(controls, p, 4e-5).unwrap();
        println!(
            "TWO_VIEWS_EXACT_AUDIT {}",
            serde_json::json!({"id":op["id"],"model_epsilon":1e-6,"certificate":certificate,"reference_point":p})
        );
    }
}
