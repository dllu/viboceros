use super::polyline_closure_reference as native;
use super::*;
use crate::app::extract_points::native as grip_native;

fn enter(app: &mut VibocerosApp, input: &str) {
    app.command_input = input.into();
    app.run_command();
}

#[test]
fn extract_points_getter_filters_picks_accepts_choices_and_groups_history() {
    let mut app = test_app();
    enter(&mut app, "Layer New Input");
    enter(&mut app, "Line 0,0,0 1,2,3");
    let source = app.document.objects().next().unwrap().id();
    let original = app.document.object(source).unwrap().clone();
    let ignored = app
        .document
        .add_geometry(Geometry::Point(point(9., 9., 9.)))
        .unwrap();
    enter(&mut app, "Layer New Output");
    let output_layer = app.document.current_layer_id();
    app.document.clear_history().unwrap();
    enter(&mut app, "ExtractPt");
    assert!(app.object_prompt.is_some());
    assert_eq!(
        app.viewport_object_filter(),
        Some(viboceros_command::ObjectSelectionFilter::ExtractPoints)
    );
    app.apply_selection_click(SelectionClick {
        object_id: Some(ignored),
        mode: SelectionMode::Replace,
    });
    assert!(!app.document.is_selected(ignored));
    enter(&mut app, "OutputLayer");
    enter(&mut app, "Current");
    enter(&mut app, "Output=PointCloud");
    app.apply_selection_click(SelectionClick {
        object_id: Some(source),
        mode: SelectionMode::Replace,
    });
    assert!(app.document.is_selected(source));
    enter(&mut app, "");
    assert!(app.command_line_idle(), "{:?}", app.command_log);
    assert!(app.object_prompt.is_none());
    assert_eq!(app.document.object(source), Some(&original));
    let output = app.document.selected_objects().next().unwrap();
    assert_eq!(output.attributes().layer_id(), output_layer);
    let Geometry::PointCloud(cloud) = output.geometry() else {
        panic!()
    };
    assert_eq!(cloud.points(), [point(0., 0., 0.), point(1., 2., 3.)]);
    assert_eq!(app.document.undo_label(), Some("ExtractPt"));
    enter(&mut app, "Undo");
    assert_eq!(app.document.objects().len(), 2);
    enter(&mut app, "Redo");
    assert_eq!(app.document.objects().len(), 3);
    assert_eq!(app.document.object(source), Some(&original));
}

#[test]
fn extract_points_getter_cancellation_preserves_redo_and_resets_options() {
    for finish in ["Cancel", ""] {
        let mut app = test_app();
        enter(&mut app, "Line 0,0 1,0");
        let source = app.document.objects().next().unwrap().id();
        app.document.clear_history().unwrap();
        enter(&mut app, "Point 9,9,9");
        enter(&mut app, "Undo");
        let before = format!("{:?}", app.document);
        enter(&mut app, "ExtractPt");
        enter(&mut app, "Output=PointCloud OutputLayer=Current");
        let prompt = app.object_prompt.as_ref().unwrap().description.clone();
        enter(&mut app, "Output=Invalid");
        assert_eq!(app.object_prompt.as_ref().unwrap().description, prompt);
        enter(&mut app, finish);
        assert!(app.object_prompt.is_none());
        assert!(app.command_line_idle());
        assert_eq!(format!("{:?}", app.document), before);
        assert!(app.document.can_redo());
        enter(&mut app, "ExtractPt");
        let choices = &app.object_prompt.as_ref().unwrap().description.choices;
        assert_eq!((choices[0].value, choices[1].value), ("Input", "Points"));
        app.select_prompt_objects(vec![source], SelectionMode::Replace);
        enter(&mut app, "");
        assert_eq!(app.document.selected_object_count(), 2);
        assert!(
            app.document
                .selected_objects()
                .all(|o| matches!(o.geometry(), Geometry::Point(_)))
        );
    }
}

#[test]
fn extract_points_getters_replay_frozen_native_curve_locations() {
    let capture = native::capture();
    let original = viboceros_command::open_3dm_with_views(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/tools/rhino_oracle/fixtures/polyline_closure.3dm",
    ))
    .unwrap()
    .0;
    let mut count = 0;
    for row in capture["results"][0]["value"]["rows"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|r| r.get("extract").is_some())
    {
        let id: ObjectId = row["id"].as_str().unwrap().parse().unwrap();
        let expected = serde_json::Value::Array(
            row["extract"]["value"]
                .as_array()
                .unwrap()
                .iter()
                .map(|o| o["point"].clone())
                .collect(),
        );
        for preselect in [false, true] {
            for output in ["Points", "PointCloud"] {
                let mut app = test_app();
                app.document = original.clone();
                app.document.clear_selection();
                app.document.clear_history().unwrap();
                let source = app.document.object(id).unwrap().clone();
                let sources = app.document.objects().len();
                if preselect {
                    app.document
                        .select_objects_direct([id], SelectionMode::Replace)
                        .unwrap();
                }
                enter(&mut app, &format!("ExtractPt Output={output}"));
                if !preselect {
                    assert!(app.object_prompt.is_some());
                    app.select_prompt_objects(vec![id], SelectionMode::Replace);
                    enter(&mut app, "");
                }
                assert!(app.command_line_idle(), "{:?}", app.command_log);
                let points = app
                    .document
                    .objects()
                    .skip(sources)
                    .flat_map(|o| match o.geometry() {
                        Geometry::Point(p) => vec![*p],
                        Geometry::PointCloud(c) => c.points().to_vec(),
                        _ => panic!("unexpected extracted geometry"),
                    })
                    .collect::<Vec<_>>();
                let label = row["recipe"]["id"].as_str().unwrap();
                assert!(native::compare_points(&points, &expected, label, false) <= 1e-8);
                assert_eq!(app.document.object(id), Some(&source));
                assert_eq!(app.document.undo_label(), Some("ExtractPt"));
                let outputs = app
                    .document
                    .objects()
                    .skip(sources)
                    .cloned()
                    .collect::<Vec<_>>();
                enter(&mut app, "Undo");
                assert_eq!(app.document.objects().len(), sources);
                assert_eq!(app.document.object(id), Some(&source));
                enter(&mut app, "Redo");
                assert_eq!(
                    app.document
                        .objects()
                        .skip(sources)
                        .cloned()
                        .collect::<Vec<_>>(),
                    outputs
                );
                count += 1;
            }
        }
    }
    assert_eq!(count, 176);
}

#[test]
fn extract_points_getters_replay_native_selected_control_and_edit_grips() {
    let rows = grip_native::rows();
    assert_eq!(rows.len(), 63);
    for row in rows {
        let preselected = row["path"] == "preselected";
        let (doc, ids) = grip_native::setup(&row);
        let mut app = test_app();
        app.document = doc;
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
        let whole = app.document.selected_object_ids().collect::<Vec<_>>();
        let grips = app
            .document
            .selected_control_points()
            .map(|(g, _)| g)
            .collect::<Vec<_>>();
        if !preselected {
            app.document.clear_selection();
        }
        enter(&mut app, &grip_native::command(&row));
        if !preselected {
            assert!(app.object_prompt.is_some(), "{:?}", app.command_log);
            assert!(app.control_point_picking_available());
            app.document
                .select_objects_direct(whole, SelectionMode::Add)
                .unwrap();
            assert!(app.handle_viewport_action(crate::viewport::ViewportOutput {
                control_point_selection: Some(crate::viewport::ControlPointSelection {
                    picks: grips,
                    mode: SelectionMode::Replace
                }),
                ..Default::default()
            }));
            enter(&mut app, "");
        }
        assert!(app.command_line_idle(), "{:?}", app.command_log);
        grip_native::compare(&app.document, &ids, &row["value"]["after"]);
        let after = grip_native::snapshot(&app.document, &ids);
        enter(&mut app, "Undo");
        grip_native::compare(&app.document, &ids, &row["value"]["undo"]);
        let undo = grip_native::snapshot(&app.document, &ids);
        enter(&mut app, "Redo");
        grip_native::compare(&app.document, &ids, &row["value"]["redo"]);
        for (id, original) in ids.iter().zip(&storage) {
            assert!(
                app.document
                    .object(*id)
                    .unwrap()
                    .geometry_snapshot()
                    .shares_storage_with(original)
            );
        }
        println!(
            "EXTRACT_POINTS_GRIPS_GETTER {}",
            serde_json::json!({"id":row["id"],
                "path":if preselected { "preselected" } else { "picked" },"success":true,"source_storage_unchanged":true,
                "after":after,"undo":undo,"redo":grip_native::snapshot(&app.document, &ids)})
        );
    }
}
