use super::native_curve_comparison;
use super::*;
#[path = "../../../crates/viboceros-command/tests/support/delete_subcurve.rs"]
mod comparison;
fn enter(app: &mut VibocerosApp, text: &str) {
    app.command_input = text.into();
    app.run_command();
}
fn p(value: &serde_json::Value) -> Point3 {
    Point3::try_from(serde_json::from_value::<[f64; 3]>(value.clone()).unwrap()).unwrap()
}
#[test]
fn delete_subcurve_sources_points_lengths_and_history_replay_native_commands() {
    let data: serde_json::Value = serde_json::from_str(include_str!(
        "../../../tools/rhino_oracle/observations/delete_subcurve.json"
    ))
    .unwrap();
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tools/rhino_oracle/fixtures/delete_subcurve");
    let mut failures = Vec::new();
    for row in data["results"].as_array().unwrap() {
        let name = row["id"].as_str().unwrap();
        let v = &row["value"];
        let outcome = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            let mut app = test_app();
            app.document = viboceros_command::open_3dm_with_views(
                root.join(format!("{name}.3dm")).to_str().unwrap(),
            )
            .unwrap()
            .0;
            if let Some(abs) = v["document_tolerance"]["absolute"].as_f64() {
                assert_eq!(app.document.tolerance().absolute(), abs);
            }
            let source = v["source_id"]
                .as_str()
                .unwrap()
                .parse::<ObjectId>()
                .unwrap();
            app.document.clear_selection();
            if v["recipe"]["preselect"] != false {
                app.document
                    .select_objects_direct([source], SelectionMode::Replace)
                    .unwrap();
            }
            app.document.clear_history().unwrap();
            let original = app.document.object(source).unwrap().clone();
            enter(&mut app, "DeleteSubCrv");
            assert!(app.subcurve_prompt.as_ref().unwrap().delete);
            if v["recipe"]["preselect"] == false {
                app.apply_selection_click(SelectionClick {
                    object_id: Some(source),
                    mode: SelectionMode::Replace,
                });
            }
            assert!(app.accept_drafting_point(p(&v["start_point"])));
            assert_eq!(
                app.document.objects().cloned().collect::<Vec<_>>(),
                vec![original.clone()]
            );
            assert!(!app.document.can_undo());
            if let Some(length) = v["length"].as_f64() {
                enter(&mut app, &length.to_string());
            }
            if app.subcurve_prompt.is_some() {
                if v["recipe"]["confirmation"] == false {
                    enter(&mut app, "");
                } else {
                    app.accept_drafting_point(p(&v["end_point"]));
                }
            }
            assert!(app.command_line_idle(), "{name}: {:?}", app.command_log);
            let mut mapping = std::collections::BTreeMap::from([(source.to_string(), source)]);
            let mut maximum =
                comparison::check(&app.document, &v["after"], &original, name, &mut mapping);
            let after = app.document.objects().cloned().collect::<Vec<_>>();
            if v["changed"] == true {
                assert_eq!(app.document.undo_label(), Some("DeleteSubCrv"));
                enter(&mut app, "Undo");
                assert_eq!(
                    app.document.objects().cloned().collect::<Vec<_>>(),
                    vec![original.clone()]
                );
                maximum = maximum.max(comparison::check(
                    &app.document,
                    &v["undo"],
                    &original,
                    name,
                    &mut mapping,
                ));
                assert!(!app.document.can_undo());
                enter(&mut app, "Redo");
                assert_eq!(app.document.objects().cloned().collect::<Vec<_>>(), after);
                maximum = maximum.max(comparison::check(
                    &app.document,
                    &v["redo"],
                    &original,
                    name,
                    &mut mapping,
                ));
            } else {
                assert!(!app.document.can_undo());
            }
            assert!(maximum <= 1e-6, "{name}: {maximum}");
        }));
        if outcome.is_err() {
            failures.push(name.to_owned());
        }
    }
    assert!(failures.is_empty(), "UI native mismatches: {failures:?}");
}
#[test]
fn deletion_getter_cancellation_stale_sources_alias_and_options_are_isolated() {
    let mut app = test_app();
    app.execute_command("Line 0,0,0 10,0,0");
    let source = app.document.objects().next().unwrap().id();
    app.document
        .select_objects_direct([source], SelectionMode::Replace)
        .unwrap();
    app.document.clear_history().unwrap();
    app.execute_command("SetObjectName Future");
    app.execute_command("Undo");
    let before = app.document.objects().cloned().collect::<Vec<_>>();
    enter(&mut app, "DeleteSubCrv");
    assert!(!app.picking_subcurve_edge());
    assert_eq!(app.active_command.unwrap().name(), "DeleteSubCrv");
    enter(&mut app, "Copy=Yes");
    enter(&mut app, "Mode=MarkEnds");
    enter(&mut app, "FromMidpoint=Yes");
    assert_eq!(app.commands.subcurve_defaults(), Default::default());
    assert!(app.document.can_redo());
    app.accept_drafting_point(point(2., 0., 0.));
    app.cancel_current_prompt_or_selection();
    assert!(app.command_line_idle());
    assert_eq!(app.document.objects().cloned().collect::<Vec<_>>(), before);
    assert!(app.document.can_redo());
    app.document.select_command_results([source]).unwrap();
    enter(&mut app, "Break");
    app.accept_drafting_point(point(2., 0., 0.));
    app.document
        .replace_object_geometries([(
            source,
            Geometry::Line(
                viboceros_geometry::LineSegment::try_new(
                    point(0., 0., 0.),
                    point(8., 0., 0.),
                    app.document.tolerance(),
                )
                .unwrap(),
            ),
        )])
        .unwrap();
    assert!(!app.validate_subcurve_source());
    assert!(app.subcurve_prompt.as_ref().unwrap().source.is_none());
    app.cancel_current_prompt_or_selection();
    app.document.select_command_results([source]).unwrap();
    enter(&mut app, "Break");
    app.accept_drafting_point(point(2., 0., 0.));
    enter(&mut app, "Point 4,4,0");
    assert!(app.subcurve_prompt.is_none());
    assert!(app.command_line_idle());
}
