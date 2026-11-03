use super::*;

#[test]
fn multiedge_sdk_matches_native_continuity_and_cusp_trimming() {
    cusps::replay_family("inset_multiedge_sdk", (28, 12), None);
}
#[cfg(feature = "native-smlib")]
#[test]
fn multiedge_sdk_with_usd_brep_matches_native_continuity_and_cusp_trimming() {
    cusps::replay_family(
        "inset_multiedge_sdk",
        (28, 12),
        Some(viboceros_smlib::curve_offset_operations()),
    );
}
#[test]
fn multiedge_commands_match_native_geometry_metadata_history_and_3dm() {
    cusps::replay_commands("inset_multiedge_command", (78, 36));
}
