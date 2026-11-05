use super::oracle_observation as observation;
use super::*;
use serde_json::Value;
#[path = "../../../crates/viboceros-oracle/tests/support/subd_volume.rs"]
mod native;
fn enter(app: &mut VibocerosApp, input: &str) {
    app.command_input = input.into();
    app.run_command();
}
#[test]
fn subd_volume_app_admission_picking_warnings_escape_and_mixed_results_match_native() {
    let base = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tools/rhino_oracle");
    let mut count = 0;
    for family in ["subd_volume_discovery", "subd_volume_open"] {
        let request: Value = serde_json::from_slice(
            &std::fs::read(base.join(format!("fixtures/{family}.json"))).unwrap(),
        )
        .unwrap();
        let expected = observation::read(&base, family);
        for (op, row) in request["operations"]
            .as_array()
            .unwrap()
            .iter()
            .zip(expected["results"].as_array().unwrap())
        {
            if !matches!(
                op["id"].as_str().unwrap(),
                "sharp_cube_1_1_0_VolumeCentroid"
                    | "reversed_cube_0_1_0_Volume"
                    | "skew_cube_0_0_0_VolumeCentroid"
                    | "open_cube_0_1_0_Volume_no"
                    | "open_cube_0_1_0_VolumeCentroid_escape"
                    | "bent_grid_0_1_1_VolumeCentroid_yes"
            ) {
                continue;
            }
            count += 1;
            eprintln!("{}", op["id"]);
            let v = &row["value"];
            let mut app = test_app();
            app.document = Document::new(Tolerance::try_new(1e-6, 1e-12, 1e-6).unwrap());
            let source = viboceros_oracle::subd_volume::source(
                op["scene"].as_str().unwrap(),
                app.document.tolerance(),
            )
            .unwrap();
            native::definition(
                &viboceros_oracle::subd_volume::definition(&source),
                &v["constructed"],
            );
            let owner = app.document.add_geometry(source).unwrap();
            native::definition(
                &viboceros_oracle::subd_volume::definition(
                    app.document.object(owner).unwrap().geometry(),
                ),
                &v["stored"],
            );
            let mut keys = vec![owner];
            if op["mixed"] == true {
                enter(
                    &mut app,
                    "MeshBox 20,0,0 22,3,0 4 XCount=1 YCount=1 ZCount=1",
                );
                keys.push(app.document.objects().last().unwrap().id());
            }
            if op["flat"] == true {
                enter(&mut app, "SubDDisplayToggle");
            }
            let sources = app.document.objects().cloned().collect::<Vec<_>>();
            app.document.clear_selection();
            app.document.clear_history().unwrap();
            enter(&mut app, "Point 40,50,60");
            enter(&mut app, "Undo");
            if op["preselect"] == true {
                app.document
                    .select_objects_direct(keys.iter().copied(), SelectionMode::Add)
                    .unwrap();
            }
            let check = |app: &VibocerosApp, key: &str| {
                native::state(
                    &viboceros_oracle::subd_volume::snapshot(&app.document, &keys),
                    &v[key],
                )
            };
            check(&app, "before");
            let command = op["command"].as_str().unwrap();
            enter(&mut app, command);
            if op["preselect"] == false {
                assert!(app.object_prompt.is_some());
                for id in &keys {
                    app.apply_selection_click(SelectionClick {
                        object_id: Some(*id),
                        mode: SelectionMode::Add,
                    });
                }
                enter(&mut app, "");
            }
            if let Some(answer) = op["open_confirmation"].as_str() {
                assert!(app.object_prompt.is_some());
                assert!(app.viewport_object_filter().is_none());
                if answer == "escape" {
                    assert!(app.answer_object_prompt_escape());
                } else {
                    enter(&mut app, if answer == "yes" { "Yes" } else { "No" });
                }
            }
            assert!(app.object_prompt.is_none());
            check(&app, "after");
            for source in &sources {
                assert_eq!(app.document.object(source.id()), Some(source));
            }
            let marker = command == "VolumeCentroid" && v["success"] == true;
            if marker {
                assert_eq!(app.document.undo_label(), Some("VolumeCentroid"));
                enter(&mut app, "Undo");
                check(&app, "undo");
                enter(&mut app, "Redo");
                check(&app, "redo");
            } else {
                assert!(!app.document.can_undo());
                assert!(app.document.can_redo());
                if let Some(expected) = native::value(v) {
                    let actual = app
                        .command_log
                        .iter()
                        .rev()
                        .find_map(|s| {
                            s.split_once("total volume ")
                                .and_then(|(_, s)| s.split_whitespace().next())
                                .and_then(|s| s.parse::<f64>().ok())
                        })
                        .unwrap();
                    assert!((actual - expected).abs() < 1e-6);
                }
                enter(&mut app, "Redo");
                check(&app, "redo");
                enter(&mut app, "Undo");
                check(&app, "undo");
            }
        }
    }
    assert_eq!(count, 6);
}
