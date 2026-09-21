use super::*;

#[test]
fn boundary_matching_replays_partial_crossing_reversed_and_competing_inputs() {
    let (actual, expected) = surfaces::replay(
        include_str!("../../../../../tools/rhino_oracle/fixtures/join_boundary_matching.json"),
        include_str!("../../../../../tools/rhino_oracle/observations/join_boundary_matching.json"),
        142,
    );
    for (a, b) in actual
        .results
        .iter()
        .zip(expected["results"].as_array().unwrap())
    {
        compare(&a.value, &b["value"], &a.id);
    }
}

pub(super) fn compare_zero_volume_face_senses(a: &Value, b: &Value, id: &str) {
    let (mut a, b) = (a.clone(), b.clone());
    let original = a["objects"].as_array_mut().unwrap();
    let expected = b["objects"].as_array().unwrap();
    assert_eq!(original.len(), expected.len());
    let mut different = 0;
    for (n, r) in original.iter_mut().zip(expected) {
        if n["brep"]["face_reversed"] == r["brep"]["face_reversed"] {
            continue;
        }
        assert_eq!(n["brep"]["volume"], json!(0.));
        assert_eq!(r["brep"]["volume"], json!(0.));
        assert_eq!(n["brep"]["surfaces"].as_array().unwrap().len(), 2);
        for (n, r) in n["brep"]["face_reversed"]
            .as_array()
            .unwrap()
            .iter()
            .zip(r["brep"]["face_reversed"].as_array().unwrap())
        {
            assert_eq!(n.as_bool().unwrap(), !r.as_bool().unwrap());
        }
        // Assert the specific discrepancy before comparing every remaining
        // recorded field. Raw observations are never rewritten or normalized.
        n["brep"]["face_reversed"] = r["brep"]["face_reversed"].clone();
        different += 1;
    }
    assert_eq!(different, 1, "{id}");
    compare(&a, &b, id);
}

#[test]
fn zero_volume_orientation_remains_explicit_and_unjoined_gap_rebuilding_matches() {
    let (actual, expected) = surfaces::replay(
        include_str!("../../../../../tools/rhino_oracle/fixtures/join_boundary_differences.json"),
        include_str!(
            "../../../../../tools/rhino_oracle/observations/join_boundary_differences.json"
        ),
        18,
    );
    for (a, b) in actual
        .results
        .iter()
        .zip(expected["results"].as_array().unwrap())
    {
        let b = &b["value"];
        if a.id.starts_with("duplicate-") {
            compare_zero_volume_face_senses(&a.value, b, &a.id);
            continue;
        }
        assert!(a.id.starts_with("near-competition-"));
        compare(&a.value, b, &a.id);
    }
}
