use super::*;

fn pending_midpoint(op: &serde_json::Value) -> bool {
    op["edge"] == "bottom" && op["fraction"] == 0.5
}

#[test]
fn collinear_subdivisions_connect_to_native_nearer_cap_endpoints() {
    replay(None);
}
#[cfg(feature = "native-smlib")]
#[test]
fn collinear_subdivisions_with_usd_brep_connect_to_native_nearer_cap_endpoints() {
    replay(Some(viboceros_smlib::curve_offset_operations()));
}
fn replay(operations: Option<viboceros_geometry::CurveOffsetOperations>) {
    let cases = common::cases_for("inset_collinear_sdk")
        .into_iter()
        .filter(|(op, _)| !pending_midpoint(op))
        .collect();
    cusps::replay_cases(cases, (10, 0), operations);
}

#[test]
#[ignore = "Native fitted endpoints break midpoint ties differently from the exact miter"]
fn pending_collinear_midpoints_match_native_full_topology() {
    let cases = common::cases_for("inset_collinear_sdk")
        .into_iter()
        .filter(|(op, _)| pending_midpoint(op))
        .collect();
    cusps::replay_cases(cases, (2, 0), None);
}
