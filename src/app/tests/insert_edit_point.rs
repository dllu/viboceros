fn audit_matrix(root: &std::path::Path, matrix: &str, count: usize) {
    let capture: Value = serde_json::from_str(
        &std::fs::read_to_string(
            root.join(format!("observations/insert_edit_point_{matrix}.json")),
        )
        .unwrap(),
    )
    .unwrap();
    let rows = capture["results"].as_array().unwrap();
    assert_eq!(rows.len(), count);
    let mut failures = Vec::new();
    for row in rows {
        let name = row["id"].as_str().unwrap();
        match std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            replay(
                &row["value"],
                &root.join(format!("fixtures/insert_edit_point_{matrix}")),
            )
        })) {
            Ok(gap) => eprintln!(
                "INSERT_EDIT_POINT_UI_AUDIT {}",
                serde_json::json!({"matrix":matrix,"id":name,"maximum_error":gap})
            ),
            Err(_) => {
                eprintln!("INSERT_EDIT_POINT_UI_MISMATCH {matrix}/{name}");
                failures.push(name);
            }
        }
    }
    assert!(failures.is_empty(), "Native UI mismatches: {failures:?}");
}
use super::insert_kink::comparison;
use super::*;
use serde_json::Value;
fn enter(app: &mut VibocerosApp, text: &str) {
    app.command_input = text.into();
    app.run_command();
}
fn replay(v: &Value, root: &std::path::Path) -> f64 {
    let r = &v["recipe"];
    let name = r["id"].as_str().unwrap();
    let mut app = test_app();
    app.document =
        viboceros_command::open_3dm_with_views(root.join(format!("{name}.3dm")).to_str().unwrap())
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
    enter(
        &mut app,
        if r["subcurve"].is_null() {
            "InsertEditPoint"
        } else {
            "InsertEditPoint SubCrv"
        },
    );
    assert_eq!(
        app.refinement_prompt.as_ref().unwrap().command_name(),
        "InsertEditPoint"
    );
    if app.refinement_prompt.as_ref().unwrap().selecting() {
        app.apply_selection_click(SelectionClick {
            object_id: Some(id),
            mode: SelectionMode::Replace,
        });
    }
    if let Some(range) = r["subcurve"].as_array() {
        let curve = before[0].geometry().curve_ref().unwrap();
        for fraction in range {
            let p = curve
                .evaluate(curve.parameter_at(fraction.as_f64().unwrap()).unwrap())
                .unwrap();
            enter(&mut app, &format!("w{}", format_model_point(p)));
        }
    }
    for token in v["tokens"].as_array().unwrap() {
        let token = token.as_str().unwrap();
        if token == "!" {
            app.cancel_current_prompt_or_selection();
        } else {
            enter(&mut app, if token == "_Enter" { "" } else { token });
        }
        if app.refinement_prompt.is_some() {
            assert_eq!(
                app.document.objects().cloned().collect::<Vec<_>>(),
                before,
                "{name}: staged edit"
            );
            assert!(!app.document.can_undo());
        }
    }
    if app.refinement_prompt.is_some() && !v["success"].as_bool().unwrap() {
        app.cancel_current_prompt_or_selection();
    }
    assert!(app.refinement_prompt.is_none(), "{name}: command ended");
    let mut gap =
        comparison::check_with_weight_tolerance(&app.document, &v["after"], id, name, 1e-12);
    if !v["undo"].is_null() {
        assert_eq!(app.document.undo_label(), Some("InsertEditPoint"));
        enter(&mut app, "Undo");
        gap = gap.max(comparison::check_with_weight_tolerance(
            &app.document,
            &v["undo"],
            id,
            name,
            1e-12,
        ));
        assert_eq!(app.document.objects().cloned().collect::<Vec<_>>(), before);
        enter(&mut app, "Redo");
        gap = gap.max(comparison::check_with_weight_tolerance(
            &app.document,
            &v["redo"],
            id,
            name,
            1e-12,
        ));
    }
    assert!(gap <= 1e-8, "{name}: {gap}");
    gap
}
#[test]
fn insert_edit_point_native_point_getters_subcurves_and_history() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tools/rhino_oracle");
    audit_matrix(&root, "command", 152);
    audit_matrix(&root, "limits", 48);
    audit_matrix(&root, "projection", 16);
}

#[path = "../../../crates/viboceros-command/tests/support/insert_edit_point.rs"]
mod history;
#[test]
fn insert_edit_point_native_redo_witnesses_through_application_input() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tools/rhino_oracle");
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
        let (doc, id, marker) = history::history_document(op);
        let mut app = test_app();
        app.document = doc;
        let name = op["id"].as_str().unwrap();
        let v = &row["value"];
        history::close_json(
            &history::history_snapshot(&app.document, id, marker),
            &v["before"],
            name,
        );
        for step in v["steps"].as_array().unwrap() {
            history::close_json(
                &history::history_snapshot(&app.document, id, marker),
                &step["before"],
                name,
            );
            if step["command"] == "InsertEditPoint" {
                enter(&mut app, "InsertEditPoint");
                if app.refinement_prompt.as_ref().unwrap().selecting() {
                    app.apply_selection_click(SelectionClick {
                        object_id: Some(id),
                        mode: SelectionMode::Replace,
                    });
                }
                if op["station"] != "empty" {
                    let c = app
                        .document
                        .object(id)
                        .unwrap()
                        .geometry()
                        .curve_ref()
                        .unwrap();
                    let p = c
                        .evaluate(if op["station"] == "existing" {
                            1. / 3.
                        } else {
                            0.35
                        })
                        .unwrap();
                    enter(&mut app, &format!("w{}", format_model_point(p)));
                }
                enter(&mut app, if op["cancel"] == true { "Cancel" } else { "" });
            } else {
                enter(&mut app, step["command"].as_str().unwrap());
            }
            assert!(app.command_line_idle(), "{name}: {:?}", app.command_log);
            history::close_json(
                &history::history_snapshot(&app.document, id, marker),
                &step["after_script"],
                name,
            );
        }
    }
}
