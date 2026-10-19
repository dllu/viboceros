use super::insert_kink::comparison;
use super::*;
use serde_json::Value;
#[path = "../../../crates/viboceros-command/tests/support/soft_edit_surface.rs"]
mod surface_comparison;
fn enter(app: &mut VibocerosApp, text: &str) {
    app.command_input = text.into();
    app.run_command();
}
fn point(v: &Value) -> Point3 {
    Point3::try_from(serde_json::from_value::<[f64; 3]>(v.clone()).unwrap()).unwrap()
}
#[test]
fn native_surface_soft_edit_application_previews_cancel_selection_and_history() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tools/rhino_oracle");
    let mut failures = Vec::new();
    for (dataset, count) in [
        ("soft_edit_surface_command", 108),
        ("soft_edit_surface_shapes", 105),
    ] {
        let capture: Value = serde_json::from_str(
            &std::fs::read_to_string(root.join(format!("observations/{dataset}.json"))).unwrap(),
        )
        .unwrap();
        let rows = capture["results"].as_array().unwrap();
        assert_eq!(rows.len(), count);
        for row in rows {
            let name = row["id"].as_str().unwrap();
            let v = &row["value"];
            let recipe = &v["recipe"];
            let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                let mut app = test_app();
                app.document = viboceros_command::open_3dm_with_views(
                    root.join(format!("fixtures/{dataset}/{name}.3dm"))
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
                let before = app.document.objects().cloned().collect::<Vec<_>>();
                if viboceros_command::surface_reference::surface(
                    app.document.object(id).unwrap().geometry(),
                    None,
                )
                .unwrap()
                .0
                .is_closed_v()
                .unwrap()
                {
                    enter(&mut app, "SoftEditSrf");
                    app.pick_refinement_object(Some(id));
                    assert!(app.refinement_prompt.as_ref().unwrap().selecting());
                    app.cancel_current_prompt_or_selection();
                    assert_eq!(app.document.objects().cloned().collect::<Vec<_>>(), before);
                    eprintln!("SOFT_EDIT_SURFACE_UI_DEFERRED {name}: closed V");
                    return;
                }

                enter(&mut app, "SoftEditSrf");
                assert!(app.refinement_prompt.is_some(), "{name}: prompt");
                if app.refinement_prompt.as_ref().unwrap().selecting() {
                    app.apply_selection_click(SelectionClick {
                        object_id: Some(id),
                        mode: SelectionMode::Replace,
                    });
                }
                assert_eq!(
                    app.refinement_prompt.as_ref().unwrap().surface(),
                    Some((id, None, false))
                );
                enter(
                    &mut app,
                    &format!("w{}", format_model_point(point(&v["reference"]))),
                );
                assert!(
                    app.refinement_prompt.as_ref().unwrap().anchor().is_some(),
                    "{name}: base accepted"
                );
                assert!(app.refinement_prompt.as_ref().unwrap().surface().is_none());
                for (axis, label) in ["U_Distance", "V_Distance"].into_iter().enumerate() {
                    enter(&mut app, label);
                    enter(
                        &mut app,
                        &recipe["distance"][axis].as_f64().unwrap().to_string(),
                    );
                }
                enter(
                    &mut app,
                    if recipe["fix_edges"] == true {
                        "FixEdges=Yes"
                    } else {
                        "FixEdges=No"
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
                enter(&mut app, "DirConstraint");
                enter(&mut app, recipe["constraint"].as_str().unwrap_or("None"));
                app.update_soft_edit_hover(point(&v["target"]));
                assert_eq!(
                    app.document.objects().cloned().collect::<Vec<_>>(),
                    before,
                    "{name}: preview mutation"
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
                assert!(app.refinement_prompt.is_none(), "{name}: ended");
                let mut gap = surface_comparison::check(&app.document, &v["after"], id, name);
                if v["undo"].is_array() {
                    assert_eq!(app.document.undo_label(), Some("SoftEditSrf"));
                    enter(&mut app, "Undo");
                    gap = gap.max(surface_comparison::check(
                        &app.document,
                        &v["undo"],
                        id,
                        name,
                    ));
                    enter(&mut app, "Redo");
                    gap = gap.max(surface_comparison::check(
                        &app.document,
                        &v["redo"],
                        id,
                        name,
                    ));
                }
                eprintln!(
                    "SOFT_EDIT_SURFACE_UI_AUDIT {}",
                    serde_json::json!({"dataset":dataset,"id":name,"maximum_error":gap})
                );
                assert!(gap <= 5e-8, "{name}: difference {gap}");
            }));
            if result.is_err() {
                failures.push(name.to_owned());
            }
        }
    }
    assert!(
        failures.is_empty(),
        "Surface soft edit UI differences: {failures:?}"
    );
}

#[test]
fn native_repeated_surface_getters_copy_toggles_and_atomic_history() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tools/rhino_oracle");
    let capture: Value = serde_json::from_str(
        &std::fs::read_to_string(root.join("observations/soft_edit_surface_sequence.json"))
            .unwrap(),
    )
    .unwrap();
    assert_eq!(capture["results"].as_array().unwrap().len(), 16);
    for row in capture["results"].as_array().unwrap() {
        let name = row["id"].as_str().unwrap();
        let v = &row["value"];
        let recipe = &v["recipe"];
        let mut app = test_app();
        app.document = viboceros_command::open_3dm_with_views(
            root.join(format!("fixtures/soft_edit_surface_sequence/{name}.3dm"))
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
        app.document
            .select_object(id, SelectionMode::Replace)
            .unwrap();
        app.document.clear_history().unwrap();
        let before = app.document.objects().cloned().collect::<Vec<_>>();
        enter(
            &mut app,
            "SoftEditSrf U_Distance=4 V_Distance=4 DirConstraint=None",
        );
        enter(
            &mut app,
            if recipe["copy"] == true {
                "Copy=Yes"
            } else {
                "Copy=No"
            },
        );
        for (i, pair) in v["all_points"]
            .as_array()
            .unwrap()
            .chunks_exact(2)
            .enumerate()
        {
            if i > 0
                && let Some(copy) = recipe["moves"][i - 1]["copy"].as_bool()
            {
                enter(&mut app, if copy { "Copy=Yes" } else { "Copy=No" });
            }
            enter(
                &mut app,
                &format!("w{}", format_model_point(point(&pair[0]))),
            );
            app.update_soft_edit_hover(point(&pair[1]));
            assert_eq!(
                app.document.objects().cloned().collect::<Vec<_>>(),
                before,
                "{name}: preview mutation"
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
        let mut gap = surface_comparison::check(&app.document, &v["after"], id, name);
        enter(&mut app, "Undo");
        gap = gap.max(surface_comparison::check(
            &app.document,
            &v["undo"],
            id,
            name,
        ));
        enter(&mut app, "Redo");
        gap = gap.max(surface_comparison::check(
            &app.document,
            &v["redo"],
            id,
            name,
        ));
        eprintln!(
            "SOFT_EDIT_SURFACE_SEQUENCE_UI_AUDIT {}",
            serde_json::json!({"id":name,"maximum_error":gap})
        );
        assert!(gap <= 1e-7, "{name}: difference {gap}");
    }
}
