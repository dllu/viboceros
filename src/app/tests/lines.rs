use super::command_file::replay;
use super::plane_surface_native_support as native;
use super::*;
#[path = "../../../crates/viboceros-command/tests/support/lines.rs"]
mod inputs;
fn enter(app: &mut VibocerosApp, text: &str) {
    app.command_input = text.into();
    app.run_command();
}

#[test]
fn adjoining_line_getters_replay_native_cancellation_undo_close_and_parameter_domains() {
    let sources = inputs::sources();
    let directory = tempfile::tempdir().unwrap();
    let mut count = 0;
    for family in inputs::FAMILIES {
        for row in native::read(family)["results"].as_array().unwrap() {
            let id = row["id"].as_str().unwrap();
            if inputs::DIAGNOSTICS.contains(&id) {
                continue;
            }
            let v = &row["value"];
            let source = &sources[id];
            let mut app = test_app();
            app.document = inputs::setup(v, source);
            let frame = native::frame(v);
            app.viewports[0].set_construction_plane(frame);
            let parents = app.document.objects().cloned().collect::<Vec<_>>();
            let is_macro = source["macro"].is_string();
            if let Some(text) = source["macro"].as_str() {
                let path = directory.path().join("Lines macro.txt");
                std::fs::write(&path, text).unwrap();
                enter(&mut app, &format!("ReadCommandFile \"{}\"", path.display()));
                for _ in 0..100 {
                    if !app.drain_command_file() {
                        break;
                    }
                }
                assert!(app.command_file_job.is_none());
                assert_eq!(
                    app.last_command_file_result,
                    Some(v["success"].as_bool().unwrap())
                );
            } else {
                enter(&mut app, "Lines");
                let trace = &v["input_trace"][0];
                assert_eq!(trace["completed"], trace["expected"]);
                assert!(trace["errors"].as_array().unwrap().is_empty());
                for step in trace["sent"].as_array().unwrap() {
                    assert_eq!(app.active_command, Some(InteractiveCommand::Lines), "{id}");
                    let token = step["token"].as_str().unwrap();
                    enter(&mut app, if token == "_Enter" { "" } else { token });
                    if app.active_command.is_some() {
                        assert_eq!(
                            app.document.objects().len(),
                            parents.len(),
                            "provisional segments must stay in the draft"
                        );
                    }
                }
            }
            assert!(app.active_command.is_none(), "{id} {:?}", app.command_log);
            let comparison = replay::compare(&app.document, &v["after"], false);
            inputs::check_types(&app.document, &v["after"]);
            let native_valid = comparison.native_valid;
            assert_eq!(
                native_valid,
                !["translated_ulp", "translated_small"].contains(&id)
            );
            let check = |doc: &Document, target: &serde_json::Value, archive: bool| {
                if native_valid {
                    replay::check(doc, target, archive)
                } else {
                    replay::compare(doc, target, archive).error
                }
            };
            let mut error = comparison.error;
            assert_eq!(app.document.can_undo(), v["before"] != v["after"], "{id}");
            let selected = app.document.selected_object_ids().collect::<Vec<_>>();
            app.commands
                .execute(
                    &mut app.document,
                    if is_macro {
                        "SelLast DeselectOthersBeforeSelect=Yes"
                    } else {
                        "SelLast DeselectOthersBeforeSelect=No"
                    },
                )
                .unwrap();
            error = error.max(check(&app.document, &v["last_selection"], false));
            app.document.set_transient_selection(selected).unwrap();
            let mut archives = 0;
            if app.document.can_undo() {
                assert_eq!(
                    app.document.undo_label(),
                    Some(if is_macro { "ReadCommandFile" } else { "Lines" })
                );
                app.document.undo().unwrap();
                error = error.max(check(&app.document, &v["undo"], false));
                assert!(!app.document.can_undo());
                app.document.redo().unwrap();
                error = error.max(check(&app.document, &v["redo"], false));
                if native_valid {
                    let path = directory.path().join("lines.3dm");
                    app.commands
                        .execute(&mut app.document, &format!("SaveAs {}", path.display()))
                        .unwrap();
                    let mut restored = Document::new(app.document.tolerance());
                    app.commands
                        .execute(&mut restored, &format!("Open3dm {}", path.display()))
                        .unwrap();
                    error = error.max(replay::check(&restored, &v["after"], true));
                    inputs::check_types(&restored, &v["after"]);
                    archives = 1;
                }
            }
            if v["pending_redo"].is_array() {
                assert!(app.document.can_redo(), "{id}");
                app.document.redo().unwrap();
                error = error.max(replay::check(&app.document, &v["pending_redo"], false));
            }
            for parent in parents {
                let now = app.document.object(parent.id()).unwrap();
                assert_eq!(now, &parent);
                assert!(
                    parent
                        .geometry_snapshot()
                        .shares_storage_with(now.geometry_snapshot())
                );
            }
            println!(
                "LINES_UI {}",
                serde_json::json!({"id":id,"maximum_coefficient_and_sample_error":error,"archives":archives,"native_valid":native_valid})
            );
            assert!(error <= 1e-6, "{id} {error:e}");
            count += 1;
        }
    }
    assert_eq!(count, 47);
}

#[test]
fn lines_picks_transparent_planes_and_macro_history_keep_drafts_atomic() {
    let mut app = test_app();
    enter(&mut app, "Point 9,9,9");
    enter(&mut app, "Undo");
    enter(&mut app, "Lines");
    assert!(app.accept_drafting_point(point(0., 0., 0.)));
    assert!(app.accept_drafting_point(point(4., 0., 0.)));
    assert!(app.document.objects().next().is_none());
    enter(&mut app, "CPlane 3Point");
    for text in ["w10,20,30", "w10,21,30", "w10,20,31"] {
        enter(&mut app, text);
    }
    assert_eq!(app.curve_points, vec![point(0., 0., 0.), point(4., 0., 0.)]);
    enter(&mut app, "0,3,1");
    assert_eq!(app.curve_points.last(), Some(&point(11., 20., 33.)));
    app.cancel_current_prompt_or_selection();
    assert!(app.document.objects().next().is_none());
    assert!(app.document.can_redo());
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("lines.txt");
    std::fs::write(
        &path,
        "! _Lines w0,0,0 w4,0,0 w4,3,1 _Undo w8,2,-1 _Enter _Point w9,9,9",
    )
    .unwrap();
    enter(&mut app, &format!("ReadCommandFile \"{}\"", path.display()));
    for _ in 0..100 {
        if !app.drain_command_file() {
            break;
        }
    }
    assert_eq!(app.last_command_file_result, Some(true));
    assert_eq!(app.document.objects().len(), 3);
    assert_eq!(app.document.undo_label(), Some("ReadCommandFile"));
    app.document.undo().unwrap();
    assert!(app.document.objects().next().is_none());
    app.document.redo().unwrap();
    assert_eq!(app.document.objects().len(), 3);
}
