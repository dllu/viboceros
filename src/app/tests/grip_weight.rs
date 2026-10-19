use super::delete_grips::close;
use super::*;
use serde_json::Value;

fn enter(app: &mut VibocerosApp, text: &str) {
    app.command_input = text.into();
    app.run_command();
}
fn sources() -> Vec<(Value, Value, viboceros_oracle::delete_grips::Prepared)> {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tools/rhino_oracle");
    let mut cases = Vec::new();
    for (name, count) in [
        ("grip_weight", 237),
        ("grip_weight_selection", 5),
        ("grip_weight_representations", 20),
    ] {
        let q: Value = serde_json::from_str(
            &std::fs::read_to_string(root.join(format!("fixtures/{name}.json"))).unwrap(),
        )
        .unwrap();
        let r: Value = serde_json::from_str(
            &std::fs::read_to_string(root.join(format!("observations/{name}.json"))).unwrap(),
        )
        .unwrap();
        assert_eq!(r["results"].as_array().unwrap().len(), count);
        for (op, row) in q["operations"]
            .as_array()
            .unwrap()
            .iter()
            .zip(r["results"].as_array().unwrap())
        {
            assert_eq!(op["id"], row["id"]);
            let mut f: viboceros_oracle::grip_weight::Fixture =
                serde_json::from_value(op.clone()).unwrap();
            f.rebase(&root.join(format!("fixtures/{name}")));
            cases.push((
                op.clone(),
                row["value"].clone(),
                viboceros_oracle::grip_weight::prepare(&f).unwrap(),
            ));
        }
    }
    cases
}
#[test]
fn complete_native_weight_application_dialog_getter_and_history() {
    let mut failures = Vec::new();
    for (op, value, state) in sources() {
        let mut app = test_app();
        app.document = state.document.clone();
        let id = op["id"].as_str().unwrap();
        let check = |app: &VibocerosApp, v: &Value, path: &str| {
            close(
                &viboceros_oracle::grip_weight::snapshot_document(&app.document, &state).unwrap(),
                v,
                path,
            );
        };
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            check(&app, &value["before"], id);
            for step in value["steps"].as_array().unwrap() {
                check(&app, &step["before"], &format!("{id}/before"));
                if step["command"] == "Weight" {
                    let dash = op["dash"].as_bool().unwrap();
                    enter(&mut app, if dash { "_-Weight" } else { "_Weight" });
                    let getter = &step["getter"];
                    if getter["selection"] == true {
                        if op["pick"] == true {
                            enter(&mut app, "SelPt");
                            enter(&mut app, "");
                        } else {
                            enter(&mut app, "Cancel");
                        }
                    }
                    for input in getter["inputs"].as_array().unwrap() {
                        if let Some(v) = input.as_f64() {
                            if dash {
                                enter(&mut app, &v.to_string());
                            } else {
                                app.set_weight_preview(v);
                            }
                        } else {
                            enter(&mut app, if input == "Enter" { "" } else { "Cancel" });
                        }
                    }
                    if !dash && app.weight_prompt.is_some() {
                        enter(&mut app, "");
                    }
                } else {
                    enter(&mut app, step["command"].as_str().unwrap());
                }
                assert!(app.command_line_idle(), "{id}: {:?}", app.command_log);
                assert!(app.weight_prompt.is_none(), "{id}");
                check(
                    &app,
                    &step["after_script"],
                    &format!("{id}/{}", step["command"]),
                );
            }
        }));
        if result.is_err() {
            failures.push(id.to_owned());
        }
    }
    assert!(failures.is_empty(), "Weight UI differences: {failures:?}");
}

#[test]
fn weight_preview_is_readonly_cancel_preserves_redo_and_stale_sources_cancel() {
    let (_, value, state) = sources()
        .into_iter()
        .find(|(op, _, _)| op["id"] == "weighted_middle")
        .unwrap();
    let mut app = test_app();
    app.document = state.document.clone();
    let snapshot =
        |doc: &Document| viboceros_oracle::grip_weight::snapshot_document(doc, &state).unwrap();
    enter(&mut app, "Weight");
    app.set_weight_preview(2.);
    close(
        &snapshot(&app.document),
        &value["before"],
        "readonly preview",
    );
    // Preview history is deliberately empty, so compare the geometry and grips.
    let preview = snapshot(app.weight_prompt.as_ref().unwrap().scene().unwrap());
    close(
        &preview["objects"],
        &value["steps"][0]["after_script"]["objects"],
        "preview geometry",
    );
    enter(&mut app, "Cancel");
    close(&snapshot(&app.document), &value["before"], "cancel");
    enter(&mut app, "Redo");
    assert!(app.document.object(state.marker).is_some());
    enter(&mut app, "Undo");
    enter(&mut app, "Weight");
    app.set_weight_preview(2.);
    app.document.clear_control_point_selection();
    app.validate_weight_preview();
    assert!(app.weight_prompt.is_none());
    assert!(app.object_prompt.is_none());
    assert!(app.document.object(state.marker).is_none());
    enter(&mut app, "Redo");
    assert!(app.document.object(state.marker).is_some());
}

#[test]
fn weight_inline_input_and_escape_use_the_same_commit_boundary() {
    let (_, value, state) = sources()
        .into_iter()
        .find(|(op, _, _)| op["id"] == "weighted_middle")
        .unwrap();
    let mut app = test_app();
    app.document = state.document.clone();
    enter(&mut app, "_-Weight 2");
    assert!(app.command_line_idle());
    close(
        &viboceros_oracle::grip_weight::snapshot_document(&app.document, &state).unwrap(),
        &value["steps"][0]["after_script"],
        "inline",
    );
    enter(&mut app, "Undo");
    enter(&mut app, "Weight");
    app.set_weight_preview(3.);
    app.cancel_current_prompt_or_selection();
    assert!(app.weight_prompt.is_none());
    close(
        &viboceros_oracle::grip_weight::snapshot_document(&app.document, &state).unwrap(),
        &value["before"],
        "escape geometry",
    );
    enter(&mut app, "Redo");
    close(
        &viboceros_oracle::grip_weight::snapshot_document(&app.document, &state).unwrap(),
        &value["steps"][0]["after_script"],
        "redo after escape",
    );
}
