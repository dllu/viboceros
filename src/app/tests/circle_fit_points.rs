use super::*;
use serde_json::{Value, json};
use viboceros_command::ObjectSelectionFilter;
use viboceros_document::{ObjectId, SelectionMode};
use viboceros_geometry::LineSegment;

fn enter(app: &mut VibocerosApp, input: &str) {
    app.command_input = input.into();
    app.run_command();
}

fn snapshot(app: &VibocerosApp, ids: &[ObjectId]) -> Value {
    Value::Array(
        app.document
            .objects()
            .map(|o| {
                let mut row = json!({
                    "source":ids.iter().position(|&id| id == o.id()),
                    "selected":app.document.is_selected(o.id())
                });
                match o.geometry() {
                    Geometry::Point(p) => {
                        row["kind"] = json!("point");
                        row["point"] = json!(p.to_array());
                    }
                    Geometry::Line(line) => {
                        row["kind"] = json!("line");
                        row["point"] = json!(line.start().to_array());
                        row["end"] = json!(line.end().to_array());
                    }
                    Geometry::Circle(_) => {
                        row["kind"] = json!("circle");
                    }
                    _ => panic!("unexpected fit object"),
                }
                row
            })
            .collect(),
    )
}
fn roles(mut value: Value) -> Value {
    for row in value.as_array_mut().unwrap() {
        let obj = row.as_object_mut().unwrap();
        obj.remove("circle");
        obj.remove("seam");
    }
    value
}

#[test]
fn circle_fit_points_input_replays_eight_native_selection_and_history_workflows() {
    let q: Value = serde_json::from_str(include_str!(
        "../../../tools/rhino_oracle/fixtures/circle_fit_selection.json"
    ))
    .unwrap();
    let r: Value = serde_json::from_str(include_str!(
        "../../../tools/rhino_oracle/observations/circle_fit_selection.json"
    ))
    .unwrap();
    let operations = q["operations"].as_array().unwrap();
    assert_eq!(operations.len(), 8);
    for (op, row) in operations.iter().zip(r["results"].as_array().unwrap()) {
        // Cover both complete CLI invocation and the Circle center option.
        for incremental in [false, true] {
            let mut app = test_app();
            app.document
                .begin_transaction("Circle FitPoints selection sources")
                .unwrap();
            let ids = op["points"]
                .as_array()
                .unwrap()
                .iter()
                .map(|p| {
                    app.document
                        .add_geometry(Geometry::Point(
                            Point3::try_from(
                                serde_json::from_value::<[f64; 3]>(p.clone()).unwrap(),
                            )
                            .unwrap(),
                        ))
                        .unwrap()
                })
                .collect::<Vec<_>>();
            let line = if op["line"] == true {
                Some(
                    app.document
                        .add_geometry(Geometry::Line(
                            LineSegment::try_new(
                                point(10., 0., 0.),
                                point(11., 1., 0.),
                                Tolerance::DEFAULT,
                            )
                            .unwrap(),
                        ))
                        .unwrap(),
                )
            } else {
                None
            };
            app.document.commit_transaction().unwrap();
            let selected = op["preselected"]
                .as_array()
                .unwrap()
                .iter()
                .map(|i| ids[i.as_u64().unwrap() as usize])
                .chain(line)
                .collect::<Vec<_>>();
            app.document
                .select_objects_direct(selected, SelectionMode::Replace)
                .unwrap();
            let native = &row["value"];
            assert_eq!(snapshot(&app, &ids), roles(native["before"].clone()));
            if incremental {
                enter(&mut app, "Circle");
                enter(&mut app, "FitPoints");
            } else {
                enter(&mut app, "_C _FitPoints");
            }
            if app.object_prompt.is_some() {
                assert_eq!(
                    app.viewport_object_filter(),
                    Some(ObjectSelectionFilter::Points)
                );
                assert_eq!(app.document.selected_object_count(), 0);
                if op["inputs"] == "all" {
                    enter(&mut app, "SelAll");
                    assert_eq!(app.document.selected_object_count(), ids.len());
                    if let Some(line) = line {
                        assert!(!app.document.is_selected(line));
                    }
                }
                if op["inputs"] != "auto" {
                    enter(&mut app, "");
                }
            }
            if app.object_prompt.is_some() {
                app.cancel_interactive_command(true);
            }
            assert!(app.active_command.is_none());
            assert_eq!(
                snapshot(&app, &ids),
                roles(native["after_script"].clone()),
                "{} {incremental}: {:?}",
                op["id"],
                app.command_log
            );
            enter(&mut app, "Undo");
            assert_eq!(
                snapshot(&app, &ids),
                roles(native["undo"].clone()),
                "{}",
                op["id"]
            );
            enter(&mut app, "Redo");
            assert_eq!(
                snapshot(&app, &ids),
                roles(native["redo"].clone()),
                "{}",
                op["id"]
            );
        }
    }
}

#[test]
fn circle_fit_points_cancelled_and_insufficient_picks_preserve_geometry_and_redo() {
    let mut app = test_app();
    for p in ["2,0,0", "0,2,0", "-2,0,0"] {
        enter(&mut app, &format!("Point {p}"));
    }
    enter(&mut app, "Circle 0,0,0 3");
    enter(&mut app, "Undo");
    let ids = app.document.objects().map(|o| o.id()).collect::<Vec<_>>();
    let before = app.document.objects().cloned().collect::<Vec<_>>();
    enter(&mut app, "Circle FitPoints");
    app.apply_selection_click(SelectionClick {
        object_id: Some(ids[0]),
        mode: SelectionMode::Replace,
    });
    enter(&mut app, "");
    assert!(app.object_prompt.is_some());
    assert_eq!(app.document.objects().cloned().collect::<Vec<_>>(), before);
    app.cancel_interactive_command(true);
    assert_eq!(app.document.selected_object_count(), 0);
    assert_eq!(app.document.objects().cloned().collect::<Vec<_>>(), before);
    enter(&mut app, "Redo");
    assert_eq!(app.document.objects().len(), 4);
}
