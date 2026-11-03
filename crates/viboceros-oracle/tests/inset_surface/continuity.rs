use super::*;
use viboceros_geometry::{CurveOffsetOperations, PlanarInsetOperations, PlanarInsetOptions};

fn fan(op: &serde_json::Value) -> bool {
    op["crease_corners"] == true
        && op["loose"] == false
        && (op["scene"] == "asymmetric" && matches!(op["distance"].as_f64(), Some(2. | 3.))
            || op["scene"] == "elliptic_stadium" && op["distance"] == 1.5)
}
fn midpoint(op: &serde_json::Value) -> bool {
    op["scene"] == "elliptic_stadium"
        && op["variant"] == "split"
        && op["distance"] == 0.3
        && op["crease_corners"] == true
}
#[test]
fn continuity_sdk_matches_native_smooth_groups_and_records_pending_cusp_fans() {
    replay(None);
}
#[cfg(feature = "native-smlib")]
#[test]
fn continuity_sdk_with_usd_brep_matches_native_smooth_groups_and_records_pending_cusp_fans() {
    replay(Some(viboceros_smlib::curve_offset_operations()));
}
fn replay(operations: Option<CurveOffsetOperations>) {
    let mut supported = Vec::new();
    let mut pending = 0;
    let tolerance = Tolerance::try_new(1e-6, 1e-12, 1e-6).unwrap();
    for (op, native) in common::cases_for("inset_continuity_sdk") {
        if midpoint(&op) {
            continue;
        }
        if !fan(&op) {
            supported.push((op, native));
            continue;
        }
        let label = op["id"].as_str().unwrap();
        let source = source(&native["before"]);
        let before = source.clone();
        assert!(native["output"]["valid"].as_bool().unwrap(), "{label}");
        let error = source
            .try_inset_planar_faces_with_options(
                &[0],
                PlanarInsetOptions {
                    distance: op["distance"].as_f64().unwrap(),
                    crease_corners: true,
                    loose: false,
                },
                tolerance,
                PlanarInsetOperations {
                    curves: operations,
                    ..Default::default()
                },
            )
            .err()
            .unwrap_or_else(|| panic!("{label}: expected an unsupported cusp error"));
        assert!(error.to_string().contains("cusp"), "{label}: {error}");
        assert_eq!(source, before);
        pending += 1;
    }
    assert_eq!((supported.len(), pending), (23, 4));
    cusps::replay_cases(supported, (17, 6), operations);
}

#[test]
fn continuity_commands_match_supported_geometry_metadata_history_and_3dm() {
    let cases = common::cases_for("inset_continuity_command")
        .into_iter()
        .filter(|(op, _)| !fan(op) && !midpoint(op))
        .collect::<Vec<_>>();
    assert_eq!(cases.len(), 69);
    cusps::replay_command_cases(cases, (51, 18));
}

#[test]
fn curvature_limits_match_native_g2_grouping() {
    cusps::replay_family("inset_continuity_limits", (6, 0), None);
}
#[cfg(feature = "native-smlib")]
#[test]
fn curvature_limits_with_usd_brep_match_native_g2_grouping() {
    cusps::replay_family(
        "inset_continuity_limits",
        (6, 0),
        Some(viboceros_smlib::curve_offset_operations()),
    );
}

#[test]
#[ignore = "Native creased cusps create fitted triangular fans; open offset cusps currently decline"]
fn pending_native_creased_cusp_fans_match_full_topology() {
    let cases = common::cases_for("inset_continuity_sdk")
        .into_iter()
        .filter(|(op, _)| fan(op))
        .collect();
    cusps::replay_cases(cases, (4, 0), None);
}

#[test]
#[ignore = "Native midpoint connector chooses the opposite equidistant cap endpoint"]
fn pending_native_midpoint_connector_matches_full_topology() {
    let cases = common::cases_for("inset_continuity_sdk")
        .into_iter()
        .filter(|(op, _)| midpoint(op))
        .collect();
    cusps::replay_cases(cases, (1, 0), None);
}
