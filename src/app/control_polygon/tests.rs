//! Temporary-range getter replay against native control vertices and attributes.
use super::*;
use crate::app::tests::{point, test_app};
use serde_json::{Value, json};
fn enter(app: &mut VibocerosApp, input: &str) {
    app.command_input = input.into();
    app.run_command();
}
#[test]
fn control_polygon_temporary_ranges_replay_native_getters_and_history() {
    let captured: Value = serde_json::from_str(include_str!(
        "../../../tools/rhino_oracle/observations/control_polygon_components.json"
    ))
    .unwrap();
    let mut count = 0;
    for row in captured["results"].as_array().unwrap() {
        let v = &row["value"];
        if v["subcurve_parameters"].is_null() {
            continue;
        }
        let id = row["id"].as_str().unwrap();
        for inline in [false, true] {
            let (document, sources) = native::setup("control_polygon_components", id, v, id);
            let mut app = test_app();
            app.document = document;
            let original = app
                .document
                .object(sources[0])
                .unwrap()
                .geometry_snapshot()
                .clone();
            let curve = original.curve_ref().unwrap();
            let p = &v["subcurve_parameters"];
            let points = [
                curve.evaluate(p[0].as_f64().unwrap()).unwrap(),
                curve.evaluate(p[1].as_f64().unwrap()).unwrap(),
            ];
            if inline {
                enter(&mut app, "ExtractControlPolygon SubCrv");
            } else {
                enter(&mut app, "ExtractControlPolygon");
                enter(&mut app, "SubCrv");
            }
            assert!(
                app.intersection_prompt
                    .as_ref()
                    .unwrap()
                    .control_polygon
                    .is_some()
            );
            app.select_intersection_prompt_objects([sources[0]], SelectionMode::Add);
            for point in points {
                assert_eq!(
                    app.accept_uv_subcurve_point(point),
                    Some(true),
                    "{id}: {:?}",
                    app.command_log
                );
            }
            assert_eq!(app.document.objects().len(), 1);
            assert!(app.document.undo_label().is_none());
            enter(&mut app, "");
            assert!(
                app.intersection_prompt.is_none(),
                "{id}: {:?}",
                app.command_log
            );
            let error = native::compare(&app.document, &sources, &v["after"], id);
            assert_eq!(
                app.document.object(sources[0]).unwrap().geometry_snapshot(),
                &original
            );
            enter(&mut app, "Undo");
            native::compare(&app.document, &sources, &v["undo"], id);
            enter(&mut app, "Redo");
            native::compare(&app.document, &sources, &v["redo"], id);
            println!(
                "CONTROL_POLYGON_RANGE_UI {}",
                json!({"id":id,"inline":inline,"maximum_error":error,"metadata_exact":true,"selection_exact":true,"history_exact":true})
            );
            count += 1;
        }
    }
    assert_eq!(count, 10);
}
#[test]
fn control_polygon_temporary_ranges_cancel_preserve_redo_and_reject_stale_sources() {
    let mut app = test_app();
    enter(&mut app, "Line 0,0,0 10,0,0");
    let source = app.document.objects().next().unwrap().id();
    enter(&mut app, "Line 0,2,0 10,2,0");
    enter(&mut app, "Undo");
    let redo = app.document.redo_label().map(str::to_owned);
    app.document.clear_selection();
    enter(&mut app, "ExtractControlPolygon SubCrv");
    app.select_intersection_prompt_objects([source], SelectionMode::Add);
    assert_eq!(app.accept_uv_subcurve_point(point(2., 0., 0.)), Some(true));
    app.cancel_current_prompt_or_selection();
    assert_eq!(app.document.objects().len(), 1);
    assert_eq!(app.document.redo_label(), redo.as_deref());
    assert!(app.intersection_prompt.is_none());
    enter(&mut app, "ExtractControlPolygon SubCrv");
    app.select_intersection_prompt_objects([source], SelectionMode::Add);
    for p in [point(2., 0., 0.), point(8., 0., 0.)] {
        app.accept_uv_subcurve_point(p);
    }
    let new_geometry = Geometry::Line(
        viboceros_geometry::LineSegment::try_new(
            point(0., 1., 0.),
            point(10., 1., 0.),
            Tolerance::default(),
        )
        .unwrap(),
    );
    app.document
        .replace_object_geometries([(source, new_geometry)])
        .unwrap();
    let undo = app.document.undo_label().map(str::to_owned);
    enter(&mut app, "");
    assert!(app.intersection_prompt.is_some());
    assert_eq!(app.document.objects().len(), 1);
    assert_eq!(app.document.undo_label(), undo.as_deref());
    assert!(app.command_log.iter().any(|s| s.contains("input changed")));
}

#[test]
fn control_polygon_ignores_native_component_preselection_and_box_face_clicks() {
    use crate::viewport::{ComponentClick, ComponentPick};
    use viboceros_command::ComponentSelectionKind as Kind;
    let mut count = 0;
    for family in ["control_polygon_components", "control_polygon_picks"] {
        let captured: Value = serde_json::from_slice(
            &std::fs::read(
                std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
                    .join(format!("tools/rhino_oracle/observations/{family}.json")),
            )
            .unwrap(),
        )
        .unwrap();
        for row in captured["results"].as_array().unwrap() {
            let v = &row["value"];
            if v["input"]["components"].is_null() && v["input"]["mouse"].is_null() {
                continue;
            }
            // Polycurve segment components do not yet have a UI representation.
            if row["id"] == "segment_polycurve" {
                continue;
            }
            let id = row["id"].as_str().unwrap();
            let (document, sources) = native::setup(family, id, v, id);
            let mut app = test_app();
            app.document = document;
            let original = app
                .document
                .objects()
                .map(|o| (o.id(), o.geometry_snapshot().clone()))
                .collect::<Vec<_>>();
            let picks = v["input"]["components"]
                .as_array()
                .map(|cs| {
                    cs.iter()
                        .map(|c| ComponentPick {
                            object: sources[c[0].as_u64().unwrap() as usize],
                            index: c[2].as_u64().unwrap() as usize,
                            kind: match c[1].as_str().unwrap() {
                                "BrepFace" => Kind::BrepFace,
                                "MeshFace" => Kind::MeshFace,
                                "SubdFace" => Kind::SubDFace,
                                _ => unreachable!(),
                            },
                        })
                        .collect::<Vec<_>>()
                })
                .unwrap_or_default();
            if !picks.is_empty() {
                for pick in &picks {
                    app.accept_component_click(ComponentClick {
                        picks: vec![*pick],
                        preselection: true,
                        modifiers: egui::Modifiers::NONE,
                    });
                }
                assert_eq!(
                    app.component_selection
                        .checked_picks(&app.document)
                        .unwrap()
                        .len(),
                    picks.len()
                );
            }
            enter(&mut app, "ExtractControlPolygon");
            if !v["input"]["mouse"].is_null() {
                app.accept_component_click(ComponentClick {
                    picks: vec![ComponentPick {
                        object: sources[0],
                        index: 5,
                        kind: Kind::BrepFace,
                    }],
                    preselection: false,
                    modifiers: egui::Modifiers::CTRL | egui::Modifiers::SHIFT,
                });
            }
            if app.object_prompt.is_some() {
                enter(&mut app, "");
            }
            assert!(app.object_prompt.is_none(), "{id}: {:?}", app.command_log);
            let error = native::compare(&app.document, &sources, &v["after"], id);
            let actual_picks = app
                .component_selection
                .checked_picks(&app.document)
                .unwrap();
            for (i, object) in app.document.objects().enumerate() {
                let components = actual_picks
                    .iter()
                    .filter(|p| p.object == object.id())
                    .map(|p| {
                        json!([
                            match p.kind {
                                Kind::BrepFace => "BrepFace",
                                Kind::MeshFace => "MeshFace",
                                Kind::SubDFace => "SubdFace",
                                _ => unreachable!(),
                            },
                            p.index
                        ])
                    })
                    .collect::<Vec<_>>();
                assert_eq!(json!(components), v["after"][i]["components"], "{id}");
            }
            for (id, geometry) in original {
                assert_eq!(
                    app.document.object(id).unwrap().geometry_snapshot(),
                    &geometry
                );
            }
            if v["success"] == true {
                enter(&mut app, "Undo");
                native::compare(&app.document, &sources, &v["undo"], id);
                assert!(
                    app.component_selection
                        .checked_picks(&app.document)
                        .unwrap()
                        .is_empty()
                );
                enter(&mut app, "Redo");
                native::compare(&app.document, &sources, &v["redo"], id);
            } else {
                assert!(app.document.undo_label().is_none());
            }
            println!(
                "CONTROL_POLYGON_COMPONENT_UI {}",
                json!({"id":id,"success":v["success"],"maximum_error":error,"component_selection_exact":true,"metadata_exact":true,"selection_exact":true,"history_exact":true})
            );
            count += 1;
        }
    }
    assert_eq!(count, 10);
}
