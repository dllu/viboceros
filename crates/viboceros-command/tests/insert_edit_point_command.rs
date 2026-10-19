use serde_json::Value;
use viboceros_command::{CommandRegistry, insert_edit_point::Draft, open_3dm_with_views};
use viboceros_document::{ObjectId, SelectionMode};
use viboceros_geometry::Point3;
#[path = "support/insert_kink.rs"]
mod comparison;
#[allow(dead_code)]
#[path = "support/simplify_curves.rs"]
mod curves;
use curves as native_curve_comparison;

#[test]
fn insert_edit_point_native_geometry_metadata_selection_and_history() {
    matrix("command", 152);
    matrix("limits", 48);
    matrix("projection", 16);
}
fn matrix(matrix: &str, count: usize) {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tools/rhino_oracle");
    let r: Value = serde_json::from_str(
        &std::fs::read_to_string(
            root.join(format!("observations/insert_edit_point_{matrix}.json")),
        )
        .unwrap(),
    )
    .unwrap();
    let rows = r["results"].as_array().unwrap();
    assert_eq!(rows.len(), count);
    let mut failures = Vec::new();
    for row in rows {
        let name = row["id"].as_str().unwrap();
        let v = &row["value"];
        let recipe = &v["recipe"];
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            let mut doc = open_3dm_with_views(
                root.join(format!("fixtures/insert_edit_point_{matrix}/{name}.3dm"))
                    .to_str()
                    .unwrap(),
            )
            .unwrap()
            .0;
            let id: ObjectId = v["source_id"].as_str().unwrap().parse().unwrap();
            doc.clear_selection();
            if v["before"][0]["selected"] == true {
                doc.select_object(id, SelectionMode::Replace).unwrap();
            }
            doc.clear_history().unwrap();
            assert!(
                comparison::check(&doc, &v["before"], id, name) <= 1e-8,
                "{name}: imported source"
            );
            let before = doc.objects().cloned().collect::<Vec<_>>();
            let range = recipe["subcurve"].as_array().map(|r| {
                let c = doc.object(id).unwrap().geometry().curve_ref().unwrap();
                [
                    c.parameter_at(r[0].as_f64().unwrap()).unwrap(),
                    c.parameter_at(r[1].as_f64().unwrap()).unwrap(),
                ]
            });
            let mut draft = Draft::new(&doc, id, range).unwrap();
            for p in v["picks"].as_array().unwrap() {
                draft
                    .point(
                        Point3::try_from(serde_json::from_value::<[f64; 3]>(p.clone()).unwrap())
                            .unwrap(),
                    )
                    .unwrap();
            }
            assert_eq!(
                doc.objects().cloned().collect::<Vec<_>>(),
                before,
                "{name}: preview mutation"
            );
            if recipe["cancel"] != true {
                doc.select_object(id, SelectionMode::Replace).unwrap();
                let sub = range
                    .map(|[a, b]| format!("SubCrv={id},{a},{b} "))
                    .unwrap_or_default();
                let points = v["tokens"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .filter_map(Value::as_str)
                    .filter(|s| !matches!(*s, "_Enter" | "!"))
                    .collect::<Vec<_>>()
                    .join(" ");
                let command = format!("InsertEditPoint Points {sub}{points}");
                let registry = CommandRegistry::with_builtins();
                if recipe["preselect"] == false || range.is_some() {
                    registry
                        .execute_postselected(&mut doc, &command, Default::default())
                        .unwrap();
                } else {
                    registry.execute(&mut doc, &command).unwrap();
                }
                if range.is_none() {
                    assert_eq!(
                        doc.object(id).unwrap().geometry(),
                        draft.geometry(),
                        "{name}: draft and API differ"
                    );
                }
            }
            assert_eq!(
                doc.object(id).unwrap().attributes(),
                before[0].attributes(),
                "{name}: attributes"
            );
            assert_eq!(
                doc.object(id).unwrap().group_ids(),
                before[0].group_ids(),
                "{name}: groups"
            );
            let mut gap =
                comparison::check_with_weight_tolerance(&doc, &v["after"], id, name, 1e-12);
            if !v["undo"].is_null() {
                assert_eq!(doc.undo_label(), Some("InsertEditPoint"));
                let registry = CommandRegistry::with_builtins();
                registry.execute(&mut doc, "Undo").unwrap();
                gap = gap.max(comparison::check_with_weight_tolerance(
                    &doc, &v["undo"], id, name, 1e-12,
                ));
                registry.execute(&mut doc, "Redo").unwrap();
                gap = gap.max(comparison::check_with_weight_tolerance(
                    &doc, &v["redo"], id, name, 1e-12,
                ));
            }
            assert!(gap <= 1e-8, "{name}: maximum difference {gap}");
        }));
        if result.is_err() {
            failures.push(name);
        }
    }
    assert!(
        failures.is_empty(),
        "InsertEditPoint differences: {failures:?}"
    );
}

#[test]
fn insert_edit_point_invalid_batches_and_stale_drafts_preserve_selection_and_redo() {
    let registry = CommandRegistry::with_builtins();
    let mut doc = viboceros_document::Document::default();
    registry
        .execute(&mut doc, "Curve Degree=3 0,0,0 1,3,2 3,-1,-2 5,0,1")
        .unwrap();
    let id = doc.objects().next().unwrap().id();
    doc.select_object(id, SelectionMode::Replace).unwrap();
    doc.clear_history().unwrap();
    registry.execute(&mut doc, "SetObjectName Future").unwrap();
    registry.execute(&mut doc, "Undo").unwrap();
    let before = format!("{doc:?}");
    for command in [
        "InsertEditPoint 1,2,3 nan,0,0",
        "InsertEditPoint Midpoint=Yes",
        "InsertEditPoint 1,2,3 bad",
        "InsertEditPoint SubCrv=bad,0,1 1,2,3",
    ] {
        assert!(registry.execute(&mut doc, command).is_err(), "{command}");
        assert_eq!(format!("{doc:?}"), before, "{command}");
    }
    let draft = Draft::new(&doc, id, None).unwrap();
    doc.replace_object_geometries([(
        id,
        viboceros_document::Geometry::Point(Point3::try_new(7., 0., 0.).unwrap()),
    )])
    .unwrap();
    let before = format!("{doc:?}");
    assert!(draft.commit(&mut doc, true).is_err());
    assert_eq!(format!("{doc:?}"), before);
}

#[path = "support/insert_edit_point.rs"]
mod history;
use history::{close_json, history_snapshot};
#[test]
fn insert_edit_point_native_empty_existing_cancel_and_redo_witnesses() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tools/rhino_oracle");
    let q: Value = serde_json::from_str(
        &std::fs::read_to_string(root.join("fixtures/insert_edit_point_history.json")).unwrap(),
    )
    .unwrap();
    let r: Value = serde_json::from_str(
        &std::fs::read_to_string(root.join("observations/insert_edit_point_history.json")).unwrap(),
    )
    .unwrap();
    assert_eq!(r["results"].as_array().unwrap().len(), 24);
    for (op, row) in q["operations"]
        .as_array()
        .unwrap()
        .iter()
        .zip(r["results"].as_array().unwrap())
    {
        assert_eq!(op["id"], row["id"]);
        let id_string = op["id"].as_str().unwrap();
        let value = &row["value"];
        let (mut doc, id, marker) = history::history_document(op);
        let registry = CommandRegistry::with_builtins();
        close_json(
            &history_snapshot(&doc, id, marker),
            &value["before"],
            id_string,
        );
        for step in value["steps"].as_array().unwrap() {
            close_json(
                &history_snapshot(&doc, id, marker),
                &step["before"],
                id_string,
            );
            if step["command"] == "InsertEditPoint" {
                if op["cancel"] != true {
                    let point = match op["station"].as_str().unwrap() {
                        "empty" => String::new(),
                        station => {
                            let c = doc.object(id).unwrap().geometry().curve_ref().unwrap();
                            let p = c
                                .evaluate(if station == "existing" { 1. / 3. } else { 0.35 })
                                .unwrap()
                                .to_array();
                            format!("{}, {}, {}", p[0], p[1], p[2])
                        }
                    };
                    let point = point.replace(' ', "");
                    let command = format!("InsertEditPoint {point}");
                    doc.select_object(id, SelectionMode::Replace).unwrap();
                    if op["preselect"] == true {
                        registry.execute(&mut doc, &command).unwrap();
                    } else {
                        registry
                            .execute_postselected(&mut doc, &command, Default::default())
                            .unwrap();
                    }
                }
            } else {
                registry
                    .execute(&mut doc, step["command"].as_str().unwrap())
                    .unwrap();
            }
            close_json(
                &history_snapshot(&doc, id, marker),
                &step["after"],
                id_string,
            );
            close_json(
                &history_snapshot(&doc, id, marker),
                &step["after_script"],
                id_string,
            );
        }
    }
}

#[test]
fn native_empty_undo_preserves_ordinary_curve_and_point_selection() {
    use viboceros_document::{Document, Geometry};
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tools/rhino_oracle");
    let q: Value = serde_json::from_str(
        &std::fs::read_to_string(root.join("fixtures/empty_undo_selection.json")).unwrap(),
    )
    .unwrap();
    let r: Value = serde_json::from_str(
        &std::fs::read_to_string(root.join("observations/empty_undo_selection.json")).unwrap(),
    )
    .unwrap();
    assert_eq!(r["results"].as_array().unwrap().len(), 4);
    for (op, row) in q["operations"]
        .as_array()
        .unwrap()
        .iter()
        .zip(r["results"].as_array().unwrap())
    {
        let mut doc = Document::default();
        let registry = CommandRegistry::with_builtins();
        registry
            .execute(
                &mut doc,
                if op["kind"] == "point" {
                    "Point 0,0,0"
                } else {
                    "Line 0,0,0 5,0,0"
                },
            )
            .unwrap();
        let id = doc.objects().next().unwrap().id();
        doc.clear_selection();
        doc.clear_history().unwrap();
        let marker = doc
            .add_geometry(Geometry::Point(Point3::try_new(30., 30., 30.).unwrap()))
            .unwrap();
        doc.undo().unwrap();
        if op["redo"] == false {
            doc.clear_history().unwrap();
        }
        doc.select_object(id, SelectionMode::Replace).unwrap();
        let snapshot = |doc: &Document| serde_json::json!({"selected":doc.is_selected(id),"marker":doc.object(marker).is_some()});
        let name = op["id"].as_str().unwrap();
        history::close_json(&snapshot(&doc), &row["value"]["before"], name);
        for step in row["value"]["steps"].as_array().unwrap() {
            history::close_json(&snapshot(&doc), &step["before"], name);
            registry
                .execute(&mut doc, step["command"].as_str().unwrap())
                .unwrap();
            history::close_json(&snapshot(&doc), &step["after"], name);
            history::close_json(&snapshot(&doc), &step["after_script"], name);
        }
    }
}
