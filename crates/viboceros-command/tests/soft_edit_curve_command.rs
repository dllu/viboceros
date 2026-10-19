use serde_json::Value;
use viboceros_command::{
    CommandRegistry, open_3dm_with_views,
    soft_edit_curve::{Draft, Options},
};
use viboceros_document::{ObjectId, SelectionMode};
use viboceros_geometry::Point3;
#[path = "support/insert_kink.rs"]
mod comparison;
#[allow(dead_code)]
#[path = "support/simplify_curves.rs"]
mod native_curve_comparison;
#[path = "support/soft_edit_curve.rs"]
mod soft_comparison;
fn point(v: &Value) -> Point3 {
    Point3::try_from(serde_json::from_value::<[f64; 3]>(v.clone()).unwrap()).unwrap()
}
fn format_point(p: Point3) -> String {
    let [x, y, z] = p.to_array();
    format!("{x},{y},{z}")
}
fn root() -> std::path::PathBuf {
    std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tools/rhino_oracle")
}

#[test]
fn native_soft_edit_curve_geometry_metadata_selection_and_history() {
    let root = root();
    let r: Value = serde_json::from_str(
        &std::fs::read_to_string(root.join("observations/soft_edit_curve_command.json")).unwrap(),
    )
    .unwrap();
    let rows = r["results"].as_array().unwrap();
    let sdk: Value = serde_json::from_str(
        &std::fs::read_to_string(root.join("observations/soft_edit_curve_lengths.json")).unwrap(),
    )
    .unwrap();
    assert_eq!(rows.len(), 187);
    let mut failures = Vec::new();
    for (row, sdk) in rows.iter().zip(sdk["results"].as_array().unwrap()) {
        assert_eq!(row["id"], sdk["id"]);
        let sdk = &sdk["value"];
        let name = row["id"].as_str().unwrap();
        let v = &row["value"];
        let recipe = &v["recipe"];
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            let mut doc = open_3dm_with_views(
                root.join(format!("fixtures/soft_edit_curve_command/{name}.3dm"))
                    .to_str()
                    .unwrap(),
            )
            .unwrap()
            .0;
            let id = v["source_id"]
                .as_str()
                .unwrap()
                .parse::<ObjectId>()
                .unwrap();
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
            let options = Options {
                distance: recipe["distance"].as_f64().unwrap(),
                copy: recipe["copy"] == true,
                fix_ends: recipe["fix_ends"] == true,
            };
            let mut draft = Draft::new(&doc, id).unwrap();
            draft.set_options(options).unwrap();
            draft.point(point(&v["reference"])).unwrap();
            let preview = draft.preview(point(&v["target"])).unwrap().unwrap();
            soft_comparison::ideal(
                preview.curve_ref().unwrap(),
                sdk,
                recipe,
                point(&v["target"]),
            );
            let rejected = viboceros_geometry::curve_points_coincident(
                draft.anchor().unwrap(),
                point(&v["target"]),
            );
            assert_eq!(
                doc.objects().cloned().collect::<Vec<_>>(),
                before,
                "{name}: preview mutation"
            );
            let registry = CommandRegistry::with_builtins();
            if (recipe["cancel"] != true || recipe["cancel_after"] == true) && !rejected {
                doc.select_object(id, SelectionMode::Replace).unwrap();
                let input = format!(
                    "SoftEditCrv {} {} {}",
                    format_point(point(&v["reference"])),
                    format_point(point(&v["target"])),
                    options.arguments()
                );
                if recipe["preselect"] == false {
                    registry
                        .execute_postselected(&mut doc, &input, Default::default())
                        .unwrap();
                } else {
                    registry.execute(&mut doc, &input).unwrap();
                }
                let result = if options.copy {
                    doc.objects().find(|o| o.id() != id).unwrap()
                } else {
                    doc.object(id).unwrap()
                };
                assert_eq!(result.geometry(), &preview, "{name}: preview and API");
            } else {
                doc.clear_selection();
            }
            assert_eq!(doc.object(id).unwrap().attributes(), before[0].attributes());
            assert_eq!(doc.object(id).unwrap().group_ids(), before[0].group_ids());
            let mut gap =
                comparison::check_with_weight_tolerance(&doc, &v["after"], id, name, 1e-12);
            if !v["undo"].is_null() {
                assert_eq!(doc.undo_label(), Some("SoftEditCrv"));
                registry.execute(&mut doc, "Undo").unwrap();
                gap = gap.max(comparison::check_with_weight_tolerance(
                    &doc, &v["undo"], id, name, 1e-12,
                ));
                registry.execute(&mut doc, "Redo").unwrap();
                gap = gap.max(comparison::check_with_weight_tolerance(
                    &doc, &v["redo"], id, name, 1e-12,
                ));
            }
            eprintln!(
                "SOFT_EDIT_CURVE_AUDIT {}",
                serde_json::json!({"id":name,"maximum_error":gap})
            );
            assert!(
                gap <= soft_comparison::COMMAND_EPSILON,
                "{name}: maximum difference {gap}"
            );
        }));
        if result.is_err() {
            failures.push(name);
        }
    }
    assert!(failures.is_empty(), "SoftEditCrv differences: {failures:?}");
}

#[test]
fn malformed_batches_and_stale_previews_preserve_model_and_redo() {
    let root = root();
    let mut doc = open_3dm_with_views(
        root.join("fixtures/soft_edit_curve_command/cubic_0.3dm")
            .to_str()
            .unwrap(),
    )
    .unwrap()
    .0;
    let id = doc.objects().next().unwrap().id();
    let registry = CommandRegistry::with_builtins();
    doc.clear_history().unwrap();
    registry.execute(&mut doc, "Point 50,60,70").unwrap();
    registry.execute(&mut doc, "Undo").unwrap();
    doc.select_object(id, SelectionMode::Replace).unwrap();
    let before = doc.objects().cloned().collect::<Vec<_>>();
    for input in [
        "SoftEditCrv 0,0,0 0,0,2 Distance=0",
        "SoftEditCrv 0,0,0 0,0,2 Distance=NaN",
        "SoftEditCrv 0,0,0 0,0,2 FixEnds=unknown",
        "SoftEditCrv 0,0,0 0,0,2 unexpected",
    ] {
        assert!(registry.execute(&mut doc, input).is_err(), "{input}");
        assert_eq!(doc.objects().cloned().collect::<Vec<_>>(), before);
        assert!(doc.can_redo());
        assert!(!doc.can_undo());
    }
    let mut draft = Draft::new(&doc, id).unwrap();
    draft.point(Point3::try_new(0., 0., 0.).unwrap()).unwrap();
    draft.point(Point3::try_new(0., 0., 2.).unwrap()).unwrap();
    registry.execute(&mut doc, "Move 0,0,0 1,0,0").unwrap();
    let changed = doc.objects().cloned().collect::<Vec<_>>();
    assert!(draft.commit(&mut doc, false).is_err());
    assert_eq!(doc.objects().cloned().collect::<Vec<_>>(), changed);
}

#[test]
fn native_soft_edit_repeated_copies_distance_picks_and_history() {
    let root = root();
    let r: Value = serde_json::from_str(
        &std::fs::read_to_string(root.join("observations/soft_edit_curve_sequence.json")).unwrap(),
    )
    .unwrap();
    let rows = r["results"].as_array().unwrap();
    assert_eq!(rows.len(), 32);
    let mut failures = Vec::new();
    for row in rows {
        let name = row["id"].as_str().unwrap();
        let v = &row["value"];
        let recipe = &v["recipe"];
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            let mut doc = open_3dm_with_views(
                root.join(format!("fixtures/soft_edit_curve_sequence/{name}.3dm"))
                    .to_str()
                    .unwrap(),
            )
            .unwrap()
            .0;
            let id = v["source_id"]
                .as_str()
                .unwrap()
                .parse::<ObjectId>()
                .unwrap();
            doc.select_object(id, SelectionMode::Replace).unwrap();
            doc.clear_history().unwrap();
            let before = doc.objects().cloned().collect::<Vec<_>>();
            let mut draft = Draft::new(&doc, id).unwrap();
            let mut options = Options {
                distance: 4.,
                copy: recipe["copy"] == true,
                fix_ends: false,
            };
            draft.set_options(options).unwrap();
            for (i, pair) in v["all_points"]
                .as_array()
                .unwrap()
                .chunks_exact(2)
                .enumerate()
            {
                draft.point(point(&pair[0])).unwrap();
                if i == 0 && !v["distance_pick"].is_null() {
                    options.distance = draft.distance_to_pick(point(&v["distance_pick"])).unwrap();
                    draft.set_options(options).unwrap();
                }
                draft.point(point(&pair[1])).unwrap();
                assert_eq!(
                    doc.objects().cloned().collect::<Vec<_>>(),
                    before,
                    "{name}: staged model"
                );
            }
            doc.begin_transaction("SoftEditCrv").unwrap();
            draft.commit(&mut doc, false).unwrap();
            doc.commit_transaction().unwrap();
            let mut gap = soft_comparison::check_many(&doc, &v["after"], id, name);
            let registry = CommandRegistry::with_builtins();
            registry.execute(&mut doc, "Undo").unwrap();
            gap = gap.max(soft_comparison::check_many(&doc, &v["undo"], id, name));
            registry.execute(&mut doc, "Redo").unwrap();
            gap = gap.max(soft_comparison::check_many(&doc, &v["redo"], id, name));
            eprintln!(
                "SOFT_EDIT_SEQUENCE_AUDIT {}",
                serde_json::json!({"id":name,"maximum_error":gap})
            );
            assert!(
                gap <= soft_comparison::COMMAND_EPSILON,
                "{name}: difference {gap}"
            );
        }));
        if result.is_err() {
            failures.push(name);
        }
    }
    assert!(
        failures.is_empty(),
        "Repeated soft edit differences: {failures:?}"
    );
}
