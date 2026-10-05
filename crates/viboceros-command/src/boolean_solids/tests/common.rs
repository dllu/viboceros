//! Independent public-box construction and full common-command replay.
use super::*;

fn shape(name: &str) -> Brep {
    box_brep(match name {
        "a" => [[0., 2.]; 3],
        "b" => [[1., 3.]; 3],
        "enclosing" => [[-1., 4.]; 3],
        "enclosing_again" => [[-2., 5.]; 3],
        "inside" => [[1.2, 1.8]; 3],
        "boundary_inside" => [[1.5, 2.]; 3],
        "touch" => [[1., 2.], [0.25, 0.75], [0.25, 0.75]],
        "face" => [[2., 3.], [1., 2.], [1., 2.]],
        "point" => [[2., 3.]; 3],
        "disjoint" => [[10., 11.]; 3],
        "shared_plane" => [[-1., 2.], [-1., 4.], [-1., 4.]],
        _ => panic!("unknown common source {name}"),
    })
}

fn inputs(case: &str) -> Vec<Brep> {
    let bare = case
        .strip_suffix("_pre")
        .or_else(|| case.strip_suffix("_keep"))
        .unwrap_or(case);
    let (kind, _) = bare.rsplit_once('_').unwrap();
    if let Some(other) = kind.strip_prefix("two_") {
        return vec![shape("a"), shape(other)];
    }
    if kind == "double" {
        return ["a", "b", "enclosing", "enclosing_again"].map(shape).into();
    }
    if kind == "four_inside" {
        return ["a", "b", "inside", "enclosing"].map(shape).into();
    }
    vec![
        shape("a"),
        shape("b"),
        shape(match kind {
            "duplicate_a" => "a",
            "duplicate_b" => "b",
            other => other,
        }),
    ]
}

#[test]
fn replays_common_intersection_participation_order_metadata_geometry_and_history() {
    let capture: Value = serde_json::from_str(include_str!(
        "../../../../../tools/rhino_oracle/observations/common_participation.json"
    ))
    .unwrap();
    let mut records = 0;
    let mut partitions = Vec::new();
    for row in capture["results"].as_array().unwrap() {
        let case = row["id"].as_str().unwrap().strip_prefix("common_").unwrap();
        let value = &row["value"];
        let (mut doc, ids, layers, groups) = setup_sources(inputs(case));
        assert!(compare_physical(&doc, &ids, &layers, &groups, &value["before"], case).is_empty());
        let order = serde_json::from_value::<Vec<usize>>(value["first"].clone()).unwrap();
        let pre = case.ends_with("_pre");
        for &i in &order {
            doc.select_objects_direct([ids[i]], SelectionMode::Add)
                .unwrap();
        }
        let set = order
            .iter()
            .map(|&i| ids[i].to_string())
            .collect::<Vec<_>>()
            .join(",");
        let command = format!(
            "BooleanIntersection DeleteInput={} FirstSet={set}",
            if case.ends_with("_keep") { "No" } else { "Yes" }
        );
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
        partitions.extend(compare_physical(
            &doc,
            &ids,
            &layers,
            &groups,
            &value["command"]["after"],
            case,
        ));
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
        records += 1;
    }
    assert_eq!(records, 80);
    assert!(
        partitions.is_empty(),
        "unrecorded common partition difference: {partitions:?}"
    );
}
