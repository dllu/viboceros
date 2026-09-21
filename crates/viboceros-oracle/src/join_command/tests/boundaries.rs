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
fn remaining_zero_volume_orientation_and_unjoined_gap_rebuilding_are_explicit() {
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
        let gap: f64 =
            a.id.strip_prefix("near-competition-")
                .unwrap()
                .split('-')
                .next()
                .unwrap()
                .parse()
                .unwrap();
        assert_eq!(a.value["succeeded"], b["succeeded"]);
        let (native, rhino) = (
            a.value["objects"].as_array().unwrap(),
            b["objects"].as_array().unwrap(),
        );
        assert_eq!(native.len(), rhino.len());
        let mut outputs = 0;
        for (n, r) in native.iter().zip(rhino) {
            if !n["source"].is_null() {
                compare(n, r, &a.id);
                continue;
            }
            outputs += 1;
            for field in ["source", "selected", "name", "layer", "color", "groups"] {
                compare(&n[field], &r[field], &a.id);
            }
            for field in n["brep"].as_object().unwrap().keys() {
                if !["vertices", "edges", "vertex_tolerances", "edge_tolerances"]
                    .contains(&field.as_str())
                {
                    compare(&n["brep"][field], &r["brep"][field], &a.id);
                }
            }
            assert_eq!(n["brep"]["surfaces"].as_array().unwrap().len(), 1);
            compare(&r["brep"]["vertices"][0][2], &json!(gap / 3.), &a.id);
            assert_ne!(n["brep"]["vertices"], r["brep"]["vertices"]);
            assert_ne!(n["brep"]["edges"], r["brep"]["edges"]);
        }
        assert_eq!(outputs, 3);
    }
}
