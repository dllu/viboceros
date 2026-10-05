use super::*;

// Explicit public-box recipes, independently constructed from the probe's
// shape definitions. Captured result coordinates never construct local inputs.
pub(super) fn shape(name: &str) -> Brep {
    let pieces = match name {
        "cavity" => vec![box_brep([[0., 3.]; 3]), box_brep([[1., 2.]; 3]).reversed()],
        "island" => vec![
            box_brep([[0., 4.]; 3]),
            box_brep([[1., 3.]; 3]).reversed(),
            box_brep([[1.5, 2.5]; 3]),
        ],
        "other_cavity" => vec![
            box_brep([[1.5, 3.5]; 3]),
            box_brep([[2.125, 2.625]; 3]).reversed(),
        ],
        _ => vec![box_brep(match name {
            "cross" => [[1.5, 3.5], [1.5, 3.5], [1., 2.]],
            "unopened" => [[2.5, 3.5], [2.5, 3.5], [1., 2.]],
            "contains_inner" => [[0.75, 2.25]; 3],
            "inside_inner" => [[1.25, 1.75]; 3],
            "equal_inner" => [[1., 2.]; 3],
            "enclosing" => [[-1., 4.]; 3],
            "boundary_inner" => [[1.25, 2.]; 3],
            "touches_inner" => [[2., 2.5], [1., 2.], [1., 2.]],
            "equal_outer" => [[0., 3.]; 3],
            "disjoint" => [[10., 11.]; 3],
            "extra_cross" => [[2.5, 4.], [1.75, 2.75], [1., 2.]],
            "extra_disjoint_cross" => [[0.5, 1.5], [0.5, 1.5], [1., 2.]],
            _ => panic!("unknown source {name}"),
        })],
    };
    Brep::try_disjoint_union(pieces, Tolerance::DEFAULT).unwrap()
}

fn inputs(case: &str) -> Vec<Brep> {
    if case.starts_with("pair_") {
        let other = if case.starts_with("pair_two_cavities") {
            "other_cavity"
        } else {
            case.strip_prefix("pair_").unwrap()
        };
        return vec![shape("cavity"), shape(other)];
    }
    if case == "island_second_contains" {
        return vec![shape("island"), shape("cross"), shape("contains_inner")];
    }
    let mut other = case.split_once('_').unwrap().1;
    for suffix in ["_reverse", "_pre", "_keep"] {
        if let Some(value) = other.strip_suffix(suffix) {
            other = value;
        }
    }
    vec![shape("cavity"), shape("cross"), shape(other)]
}

#[test]
fn replays_compound_sets_common_intersection_metadata_and_history() {
    let capture: Value = serde_json::from_str(include_str!(
        "../../../../../tools/rhino_oracle/observations/compound_intersection.json"
    ))
    .unwrap();
    let mut matched = 0;
    let mut failures = 0;
    let mut partitions = Vec::new();
    for row in capture["results"].as_array().unwrap() {
        let case = row["id"]
            .as_str()
            .unwrap()
            .strip_prefix("compound_")
            .unwrap();
        if case.starts_with("sdk_") {
            continue;
        }
        let value = &row["value"];
        let (mut doc, ids, layers, groups) = setup_sources(inputs(case));
        assert!(compare_physical(&doc, &ids, &layers, &groups, &value["before"], case).is_empty());
        let first = serde_json::from_value::<Vec<usize>>(value["first"].clone()).unwrap();
        let second = serde_json::from_value::<Vec<usize>>(value["second"].clone()).unwrap();
        let pre = case.ends_with("_pre");
        let picks = if pre {
            first.clone()
        } else {
            (0..ids.len()).collect()
        };
        doc.select_objects_direct(picks.iter().map(|&i| ids[i]), SelectionMode::Add)
            .unwrap();
        let set = |indices: &[usize]| {
            indices
                .iter()
                .map(|&i| ids[i].to_string())
                .collect::<Vec<_>>()
                .join(",")
        };
        let command = if second.is_empty() {
            "BooleanIntersection".to_owned()
        } else {
            format!(
                "BooleanIntersection DeleteInput={} FirstSet={} SecondSet={}",
                if case.ends_with("_keep") { "No" } else { "Yes" },
                set(&first),
                set(&second)
            )
        };
        let native_result = value["command"]["events"]
            .as_array()
            .unwrap()
            .iter()
            .find(|e| e["name"] == "BooleanIntersection")
            .unwrap()["result"]
            .as_str()
            .unwrap();
        let registry = CommandRegistry::with_builtins();
        if native_result != "Success" {
            registry.execute(&mut doc, "Point 100,100,100").unwrap();
            registry.execute(&mut doc, "Undo").unwrap();
        }
        let before = doc.objects().cloned().collect::<Vec<_>>();
        let selection = doc.selected_object_ids().collect::<Vec<_>>();
        let undo = doc.undo_label().map(str::to_owned);
        let redo = doc.redo_label().map(str::to_owned);
        let result = if pre {
            registry.execute(&mut doc, &command)
        } else {
            registry.execute_postselected(&mut doc, &command, Default::default())
        };
        let expected = &value["command"]["after"];
        if native_result == "Success" {
            assert!(result.is_ok(), "{case}: {result:?}");
            partitions.extend(compare_physical(
                &doc, &ids, &layers, &groups, expected, case,
            ));
        } else {
            assert!(
                result.is_err(),
                "{case}: expected native {native_result}, got {result:?}"
            );
            assert_eq!(doc.objects().cloned().collect::<Vec<_>>(), before, "{case}");
            assert_eq!(
                doc.selected_object_ids().collect::<Vec<_>>(),
                selection,
                "{case}"
            );
            assert_eq!(doc.undo_label(), undo.as_deref());
            assert_eq!(doc.redo_label(), redo.as_deref());
            let expected = if case == "second_extra_disjoint_cross" {
                let dots = expected
                    .as_array()
                    .unwrap()
                    .iter()
                    .filter(|g| g["kind"] == "TextDot")
                    .collect::<Vec<_>>();
                assert_eq!(dots.len(), 2);
                assert!(dots.iter().all(|g| g["text"] == "!"));
                failures += 1;
                Value::Array(
                    expected
                        .as_array()
                        .unwrap()
                        .iter()
                        .filter(|g| g["kind"] == "Brep")
                        .cloned()
                        .collect(),
                )
            } else {
                expected.clone()
            };
            assert!(compare_physical(&doc, &ids, &layers, &groups, &expected, case).is_empty());
            registry.execute(&mut doc, "Redo").unwrap();
            assert_eq!(doc.objects().len(), ids.len() + 1);
        }
        if case != "second_extra_disjoint_cross" {
            matched += 1;
        }
        if !value["undo"].is_null() {
            for _ in 0..2 {
                registry.execute(&mut doc, "Undo").unwrap();
                compare_physical(&doc, &ids, &layers, &groups, &value["undo"]["after"], case);
                registry.execute(&mut doc, "Redo").unwrap();
                compare_physical(&doc, &ids, &layers, &groups, &value["redo"]["after"], case);
            }
        }
    }
    assert_eq!((matched, failures), (31, 1));
    let expected: Vec<Value> = serde_json::from_str(include_str!(
        "../../../../../docs/compound-intersection-partitions.json"
    ))
    .unwrap();
    assert_eq!(partitions, expected, "unrecorded native seam difference");
}

#[test]
fn inconsistent_raw_compound_orientation_is_rejected_during_staging() {
    // Document insertion normalizes a wholly reversed solid. Test the raw
    // staging inputs directly so that normalization cannot hide this guard.
    let a = shape("cavity").reversed();
    let b = shape("cross");
    let before = (a.clone(), b.clone());
    let result = compound_intersection(&[&a, &b], Tolerance::DEFAULT, false, 1, Kernel::Polyhedral);
    assert!(
        matches!(
            &result,
            Err(GeometryError::UnsupportedPolyhedralBrepBoolean { .. })
        ),
        "raw orientation must be rejected"
    );
    assert_eq!((a, b), before);
}
