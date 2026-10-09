use super::*;

fn request(steps: Value) -> ProbeRequest {
    serde_json::from_value(json!({"protocol_version": 1, "iterations": 1,
        "operations": [{"op": "block_workflow", "id": "case",
            "sources": [{"type": "point", "point": [11., 22., 33.]},
                        {"type": "point", "point": [4., 5., 6.]}], "steps": steps}]}))
    .unwrap()
}

#[test]
fn independent_placement_witnesses_and_live_shared_redefinition() {
    let r = request(json!([
        {"action":"create", "name":"leaf", "base":[10.,20.,30.], "sources":[0]},
        {"action":"insert", "name":"leaf", "transform":[[0.,-3.,0.,10.],[2.,0.,0.,20.],[0.,0.,-4.,30.],[0.,0.,0.,1.]]},
        {"action":"create", "name":"wrapper", "base":[1.,2.,3.], "sources":[3]},
        {"action":"create", "name":"leaf", "base":[0.,0.,0.], "sources":[1]},
        {"action":"explode", "object":4, "recursive":true}
    ]));
    let result = run_request(&r).unwrap();
    let states = &result.results[0].value["states"];
    assert_eq!(
        states[2]["objects"][2]["geometry"]["leaves"][0]["geometry"]["points"],
        json!([[4., 22., 18.]])
    );
    let final_state = &states[5];
    assert_eq!(final_state["outputs"], json!([6]));
    assert_eq!(
        final_state["objects"][2]["geometry"]["points"],
        json!([[-5., 28., 6.]])
    );
    assert_eq!(final_state["definitions"].as_array().unwrap().len(), 2);
}

#[test]
fn invalid_lifetimes_cycles_matrices_and_iterations_are_errors() {
    let create = json!({"action":"create", "name":"leaf", "base":[0.,0.,0.], "sources":[0]});
    for second in [
        json!({"action":"create", "name":"other", "base":[0.,0.,0.], "sources":[0]}),
        json!({"action":"create", "name":"leaf", "base":[0.,0.,0.], "sources":[2]}),
        json!({"action":"insert", "name":"leaf", "transform":[[0.,0.,0.,0.],[0.,1.,0.,0.],[0.,0.,1.,0.],[0.,0.,0.,1.]]}),
        json!({"action":"insert", "name":"leaf", "transform":[[1.,0.,0.,0.],[0.,1.,0.,0.],[0.,0.,1.,0.],[0.,0.,1.,1.]]}),
        json!({"action":"explode", "object":1}),
    ] {
        assert!(run_request(&request(json!([create.clone(), second]))).is_err());
    }
    let mut r = request(json!([create]));
    r.iterations = 2;
    assert!(run_request(&r).is_err());
}

#[test]
fn bounded_workflow_rejects_too_many_steps_and_handles() {
    let create = json!({"action":"create", "name":"leaf", "base":[0.,0.,0.], "sources":[0]});
    assert!(run_request(&request(json!(vec![create; 65]))).is_err());
    let mut left = 2;
    assert!(spend(&mut left, 3).is_err());
    assert_eq!(left, 2);
    assert_eq!(BlockExplosionApi::default(), BlockExplosionApi::Command);
}
