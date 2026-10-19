use super::insert_kink::comparison;
use super::*;
use serde_json::Value;
#[path = "../../../crates/viboceros-command/tests/support/soft_edit_curve.rs"]
mod soft_comparison;
fn enter(app: &mut VibocerosApp, s: &str) {
    app.command_input = s.into();
    app.run_command();
}
fn point(v: &Value) -> Point3 {
    Point3::try_from(serde_json::from_value::<[f64; 3]>(v.clone()).unwrap()).unwrap()
}

#[test]
fn native_soft_edit_curve_application_previews_and_history() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tools/rhino_oracle");
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
            let mut app = test_app();
            app.document = viboceros_command::open_3dm_with_views(
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
            app.document.clear_selection();
            if v["before"][0]["selected"] == true {
                app.document
                    .select_object(id, SelectionMode::Replace)
                    .unwrap();
            }
            app.document.clear_history().unwrap();
            let original = app.document.objects().cloned().collect::<Vec<_>>();
            enter(&mut app, "SoftEditCrv");
            if app.refinement_prompt.as_ref().unwrap().selecting() {
                app.apply_selection_click(SelectionClick {
                    object_id: Some(id),
                    mode: SelectionMode::Replace,
                });
            }
            enter(
                &mut app,
                &format!("w{}", format_model_point(point(&v["reference"]))),
            );
            enter(&mut app, "Distance");
            enter(&mut app, &recipe["distance"].as_f64().unwrap().to_string());
            enter(
                &mut app,
                if recipe["fix_ends"] == true {
                    "FixEnds=Yes"
                } else {
                    "FixEnds=No"
                },
            );
            enter(
                &mut app,
                if recipe["copy"] == true {
                    "Copy=Yes"
                } else {
                    "Copy=No"
                },
            );
            app.update_soft_edit_hover(point(&v["target"]));
            assert_eq!(
                app.document.objects().cloned().collect::<Vec<_>>(),
                original,
                "{name}: preview mutation"
            );
            assert!(app.refinement_prompt.as_ref().unwrap().scene().is_some());
            let scene = app.refinement_prompt.as_ref().unwrap().scene().unwrap();
            let preview = if recipe["copy"] == true {
                scene.objects().find(|o| o.id() != id).unwrap()
            } else {
                scene.object(id).unwrap()
            };
            soft_comparison::ideal(
                preview.geometry().curve_ref().unwrap(),
                sdk,
                recipe,
                point(&v["target"]),
            );
            if recipe["cancel"] != true || recipe["cancel_after"] == true {
                enter(
                    &mut app,
                    &format!("w{}", format_model_point(point(&v["target"]))),
                );
            }
            if recipe["cancel"] == true {
                app.cancel_current_prompt_or_selection();
            } else {
                enter(&mut app, "");
            }
            assert!(app.refinement_prompt.is_none(), "{name}: command ended");
            let mut gap = comparison::check_with_weight_tolerance(
                &app.document,
                &v["after"],
                id,
                name,
                1e-12,
            );
            if !v["undo"].is_null() {
                assert_eq!(app.document.undo_label(), Some("SoftEditCrv"));
                enter(&mut app, "Undo");
                gap = gap.max(comparison::check_with_weight_tolerance(
                    &app.document,
                    &v["undo"],
                    id,
                    name,
                    1e-12,
                ));
                enter(&mut app, "Redo");
                gap = gap.max(comparison::check_with_weight_tolerance(
                    &app.document,
                    &v["redo"],
                    id,
                    name,
                    1e-12,
                ));
            }
            eprintln!(
                "SOFT_EDIT_CURVE_UI_AUDIT {}",
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
    assert!(
        failures.is_empty(),
        "Soft edit application differences: {failures:?}"
    );
}

#[test]
fn native_soft_edit_curve_repeated_application_inputs_and_distance_picks() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tools/rhino_oracle");
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
            let mut app = test_app();
            app.document = viboceros_command::open_3dm_with_views(
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
            app.document
                .select_object(id, SelectionMode::Replace)
                .unwrap();
            app.document.clear_history().unwrap();
            let original = app.document.objects().cloned().collect::<Vec<_>>();
            enter(&mut app, "SoftEditCrv");
            for (i, pair) in v["all_points"]
                .as_array()
                .unwrap()
                .chunks_exact(2)
                .enumerate()
            {
                enter(
                    &mut app,
                    &format!("w{}", format_model_point(point(&pair[0]))),
                );
                if i == 0 {
                    enter(&mut app, "Distance");
                    if v["distance_pick"].is_null() {
                        enter(&mut app, "4");
                    } else {
                        enter(
                            &mut app,
                            &format!("w{}", format_model_point(point(&v["distance_pick"]))),
                        );
                    }
                    enter(
                        &mut app,
                        if recipe["copy"] == true {
                            "Copy=Yes"
                        } else {
                            "Copy=No"
                        },
                    );
                    enter(&mut app, "FixEnds=No");
                }
                app.update_soft_edit_hover(point(&pair[1]));
                assert_eq!(
                    app.document.objects().cloned().collect::<Vec<_>>(),
                    original,
                    "{name}: preview model"
                );
                enter(
                    &mut app,
                    &format!("w{}", format_model_point(point(&pair[1]))),
                );
            }
            if recipe["cancel"] == true {
                app.cancel_current_prompt_or_selection();
            } else {
                enter(&mut app, "");
            }
            assert!(app.refinement_prompt.is_none());
            let mut gap = soft_comparison::check_many(&app.document, &v["after"], id, name);
            enter(&mut app, "Undo");
            gap = gap.max(soft_comparison::check_many(
                &app.document,
                &v["undo"],
                id,
                name,
            ));
            enter(&mut app, "Redo");
            gap = gap.max(soft_comparison::check_many(
                &app.document,
                &v["redo"],
                id,
                name,
            ));
            eprintln!(
                "SOFT_EDIT_SEQUENCE_UI_AUDIT {}",
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
        "Repeated soft edit application differences: {failures:?}"
    );
}

#[test]
fn soft_edit_cancel_preserves_redo_and_unit_input_is_preview_only() {
    let mut app = test_app();
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    app.document = viboceros_command::open_3dm_with_views(
        root.join("tools/rhino_oracle/fixtures/soft_edit_curve_command/cubic_0.3dm")
            .to_str()
            .unwrap(),
    )
    .unwrap()
    .0;
    let id = app.document.objects().next().unwrap().id();
    app.document.clear_history().unwrap();
    enter(&mut app, "Point 30,30,30");
    enter(&mut app, "Undo");
    let before = app.document.objects().cloned().collect::<Vec<_>>();
    app.document
        .select_object(id, SelectionMode::Replace)
        .unwrap();
    enter(&mut app, "SoftEditCrv");
    let curve = before[0].geometry().curve_ref().unwrap();
    let base = curve
        .evaluate(curve.parameter_at(1. / 3.).unwrap())
        .unwrap();
    enter(&mut app, &format!("w{}", format_model_point(base)));
    enter(&mut app, "Distance");
    enter(&mut app, "0.4cm");
    assert_eq!(app.commands.soft_edit_curve_options().distance, 4.);
    enter(&mut app, "FixEnds=Yes");
    enter(&mut app, "");
    assert!(
        app.refinement_prompt
            .as_ref()
            .is_some_and(|p| p.anchor().is_some())
    );
    let destination = base
        .translated(viboceros_geometry::Vector3::try_new(0., 0., 2.).unwrap())
        .unwrap();
    app.update_soft_edit_hover(destination);
    assert_eq!(app.document.objects().cloned().collect::<Vec<_>>(), before);
    assert!(app.document.can_redo());
    assert!(!app.document.can_undo());
    app.cancel_current_prompt_or_selection();
    assert!(app.refinement_prompt.is_none());
    assert!(app.document.can_redo());
    assert_eq!(app.document.objects().cloned().collect::<Vec<_>>(), before);
    enter(&mut app, "Redo");
    assert_eq!(app.document.objects().len(), 2);
}
