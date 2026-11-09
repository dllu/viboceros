//! Default and zero semantics of the local certified fitting protocol.
use serde_json::{Value, json};
use viboceros_oracle::{ProbeRequest, run_request};

fn run(operation: &Value) -> Value {
    let request: ProbeRequest = serde_json::from_value(json!({"protocol_version":1,"iterations":1,
        "tolerance":{"absolute":0.05,"relative":1e-9,"angular":1e-6},"operations":[operation]}))
    .unwrap();
    run_request(&request).unwrap().results.remove(0).value
}

#[test]
fn omitted_and_null_tolerances_use_the_request_tolerance_and_default_ceilings() {
    let query: Value = serde_json::from_slice(
        &std::fs::read(
            std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("../../tools/rhino_oracle/fixtures/surface_fit_strips.json"),
        )
        .unwrap(),
    )
    .unwrap();
    let mut op = json!({"op":"surface_fit_bounded","id":"defaults","surface":query["operations"][0]["surface"],"degree_u":3,"degree_v":1});
    let omitted = run(&op);
    assert!(!omitted["surface"].is_null());
    assert!(omitted["maximum_deviation_bound"].as_f64().unwrap() <= 0.05);
    op["absolute_tolerance"] = json!(null);
    assert_eq!(run(&op), omitted);
    op["absolute_tolerance"] = json!(0.05);
    op["maximum_control_count"] = json!([64, 64]);
    assert_eq!(run(&op), omitted);
}

#[test]
fn protocol_zero_is_strict_and_accepts_an_exact_constant_surface() {
    let surface = json!({"degree_u":1,"degree_v":1,"control_point_count_u":2,"control_point_count_v":2,
        "knots_u":[0.,0.,1.,1.],"knots_v":[0.,0.,1.,1.],
        "control_points":[{"point":[2.,3.,4.],"weight":1.},{"point":[2.,3.,4.],"weight":1.},
                          {"point":[2.,3.,4.],"weight":1.},{"point":[2.,3.,4.],"weight":1.}]});
    let result = run(
        &json!({"op":"surface_fit_bounded","id":"zero","surface":surface,"degree_u":3,"degree_v":3,"absolute_tolerance":0.}),
    );
    assert!(!result["surface"].is_null());
    assert_eq!(result["maximum_deviation_bound"], 0.);
    assert_eq!(result["before"], result["after"]);
}
