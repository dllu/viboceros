//! Exact replay of recorded Rhino mesh edge and vertex edits.
use super::*;

#[test]
fn permanent_angle_unweld_fixture_matches_recorded_rhino_geometry_and_order() {
    assert_recorded_geometry(
        include_str!("../../../tools/rhino_oracle/fixtures/mesh_unweld.json"),
        include_str!("../../../tools/rhino_oracle/observations/mesh_unweld.json"),
        6,
    );
}

#[test]
#[ignore = "known Rhino non-manifold angle-unweld face-order mismatch; see docs/mesh-unweld-nonmanifold.md"]
fn nonmanifold_angle_unweld_matches_recorded_rhino_geometry_and_order() {
    assert_recorded_geometry(
        include_str!("../../../tools/rhino_oracle/fixtures/mesh_unweld_nonmanifold.json"),
        include_str!("../../../tools/rhino_oracle/observations/mesh_unweld_nonmanifold.json"),
        36,
    );
}

#[test]
#[ignore = "known Rhino non-manifold angle-unweld threshold mismatch; see docs/mesh-unweld-nonmanifold.md"]
fn nonmanifold_angle_unweld_thresholds_match_recorded_rhino_geometry_and_order() {
    assert_recorded_geometry(
        include_str!(
            "../../../tools/rhino_oracle/fixtures/mesh_unweld_nonmanifold_thresholds.json"
        ),
        include_str!(
            "../../../tools/rhino_oracle/observations/mesh_unweld_nonmanifold_thresholds.json"
        ),
        3,
    );
}

#[test]
fn permanent_edge_unweld_fixture_matches_recorded_rhino_geometry_and_partitions() {
    assert_recorded_geometry(
        include_str!("../../../tools/rhino_oracle/fixtures/mesh_unweld_edge.json"),
        include_str!("../../../tools/rhino_oracle/observations/mesh_unweld_edge.json"),
        19,
    );
}

#[test]
fn permanent_vertex_unweld_fixture_matches_recorded_rhino_geometry_and_partitions() {
    assert_recorded_geometry(
        include_str!("../../../tools/rhino_oracle/fixtures/mesh_unweld_vertex.json"),
        include_str!("../../../tools/rhino_oracle/observations/mesh_unweld_vertex.json"),
        10,
    );
}

#[test]
fn permanent_vertex_weld_fixture_matches_recorded_rhino_geometry_and_partitions() {
    assert_recorded_geometry(
        include_str!("../../../tools/rhino_oracle/fixtures/mesh_weld_vertex.json"),
        include_str!("../../../tools/rhino_oracle/observations/mesh_weld_vertex.json"),
        10,
    );
}

#[test]
fn permanent_edge_weld_fixture_matches_recorded_rhino_geometry_and_partitions() {
    assert_recorded_geometry(
        include_str!("../../../tools/rhino_oracle/fixtures/mesh_weld_edge.json"),
        include_str!("../../../tools/rhino_oracle/observations/mesh_weld_edge.json"),
        9,
    );
}

#[test]
fn permanent_edge_split_fixture_matches_recorded_rhino_geometry_and_order() {
    assert_recorded_geometry(
        include_str!("../../../tools/rhino_oracle/fixtures/mesh_split_edge.json"),
        include_str!("../../../tools/rhino_oracle/observations/mesh_split_edge.json"),
        27,
    );
}

#[test]
fn permanent_edge_collapse_fixture_matches_recorded_rhino_geometry_and_order() {
    assert_recorded_geometry(
        include_str!("../../../tools/rhino_oracle/fixtures/mesh_collapse_edge.json"),
        include_str!("../../../tools/rhino_oracle/observations/mesh_collapse_edge.json"),
        15,
    );
}

fn assert_recorded_geometry(request: &str, recorded: &str, expected_count: usize) {
    let request: ProbeRequest = serde_json::from_str(request).unwrap();
    let recorded: ProbeResponse = serde_json::from_str(recorded).unwrap();
    assert_eq!(recorded.engine, "rhino");
    assert_eq!(recorded.error, None);
    assert_eq!(recorded.protocol_version, request.protocol_version);
    assert_eq!(recorded.iterations, request.iterations);
    let native = run_request(&request).unwrap();
    assert_eq!(native.results.len(), expected_count);
    assert_eq!(native.results.len(), recorded.results.len());
    for (native, recorded) in native.results.iter().zip(&recorded.results) {
        assert_eq!(native.id, recorded.id);
        assert_eq!(native.value, recorded.value, "{}", native.id);
    }
}
