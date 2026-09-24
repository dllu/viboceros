use super::*;

#[test]
fn fillet_corners_geometry_matches_saved_rhino_samples() {
    let request: ProbeRequest = serde_json::from_str(include_str!(
        "../../../tools/rhino_oracle/fixtures/curve_fillet_corners.json"
    ))
    .unwrap();
    let response = run_request(&request).unwrap();
    let observed: Value = serde_json::from_str(include_str!(
        "../../../tools/rhino_oracle/observations/curve_fillet_corners.json"
    ))
    .unwrap();
    assert_eq!(observed["engine"], "rhino");
    assert_eq!(response.results.len(), 7);
    for (row, reference) in response
        .results
        .into_iter()
        .zip(observed["results"].as_array().unwrap())
    {
        assert_eq!(row.id, reference["id"].as_str().unwrap());
        assert_eq!(row.value["closed"], reference["value"]["closed"]);
        let actual_length = row.value["length"].as_f64().unwrap();
        let expected_length = reference["value"]["length"].as_f64().unwrap();
        assert!((actual_length - expected_length).abs() < 1e-10);
        let samples = row.value["samples"].as_array().unwrap();
        let expected = reference["value"]["samples"].as_array().unwrap();
        assert_eq!(samples.len(), 65);
        assert_eq!(expected.len(), 65);
        for (station, (actual, observed)) in samples.iter().zip(expected).enumerate() {
            for coordinate in 0..3 {
                let actual = actual[coordinate].as_f64().unwrap();
                let observed = observed[coordinate].as_f64().unwrap();
                assert!(
                    (actual - observed).abs() < 1e-10,
                    "{}, station {station}, coordinate {coordinate}: {actual} != {observed}",
                    row.id
                );
            }
        }
    }
}
