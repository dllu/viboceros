use super::*;

fn source(name: &str) -> Brep {
    let Some(kind) = name.strip_prefix("cavity_") else {
        return compound::shape(name);
    };
    let outer = if kind == "contained_outer" {
        [[-0.5, 3.5]; 3]
    } else if kind == "equal_outer" {
        [[0., 3.]; 3]
    } else {
        [[0.5, 3.5]; 3]
    };
    let void = match kind {
        "crossing" | "contained_outer" | "equal_outer" => [[1.5, 2.5]; 3],
        "nested" => [[1.25, 1.75]; 3],
        "equal" => [[1., 2.]; 3],
        "face" => [[2., 2.5], [1., 2.], [1., 2.]],
        "edge" => [[2., 2.5], [2., 2.5], [1., 2.]],
        "point" => [[2., 2.5]; 3],
        "disjoint" => [[2.125, 2.625]; 3],
        "boundary_nested" => [[1.25, 2.]; 3],
        _ => panic!("unknown cavity {kind}"),
    };
    Brep::try_disjoint_union(
        vec![box_brep(outer), box_brep(void).reversed()],
        Tolerance::DEFAULT,
    )
    .unwrap()
}

fn inputs(case: &str) -> Vec<Brep> {
    match case {
        "first_multi" | "second_multi" => {
            return vec![
                source("cavity"),
                source("cavity_crossing"),
                source("extra_cross"),
            ];
        }
        "common_island" => return vec![source("island"), source("cross")],
        "common_three_cavities" => {
            return vec![
                source("cavity"),
                source("cavity_crossing"),
                source("contains_inner"),
            ];
        }
        "common_three_touch" => {
            return vec![
                source("cavity"),
                source("touches_inner"),
                source("enclosing"),
            ];
        }
        _ => {}
    }
    if let Some(kind) = case.strip_prefix("common_three_enclosing_") {
        return vec![
            source("cavity"),
            source(&format!(
                "cavity_{}",
                if kind == "first" { "crossing" } else { kind }
            )),
            source("enclosing"),
        ];
    }
    let mut bare = case;
    for suffix in ["_reverse", "_pre", "_keep"] {
        if let Some(s) = bare.strip_suffix(suffix) {
            bare = s;
        }
    }
    let other = if let Some(kind) = bare.strip_prefix("pair_") {
        format!("cavity_{kind}")
    } else if let Some(kind) = bare.strip_prefix("common_cavities_") {
        format!("cavity_{kind}")
    } else {
        bare.strip_prefix("common_").unwrap().to_owned()
    };
    vec![source("cavity"), source(&other)]
}

// This is an oriented boundary integral, not a material mass property. It also
// checks the retained inward winding of the native non-solid edge contacts.
fn oriented_volume(boundaries: &Boundary, reversed: &[bool]) -> f64 {
    let mut sum = 0.;
    for (face, &rev) in boundaries.iter().zip(reversed) {
        for ring in face {
            for i in 1..ring.len() - 1 {
                let (a, b, c) = (ring[0], ring[i], ring[i + 1]);
                let cross = [
                    b[1] * c[2] - b[2] * c[1],
                    b[2] * c[0] - b[0] * c[2],
                    b[0] * c[1] - b[1] * c[0],
                ];
                sum += (if rev { -1. } else { 1. })
                    * a.iter().zip(cross).map(|(a, b)| a * b).sum::<f64>()
                    / 6.;
            }
        }
    }
    sum
}

#[test]
fn replays_oriented_pairs_common_shell_participation_and_open_contacts() {
    let capture: Value = serde_json::from_str(include_str!(
        "../../../../../tools/rhino_oracle/observations/compound_pairs.json"
    ))
    .unwrap();
    let mut count = 0;
    let mut open = 0;
    let mut partitions = Vec::new();
    for row in capture["results"].as_array().unwrap() {
        let case = row["id"].as_str().unwrap().strip_prefix("pairs_").unwrap();
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
        let mut command = format!(
            "BooleanIntersection DeleteInput={} FirstSet={}",
            if case.ends_with("_keep") { "No" } else { "Yes" },
            set(&first)
        );
        if !second.is_empty() {
            command.push_str(&format!(" SecondSet={}", set(&second)));
        }
        let registry = CommandRegistry::with_builtins();
        let native = value["command"]["events"]
            .as_array()
            .unwrap()
            .iter()
            .find(|e| e["name"] == "BooleanIntersection")
            .unwrap()["result"]
            .as_str()
            .unwrap();
        if native != "Success" {
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
        if native == "Success" {
            assert!(result.is_ok(), "{case}: {result:?}");
        } else {
            assert!(result.is_err(), "{case}: expected {native}, got {result:?}");
            assert_eq!(doc.objects().cloned().collect::<Vec<_>>(), before);
            assert_eq!(doc.selected_object_ids().collect::<Vec<_>>(), selection);
            assert_eq!(doc.undo_label(), undo.as_deref());
            assert_eq!(doc.redo_label(), redo.as_deref());
        }
        let expected = &value["command"]["after"];
        partitions.extend(compare_physical(
            &doc, &ids, &layers, &groups, expected, case,
        ));
        let actual = snapshot(&doc, &ids, &layers, &groups);
        let permutation = match_rows(&actual, expected, case);
        let objects = doc.objects().collect::<Vec<_>>();
        for (g, &i) in expected.as_array().unwrap().iter().zip(&permutation) {
            let Geometry::Brep(b) = objects[i].geometry() else {
                panic!("B-rep")
            };
            let boundary = serde_json::from_value::<Boundary>(g["face_regions"].clone()).unwrap();
            let flipped = serde_json::from_value::<Vec<bool>>(g["face_reversed"].clone()).unwrap();
            let ours = oriented_volume(
                &regions(b),
                &b.faces()
                    .iter()
                    .map(|f| f.is_reversed())
                    .collect::<Vec<_>>(),
            );
            let captured = oriented_volume(&boundary, &flipped);
            assert!(
                (ours - captured).abs() < 1e-10,
                "{case}: boundary winding {ours} != {captured}"
            );
            if g["solid"] == false {
                open += 1;
                assert!(!b.is_solid());
                assert!(!b.is_manifold());
                assert!(b.edge_use_counts().contains(&4));
                assert!(g["volume"].is_null());
                assert!((captured + 1.25).abs() < 1e-10);
            }
        }
        if native != "Success" {
            registry.execute(&mut doc, "Redo").unwrap();
            assert_eq!(doc.objects().len(), ids.len() + 1);
        }
        if !value["undo"].is_null() {
            for _ in 0..2 {
                registry.execute(&mut doc, "Undo").unwrap();
                compare_physical(&doc, &ids, &layers, &groups, &value["undo"]["after"], case);
                registry.execute(&mut doc, "Redo").unwrap();
                compare_physical(&doc, &ids, &layers, &groups, &value["redo"]["after"], case);
            }
        }
        count += 1;
    }
    assert_eq!((count, open), (46, 2));
    let recorded: Vec<Value> = serde_json::from_str(include_str!(
        "../../../../../docs/compound-pairs-partitions.json"
    ))
    .unwrap();
    assert_eq!(partitions, recorded, "unrecorded native seam difference");
}
