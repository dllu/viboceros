//! Replay complete surface definitions from actual Rhino ExtrudeCrv commands.
use super::*;

#[test]
fn numeric_curve_extrusion_matches_recorded_rhino_directions_and_raw_surfaces() {
    let request: ProbeRequest = serde_json::from_str(include_str!(
        "../../../tools/rhino_oracle/fixtures/curve_extrusion_directions.json"
    ))
    .unwrap();
    let recorded: ProbeResponse = serde_json::from_str(include_str!(
        "../../../tools/rhino_oracle/observations/curve_extrusion_directions.json"
    ))
    .unwrap();
    assert_eq!(recorded.engine, "rhino");
    assert_eq!(recorded.results.len(), 27);
    let actual = run_request(&request).unwrap();
    assert_eq!(actual.results.len(), recorded.results.len());
    for (actual, recorded) in actual.results.iter().zip(&recorded.results) {
        assert_eq!(actual.id, recorded.id);
        crate::test_json::close(&actual.value, &recorded.value, &actual.id, 1e-10, 1e-12);
    }
}

#[cfg(feature = "native-smlib")]
#[test]
fn short_span_extrusion_matches_recorded_rhino_basis_and_direction() {
    let mut request: ProbeRequest = serde_json::from_str(include_str!(
        "../../../tools/rhino_oracle/fixtures/curve_modeling_short_spans.json"
    ))
    .unwrap();
    request
        .operations
        .retain(|op| matches!(op, Operation::CurveExtrudeCommand { .. }));
    let recorded: ProbeResponse = serde_json::from_str(include_str!(
        "../../../tools/rhino_oracle/observations/curve_modeling_short_spans.json"
    ))
    .unwrap();
    let expected = recorded
        .results
        .iter()
        .find(|result| result.id == "short-span-extrusion")
        .unwrap();
    let actual = run_request(&request).unwrap();
    assert_eq!(actual.results.len(), 1);
    assert_eq!(actual.results[0].id, expected.id);
    crate::test_json::close(
        &actual.results[0].value,
        &expected.value,
        &expected.id,
        1e-10,
        1e-12,
    );
}
