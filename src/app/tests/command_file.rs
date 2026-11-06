use super::plane_surface_native_support as native;
use super::surface_point_grid::replay as grid;
use super::*;
#[path = "../../../crates/viboceros-command/tests/support/command_file.rs"]
pub(super) mod replay;

fn enter(app: &mut VibocerosApp, text: &str) {
    app.command_input = text.into();
    app.run_command();
}
fn drain(app: &mut VibocerosApp) {
    for _ in 0..1000 {
        if !app.drain_command_file() {
            return;
        }
    }
    panic!("macro did not yield");
}

#[test]
fn command_files_replay_native_input_and_group_completed_edits_into_one_undo() {
    let directory = tempfile::tempdir().unwrap();
    let file = directory.path().join("owned command file.txt");
    let mut count = 0;
    for family in [
        "command_file",
        "command_file_holdouts",
        "command_file_extras",
        "command_file_polyline",
    ] {
        let data = native::read(family);
        for row in data["results"].as_array().unwrap() {
            let v = &row["value"];
            let id = row["id"].as_str().unwrap();
            let sources: serde_json::Value = serde_json::from_str(include_str!(
                "../../../tools/rhino_oracle/fixtures/command_file_sources.json"
            ))
            .unwrap();
            let source = &sources[id];
            let mut text = source["text"].as_str().unwrap().to_owned();
            if let Some(child) = source["child"].as_str() {
                let child_path = directory.path().join("owned child file.txt");
                std::fs::write(&child_path, child).unwrap();
                text = text.replace("@CHILD@", child_path.to_str().unwrap());
            }
            let bytes = match source["encoding"].as_str().unwrap_or("utf-8") {
                "utf-16" => [
                    vec![0xff, 0xfe],
                    text.encode_utf16().flat_map(u16::to_le_bytes).collect(),
                ]
                .concat(),
                "utf-8-sig" => [vec![0xef, 0xbb, 0xbf], text.as_bytes().to_vec()].concat(),
                _ => text.as_bytes().to_vec(),
            };
            std::fs::write(&file, bytes).unwrap();
            let mut app = test_app();
            app.document = native::setup(v);
            let parents = app.document.objects().cloned().collect::<Vec<_>>();
            enter(
                &mut app,
                &format!("_-ReadCommandFile \"{}\"", file.display()),
            );
            drain(&mut app);
            for trace in v["input_trace"].as_array().unwrap() {
                assert!(trace["errors"].as_array().unwrap().is_empty());
                assert_eq!(trace["completed"], trace["expected"]);
                for step in trace["sent"].as_array().unwrap() {
                    let token = step["token"].as_str().unwrap().trim_start_matches('_');
                    enter(&mut app, if token == "Enter" { "" } else { token });
                    drain(&mut app);
                }
            }
            assert!(app.command_file_job.is_none(), "{id} {:?}", app.command_log);
            assert_eq!(
                app.last_command_file_result,
                Some(v["success"].as_bool().unwrap()),
                "{id} {:?}",
                app.command_log
            );
            let mut error = replay::check(&app.document, &v["after"], false);
            check_layers(&app.document, v, "after");
            if v["last_selection"].is_array() {
                let selection = app.document.selected_object_ids().collect::<Vec<_>>();
                app.commands.execute(&mut app.document, "SelLast").unwrap();
                error = error.max(replay::check(&app.document, &v["last_selection"], false));
                app.document.set_transient_selection(selection).unwrap();
            }
            let mut undo_steps = 0;
            while app.document.can_undo() {
                app.document.undo().unwrap();
                undo_steps += 1;
                error = error.max(replay::check(
                    &app.document,
                    &v["undo_steps"][undo_steps - 1],
                    false,
                ));
            }
            assert_eq!(undo_steps, v["undo_steps"].as_array().unwrap().len());
            check_layers(&app.document, v, "undo");
            for target in v["redo_steps"].as_array().unwrap() {
                app.document.redo().unwrap();
                error = error.max(replay::check(&app.document, target, false));
            }
            let layer_history_matches_native = check_layers_match(&app.document, v, "redo");
            if !layer_history_matches_native {
                assert_eq!(id, "layer_options");
                check_layers(&app.document, v, "after");
            }
            let mut archive = 0;
            if undo_steps > 0 {
                let archive_path = directory.path().join("macro.3dm");
                app.commands
                    .execute(
                        &mut app.document,
                        &format!("SaveAs {}", archive_path.display()),
                    )
                    .unwrap();
                let mut restored = Document::new(app.document.tolerance());
                app.commands
                    .execute(
                        &mut restored,
                        &format!("Open3dm {}", archive_path.display()),
                    )
                    .unwrap();
                error = error.max(replay::check(&restored, &v["after"], true));
                check_layers(&restored, v, "after");
                archive = 1;
            }
            for parent in parents {
                assert_eq!(app.document.object(parent.id()), Some(&parent));
                assert!(
                    parent.geometry_snapshot().shares_storage_with(
                        app.document
                            .object(parent.id())
                            .unwrap()
                            .geometry_snapshot()
                    )
                );
            }
            println!(
                "COMMAND_FILE_UI {}",
                serde_json::json!({"id":id,"maximum_coefficient_and_sample_error":error,"undo_steps":undo_steps,"archives":archive,"layer_history_matches_native":layer_history_matches_native})
            );
            assert!(error <= 1e-6, "{id} {error:e}");
            count += 1;
        }
    }
    assert_eq!(count, 28);
}

fn check_layers(doc: &Document, v: &serde_json::Value, phase: &str) {
    assert!(
        check_layers_match(doc, v, phase),
        "layer state differs at {phase}"
    );
}
fn check_layers_match(doc: &Document, v: &serde_json::Value, phase: &str) -> bool {
    let Some(before) = v["layers_before"].as_array() else {
        return true;
    };
    let names = before
        .iter()
        .map(|l| l["name"].as_str().unwrap())
        .collect::<Vec<_>>();
    let expected = v["layers_".to_owned() + phase].as_array().unwrap();
    let current_matches = doc.layer(doc.current_layer_id()).unwrap().name()
        == expected.iter().find(|l| l["current"] == true).unwrap()["name"]
            .as_str()
            .unwrap();
    let mut expected = expected
        .iter()
        .filter(|l| !names.contains(&l["name"].as_str().unwrap()))
        .cloned()
        .collect::<Vec<_>>();
    let mut actual=doc.layers().filter(|l|!names.contains(&l.name())).map(|l|serde_json::json!({"name":l.name(),"visible":l.is_visible(),"locked":l.is_locked(),"color":[l.color().red,l.color().green,l.color().blue],"current":l.id()==doc.current_layer_id()})).collect::<Vec<_>>();
    expected.sort_by_key(|l| l["name"].as_str().unwrap().to_owned());
    actual.sort_by_key(|l| l["name"].as_str().unwrap().to_owned());
    current_matches && actual == expected
}

#[test]
fn filename_prompt_and_recursive_file_failure_preserve_pending_redo() {
    let mut app = test_app();
    app.commands
        .execute(&mut app.document, "Point 1,2,3")
        .unwrap();
    app.document.undo().unwrap();
    enter(&mut app, "ReadCommandFile");
    assert!(app.command_file_prompt);
    assert!(!app.document.can_undo());
    assert!(!app.document.can_redo());
    app.cancel_current_prompt_or_selection();
    assert!(app.document.can_redo());
    assert_eq!(app.last_command_file_result, Some(false));
    let directory = tempfile::tempdir().unwrap();
    let file = directory.path().join("self macro.txt");
    std::fs::write(&file, format!("_-ReadCommandFile \"{}\"", file.display())).unwrap();
    enter(
        &mut app,
        &format!("_-ReadCommandFile \"{}\"", file.display()),
    );
    drain(&mut app);
    assert!(app.command_file_job.is_none());
    assert_eq!(app.last_command_file_result, Some(false));
    assert!(app.document.objects().next().is_none());
    assert!(app.document.can_redo());
    assert!(app.command_log.iter().any(|s| s.contains("nesting limit")));
}

#[test]
fn paused_getter_rejects_invalid_input_and_escape_discards_the_macro_tail() {
    let directory = tempfile::tempdir().unwrap();
    let file = directory.path().join("paused.txt");
    std::fs::write(&file, "! _Line _Pause w4,3,2 _Point w8,9,10").unwrap();
    let mut app = test_app();
    enter(&mut app, "ReadCommandFile");
    enter(&mut app, &format!("\"{}\"", file.display()));
    drain(&mut app);
    assert!(app.command_file_job.is_some());
    enter(&mut app, "wNaN,0,0");
    assert!(app.command_file_job.is_some());
    assert!(app.document.objects().next().is_none());
    app.cancel_current_prompt_or_selection();
    assert!(app.command_file_job.is_none());
    assert!(app.active_command.is_none());
    assert!(!app.document.can_undo());
}

#[test]
fn starting_a_file_closes_modeling_and_transparent_view_prompts() {
    let directory = tempfile::tempdir().unwrap();
    let file = directory.path().join("macro.txt");
    std::fs::write(&file, "! _Point w1,2,3").unwrap();
    for prompt in [
        "Line",
        "Zoom Factor",
        "Zoom Target",
        "Zoom Window",
        "SnapSize",
    ] {
        let mut app = test_app();
        enter(&mut app, prompt);
        assert!(!app.command_file_child_idle(), "{prompt}");
        enter(&mut app, &format!("ReadCommandFile \"{}\"", file.display()));
        drain(&mut app);
        assert_eq!(
            app.last_command_file_result,
            Some(true),
            "{prompt} {:?}",
            app.command_log
        );
        assert!(app.command_file_child_idle());
        assert_eq!(app.document.objects().len(), 1);
        assert_eq!(app.document.undo_label(), Some("ReadCommandFile"));
        app.document.undo().unwrap();
        assert!(app.document.objects().next().is_none());
    }
}
