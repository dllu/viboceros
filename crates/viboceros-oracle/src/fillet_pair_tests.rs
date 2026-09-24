use super::*;

#[test]
fn nonmeeting_arc_fillets_match_saved_rhino_samples() {
    let request: ProbeRequest = serde_json::from_str(include_str!(
        "../../../tools/rhino_oracle/fixtures/curve_fillet_nonmeeting.json"
    ))
    .unwrap();
    let observed: Value = serde_json::from_str(include_str!(
        "../../../tools/rhino_oracle/observations/curve_fillet_nonmeeting.json"
    ))
    .unwrap();
    let actual = run_request(&request).unwrap();
    assert_eq!(observed["engine"], "rhino");
    for (row, reference) in actual
        .results
        .iter()
        .zip(observed["results"].as_array().unwrap())
    {
        assert_eq!(row.id, reference["id"].as_str().unwrap());
        let length = row.value["length"].as_f64().unwrap();
        let expected = reference["value"]["length"].as_f64().unwrap();
        assert!((length - expected).abs() < 1e-10, "{}: length", row.id);
        let samples = row.value["samples"].as_array().unwrap();
        let reference_samples = reference["value"]["samples"].as_array().unwrap();
        assert_eq!(samples.len(), reference_samples.len());
        for (station, (sample, expected)) in samples.iter().zip(reference_samples).enumerate() {
            for coordinate in 0..3 {
                let actual = sample[coordinate].as_f64().unwrap();
                let expected = expected[coordinate].as_f64().unwrap();
                assert!(
                    (actual - expected).abs() < 1e-10,
                    "{}, station {station}, coordinate {coordinate}: {actual} != {expected}",
                    row.id
                );
            }
        }
    }
}

#[test]
fn joined_curve_fillets_match_saved_rhino_samples() {
    let request: ProbeRequest = serde_json::from_str(include_str!(
        "../../../tools/rhino_oracle/fixtures/curve_fillet_pair.json"
    ))
    .unwrap();
    let response = run_request(&request).unwrap();
    let observed: Value = serde_json::from_str(include_str!(
        "../../../tools/rhino_oracle/observations/curve_fillet_pair.json"
    ))
    .unwrap();
    assert_eq!(observed["engine"], "rhino");
    assert_eq!(response.results.len(), request.operations.len());
    assert_eq!(
        response.results.len(),
        observed["results"].as_array().unwrap().len()
    );
    for (row, reference) in response
        .results
        .into_iter()
        .zip(observed["results"].as_array().unwrap())
    {
        assert_eq!(row.id, reference["id"].as_str().unwrap());
        assert_eq!(row.value["closed"], reference["value"]["closed"]);
        let epsilon = if row.id.starts_with("nurbs-") {
            1e-7
        } else {
            1e-10
        };
        let actual_length = row.value["length"].as_f64().unwrap();
        let expected_length = reference["value"]["length"].as_f64().unwrap();
        assert!(
            (actual_length - expected_length).abs() < epsilon,
            "{}: length {actual_length} != {expected_length}",
            row.id
        );
        let samples = row.value["samples"].as_array().unwrap();
        let expected = reference["value"]["samples"].as_array().unwrap();
        assert_eq!(samples.len(), 65);
        assert_eq!(expected.len(), 65);
        for (station, (actual, observed)) in samples.iter().zip(expected).enumerate() {
            for coordinate in 0..3 {
                let actual = actual[coordinate].as_f64().unwrap();
                let observed = observed[coordinate].as_f64().unwrap();
                assert!(
                    (actual - observed).abs() < epsilon,
                    "{}, station {station}, coordinate {coordinate}: {actual} != {observed}",
                    row.id
                );
            }
        }
    }
}

#[test]
fn separate_curve_fillet_options_match_saved_rhino_samples() {
    let request: ProbeRequest = serde_json::from_str(include_str!(
        "../../../tools/rhino_oracle/fixtures/curve_fillet_pair_parts.json"
    ))
    .unwrap();
    let response = run_request(&request).unwrap();
    let observed: Value = serde_json::from_str(include_str!(
        "../../../tools/rhino_oracle/observations/curve_fillet_pair_parts.json"
    ))
    .unwrap();
    assert_eq!(observed["engine"], "rhino");
    for (row, reference) in response
        .results
        .into_iter()
        .zip(observed["results"].as_array().unwrap())
    {
        assert_eq!(row.id, reference["id"].as_str().unwrap());
        let parts = row.value["parts"].as_array().unwrap();
        let expected = reference["value"]["parts"].as_array().unwrap();
        assert_eq!(parts.len(), expected.len(), "{}", row.id);
        for (part_index, (part, reference)) in parts.iter().zip(expected).enumerate() {
            assert_eq!(part["closed"], reference["closed"]);
            let length = part["length"].as_f64().unwrap();
            let expected_length = reference["length"].as_f64().unwrap();
            assert!(
                (length - expected_length).abs() < 1e-10,
                "{}, part {part_index}",
                row.id
            );
            let samples = part["samples"].as_array().unwrap();
            let expected_samples = reference["samples"].as_array().unwrap();
            assert_eq!(samples.len(), 65);
            for (station, (sample, expected_sample)) in
                samples.iter().zip(expected_samples).enumerate()
            {
                for axis in 0..3 {
                    let actual = sample[axis].as_f64().unwrap();
                    let expected = expected_sample[axis].as_f64().unwrap();
                    assert!(
                        (actual - expected).abs() < 1e-10,
                        "{}, part {part_index}, station {station}, axis {axis}: {actual} != {expected}",
                        row.id
                    );
                }
            }
        }
    }
}
