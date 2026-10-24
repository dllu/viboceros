//! Both routes into the shared temporary getter replay native range scenes.
use super::*;
use crate::app::tests::{point, test_app};
use serde_json::json;

fn enter(app: &mut VibocerosApp, input: &str) {
    app.command_input = input.into();
    app.run_command();
}
#[test]
fn extract_point_temporary_range_getters_replay_native_geometry_and_history() {
    for row in native::range_rows() {
        for inline in [true, false] {
            let mut app = test_app();
            let (doc, ids) = native::setup(&row);
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
            app.document.clear_selection();
            let base = native::command(&row);
            if inline {
                enter(&mut app, &format!("{base} SubCrv"));
                app.document
                    .select_objects_direct(whole, SelectionMode::Add)
                    .unwrap();
            } else {
                enter(&mut app, &base);
                assert!(
                    app.object_prompt.is_some(),
                    "{} {:?}",
                    row["id"],
                    app.command_log
                );
                app.document
                    .select_objects_direct(whole, SelectionMode::Add)
                    .unwrap();
                enter(&mut app, "SubCrv");
            }
            assert!(
                app.intersection_prompt.is_some(),
                "{} {:?}",
                row["id"],
                app.command_log
            );
            for (index, range) in row["value"]["getter"]["points"]
                .as_array()
                .unwrap()
                .iter()
                .enumerate()
            {
                if index > 0 {
                    enter(&mut app, "SubCrv");
                }
                app.select_intersection_prompt_objects(
                    [ids[range[0].as_u64().unwrap() as usize]],
                    SelectionMode::Add,
                );
                for p in [&range[1], &range[2]] {
                    let values = p.as_array().unwrap();
                    assert_eq!(
                        app.accept_uv_subcurve_point(point(
                            values[0].as_f64().unwrap(),
                            values[1].as_f64().unwrap(),
                            values[2].as_f64().unwrap()
                        )),
                        Some(true)
                    );
                }
            }
            if row["value"]["input"]["range_grips"] == true {
                enter(&mut app, "SelPt");
            }
            enter(&mut app, "");
            assert!(
                app.command_line_idle(),
                "{} {:?}",
                row["id"],
                app.command_log
            );
            assert_eq!(app.document.undo_label(), Some("ExtractPt"));
            let mut stages = Vec::new();
            for stage in ["after", "undo", "redo"] {
                if stage != "after" {
                    enter(&mut app, if stage == "undo" { "Undo" } else { "Redo" });
                }
                native::compare(&app.document, &ids, &row["value"][stage]);
                for (id, original) in ids.iter().zip(&storage) {
                    assert!(
                        app.document
                            .object(*id)
                            .unwrap()
                            .geometry_snapshot()
                            .shares_storage_with(original)
                    );
                }
                stages.push(native::snapshot(&app.document, &ids));
            }
            println!(
                "EXTRACT_POINTS_RANGE_GETTER {}",
                json!({"id":row["id"],"path":if inline {"inline"} else {"promoted"},
                "success":true,"source_storage_unchanged":true,"after":stages[0],"undo":stages[1],"redo":stages[2]})
            );
        }
    }
}

#[test]
fn extract_point_ranges_cancel_preserve_redo_and_reject_stale_sources() {
    let mut app = test_app();
    enter(&mut app, "Line 0,0,0 10,0,0");
    let source = app.document.objects().next().unwrap().id();
    enter(&mut app, "Line 0,2,0 10,2,0");
    enter(&mut app, "Undo");
    let redo = app.document.redo_label().map(str::to_owned);
    app.document.clear_selection();
    enter(&mut app, "ExtractPt SubCrv");
    app.select_intersection_prompt_objects([source], SelectionMode::Add);
    app.accept_uv_subcurve_point(point(2., 0., 0.));
    app.cancel_current_prompt_or_selection();
    assert_eq!(app.document.objects().len(), 1);
    assert_eq!(app.document.redo_label(), redo.as_deref());
    enter(&mut app, "ExtractPt SubCrv");
    app.select_intersection_prompt_objects([source], SelectionMode::Add);
    for p in [point(2., 0., 0.), point(8., 0., 0.)] {
        app.accept_uv_subcurve_point(p);
    }
    let geometry = Geometry::Line(
        viboceros_geometry::LineSegment::try_new(
            point(0., 1., 0.),
            point(10., 1., 0.),
            app.document.tolerance(),
        )
        .unwrap(),
    );
    app.document
        .replace_object_geometries([(source, geometry)])
        .unwrap();
    let before = format!("{:?}", app.document);
    enter(&mut app, "");
    assert_eq!(format!("{:?}", app.document), before);
    assert!(app.intersection_prompt.is_some());
    assert!(app.command_log.iter().any(|s| s.contains("input changed")));
}

#[test]
fn empty_range_getters_exit_without_consuming_redo() {
    let mut app = test_app();
    enter(&mut app, "Line 0,0,0 10,0,0");
    enter(&mut app, "Undo");
    let redo = app.document.redo_label().map(str::to_owned);
    enter(&mut app, "ExtractPt SubCrv");
    enter(&mut app, "");
    enter(&mut app, "");
    assert!(app.command_line_idle());
    assert_eq!(app.document.redo_label(), redo.as_deref());
}

#[test]
fn pending_range_rejects_a_source_changed_between_endpoint_picks() {
    let mut app = test_app();
    enter(&mut app, "Line 0,0,0 10,0,0");
    let id = app.document.objects().next().unwrap().id();
    app.document.clear_selection();
    enter(&mut app, "ExtractPt SubCrv");
    app.select_intersection_prompt_objects([id], SelectionMode::Add);
    assert_eq!(app.accept_uv_subcurve_point(point(2., 0., 0.)), Some(true));
    let line = viboceros_geometry::LineSegment::try_new(
        point(0., 1., 0.),
        point(10., 1., 0.),
        app.document.tolerance(),
    )
    .unwrap();
    app.document
        .replace_object_geometries([(id, Geometry::Line(line))])
        .unwrap();
    let before = format!("{:?}", app.document);
    assert_eq!(app.accept_uv_subcurve_point(point(8., 1., 0.)), Some(false));
    assert_eq!(format!("{:?}", app.document), before);
    assert!(app.command_log.iter().any(|s| s.contains("input changed")));
}
