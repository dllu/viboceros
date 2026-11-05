//! Native SubD controls through command entry, selection prompts and live Dir.
use super::*;
use viboceros_oracle::subd_direction::{Fixture, snapshot, source};

fn enter(app: &mut VibocerosApp, text: &str) {
    app.command_input = text.into();
    app.run_command();
}

#[test]
fn subd_direction_native_workflows_through_application_selection_and_inspector() {
    let base = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tools/rhino_oracle");
    let mut count = 0;
    for family in ["subd_direction_discovery", "subd_direction_controls"] {
        let request: serde_json::Value = serde_json::from_slice(
            &std::fs::read(base.join(format!("fixtures/{family}.json"))).unwrap(),
        )
        .unwrap();
        let native = super::oracle_observation::read(&base, family);
        for (op, row) in request["operations"]
            .as_array()
            .unwrap()
            .iter()
            .zip(native["results"].as_array().unwrap())
        {
            count += 1;
            let f: Fixture = serde_json::from_value(op.clone()).unwrap();
            let mut app = test_app();
            let id = app
                .document
                .add_geometry_with_attributes(
                    source(&f.scene, app.document.tolerance()).unwrap(),
                    viboceros_document::ObjectAttributes::on_layer(app.document.current_layer_id())
                        .with_name("Direction source"),
                )
                .unwrap();
            let mut ids = vec![id];
            if f.mixed {
                enter(&mut app, "Line 20,0,0 23,1,0");
                let peer = app.document.objects().last().unwrap().id();
                app.document
                    .set_object_names([(peer, Some("Direction curve".into()))])
                    .unwrap();
                ids.push(peer);
            }
            app.document
                .select_objects_direct(ids.iter().copied(), SelectionMode::Replace)
                .unwrap();
            enter(&mut app, "Group OwnedDirectionGroup");
            if f.flat {
                enter(&mut app, "SubDDisplayToggle");
            }
            app.document.clear_selection();
            app.document.clear_history().unwrap();
            enter(&mut app, "Point 40,50,60");
            enter(&mut app, "Undo");
            if f.preselect {
                app.document
                    .select_objects_direct(ids.iter().copied(), SelectionMode::Add)
                    .unwrap();
            }
            let snap = |app: &VibocerosApp| snapshot(&app.document, &ids).unwrap();
            let before = snap(&app);
            let storage = ids
                .iter()
                .map(|id| {
                    app.document
                        .object(*id)
                        .unwrap()
                        .geometry_snapshot()
                        .clone()
                })
                .collect::<Vec<_>>();
            enter(&mut app, &f.command);
            if !f.preselect {
                assert!(
                    app.object_prompt.is_some(),
                    "{}: {:?}",
                    op["id"],
                    app.command_log
                );
                for &id in &ids {
                    app.apply_selection_click(SelectionClick {
                        object_id: Some(id),
                        mode: SelectionMode::Add,
                    });
                }
                enter(&mut app, "");
            }
            if f.command == "Dir" {
                assert!(
                    app.direction_session.is_some(),
                    "{}: {:?}",
                    op["id"],
                    app.command_log
                );
                assert!(
                    !app.direction_session
                        .as_ref()
                        .unwrap()
                        .overlay()
                        .markers
                        .is_empty()
                );
                for step in &f.steps {
                    enter(&mut app, step);
                    for (id, storage) in ids.iter().zip(&storage) {
                        assert!(storage.shares_storage_with(
                            app.document.object(*id).unwrap().geometry_snapshot()
                        ));
                    }
                    assert!(!app.document.can_undo());
                    assert!(app.document.can_redo());
                }
                if f.cancel {
                    app.cancel_current_prompt_or_selection();
                } else {
                    enter(&mut app, "");
                }
            }
            assert!(
                app.command_line_idle(),
                "{}: {:?}",
                op["id"],
                app.command_log
            );
            let after = snap(&app);
            let changed = row["value"]["changed"] == true;
            assert_eq!(app.document.can_undo(), changed, "{}", op["id"]);
            let (undo, redo) = if changed {
                enter(&mut app, "Undo");
                let undo = snap(&app);
                enter(&mut app, "Redo");
                (undo, snap(&app))
            } else {
                enter(&mut app, "Redo");
                let redo = snap(&app);
                enter(&mut app, "Undo");
                (snap(&app), redo)
            };
            let actual = serde_json::json!({"before":before,"after":after,"undo":undo,"redo":redo});
            for phase in ["before", "after", "undo", "redo"] {
                super::subd_direction_support::compare(
                    &actual[phase],
                    &row["value"][phase],
                    &format!("{}/{phase}", op["id"]),
                );
            }
        }
    }
    assert_eq!(count, 82);
}
