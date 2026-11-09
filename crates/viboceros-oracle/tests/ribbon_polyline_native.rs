//! Ordinary polyline Ribbon support tensors and native topology, at 1e-9.
use serde_json::{Value, json};
use viboceros_geometry::*;
#[path = "../../viboceros-command/tests/support/merge_surfaces.rs"]
#[allow(dead_code)]
mod captured;
#[path = "../../viboceros-command/tests/support/interpolation_plane.rs"]
#[allow(dead_code)]
mod curves;
#[path = "support/ribbon.rs"]
#[allow(dead_code)]
mod support;

#[path = "ribbon_polyline_native/replay.rs"]
mod replay;

#[test]
fn ordinary_polyline_ribbons_close_the_sharp_zigzag_gap() {
    replay::family("ribbon_polyline", 52);
}

#[test]
fn smooth_angles_and_sharp_correspondence_match_independent_native_holdouts() {
    replay::family("ribbon_corner_correspondence", 29);
    replay::family("ribbon_smooth_angles", 11);
    replay::family("ribbon_sharp_holdouts", 12);
    replay::family("ribbon_sharp_seams", 6);
}

#[test]
fn python_geometry_operation_keeps_charts_and_returns_components_without_command_claims() {
    use viboceros_oracle::{ProbeRequest, run_request};
    let mut op = json!({"op":"polyline_ribbon_geometry","id":"ribbon",
        "vertices":[[0.,0.,0.],[5.,0.,0.],[5.,4.,0.]],"parameters":[-4.,0.,8.],
        "distance":-0.5,"both_sides":true,"corner":"Round"});
    let call = |op: &Value| {
        run_request(
            &serde_json::from_value::<ProbeRequest>(
                json!({"protocol_version":1,"iterations":1,"operations":[op]}),
            )
            .unwrap(),
        )
    };
    let v = call(&op).unwrap().results.remove(0).value;
    assert_eq!(v["before"], v["after"]);
    assert_eq!(v["before"]["parameters"], op["parameters"]);
    assert_eq!(v["full_ribbon_command"], false);
    assert_eq!(v["distance_certified"], false);
    assert_eq!(v["breps"].as_array().unwrap().len(), 1);
    assert_eq!(v["breps"][0]["faces"].as_array().unwrap().len(), 7);
    op["both_sides"] = json!(false);
    op["corner"] = json!("None");
    assert_eq!(
        call(&op).unwrap().results[0].value["breps"]
            .as_array()
            .unwrap()
            .len(),
        1
    );
    op["both_sides"] = json!(true);
    op["corner"] = json!("Smooth");
    assert_eq!(
        call(&op).unwrap().results[0].value["breps"][0]["faces"]
            .as_array()
            .unwrap()
            .len(),
        7
    );
    op["corner"] = json!("Unknown");
    assert!(call(&op).is_err());
    op["corner"] = json!("Sharp");
    op["distance"] = json!(0.);
    assert!(call(&op).is_err());
    op["distance"] = json!(0.5);
    op["parameters"] = json!([0., 1.]);
    assert!(call(&op).is_err());
}
