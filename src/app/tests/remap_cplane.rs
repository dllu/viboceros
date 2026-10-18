use super::grip_transform::{compare, geometry};
use super::scale_by_plane::{frame, p};
use super::*;
use serde_json::{Value, json};
use viboceros_document::{ControlPointId, ObjectAttributes};
use viboceros_geometry::{PointCloud3, Vector3};

fn enter(app: &mut VibocerosApp, text: &str) {
    app.command_input = text.into();
    app.run_command();
}

fn normalized(mut value: Value) -> Value {
    for group in value["groups"].as_array_mut().unwrap() {
        let name = group["name"].as_str().unwrap();
        if name != "Owned remap group" {
            assert!(
                name.strip_prefix("Group")
                    .is_some_and(|n| !n.is_empty() && n.bytes().all(|c| c.is_ascii_digit()))
            );
            group["name"] = json!("generated");
        }
    }
    value
}

fn snapshot(app: &VibocerosApp, sources: &[ObjectId], points: bool) -> Value {
    let groups = app.document.groups().collect::<Vec<_>>();
    let mut rows = if points {
        json!(app.document.objects().map(|o| {
            let Geometry::PointCloud(cloud) = o.geometry() else { panic!("remap point cloud"); };
            json!({"points":cloud.points().iter().map(|p|p.to_array()).collect::<Vec<_>>(),
                   "name":o.attributes().name(),"selected":app.document.is_selected(o.id()),"grips_on":false,"grips":[]})
        }).collect::<Vec<_>>())
    } else {
        super::grip_transform::snapshot(app, sources[0], None)
    };
    for (row, object) in rows
        .as_array_mut()
        .unwrap()
        .iter_mut()
        .zip(app.document.objects())
    {
        row.as_object_mut().unwrap().remove("role");
        row.as_object_mut().unwrap().remove("kind");
        row["source"] = json!(sources.iter().position(|id| *id == object.id()));
        row["groups"] = json!(
            object
                .group_ids()
                .iter()
                .map(|id| groups.iter().position(|g| g.id() == *id).unwrap())
                .collect::<Vec<_>>()
        );
    }
    let plane = app.viewports[app.active_viewport].construction_plane();
    json!({"objects":rows,"groups":groups.iter().map(|g|json!({"name":g.name(),"count":g.members().len()})).collect::<Vec<_>>(),
           "plane":{"origin":plane.origin().to_array(),"x_axis":plane.x_axis().as_vector().to_array(),"y_axis":plane.y_axis().as_vector().to_array()}})
}

fn setup(app: &mut VibocerosApp, op: &Value, native: &Value) -> Vec<ObjectId> {
    app.active_viewport = 0;
    app.viewports[0].set_construction_plane(frame(&native["before"]["plane"]));
    let target = Frame3::try_from_directions(
        point(-3., 7., 11.),
        Vector3::try_new(0., 1., 1.).unwrap(),
        Vector3::try_new(2., 1., -1.).unwrap(),
        Tolerance::DEFAULT,
    )
    .unwrap();
    app.viewports[1].set_construction_plane(target);
    app.active_viewport = 1;
    if op["mode"] == "CPlane" {
        assert!(app.try_run_named_cplane_command("NamedCPlane Save Owned target"));
    } else {
        assert!(app.try_run_named_view_command("NamedView Save Owned target"));
    }
    app.active_viewport = 0;
    let mut ids = Vec::new();
    for row in native["before"]["objects"].as_array().unwrap() {
        let geometry = if op["source"] == "points" {
            Geometry::PointCloud(
                PointCloud3::try_new(row["points"].as_array().unwrap().iter().map(p).collect())
                    .unwrap(),
            )
        } else {
            geometry(op, row)
        };
        let id = app
            .document
            .add_geometry_with_attributes(
                geometry,
                ObjectAttributes::on_layer(app.document.current_layer_id())
                    .with_name(row["name"].as_str().unwrap()),
            )
            .unwrap();
        if row["grips_on"] == true {
            app.document.enable_control_points([id]).unwrap();
            app.document
                .select_control_points(
                    row["grips"]
                        .as_array()
                        .unwrap()
                        .iter()
                        .filter(|g| g["selected"] == true)
                        .map(|g| ControlPointId {
                            object: id,
                            index: g["index"].as_u64().unwrap() as usize,
                        }),
                    SelectionMode::Add,
                )
                .unwrap();
        }
        if row["selected"] == true {
            app.document
                .select_objects_direct([id], SelectionMode::Add)
                .unwrap();
        }
        ids.push(id);
    }
    if op["group"] == true {
        app.document
            .add_group(Some("Owned remap group".into()), ids.iter().copied())
            .unwrap();
    }
    app.document.clear_history().unwrap();
    ids
}

#[test]
fn remap_cplane_replays_38_native_commands_through_complete_and_incremental_input() {
    let q: Value = serde_json::from_str(include_str!(
        "../../../tools/rhino_oracle/fixtures/remap_cplane.json"
    ))
    .unwrap();
    let r: Value = serde_json::from_str(include_str!(
        "../../../tools/rhino_oracle/observations/remap_cplane.json"
    ))
    .unwrap();
    assert_eq!(q["operations"].as_array().unwrap().len(), 38);
    assert_eq!(r["results"].as_array().unwrap().len(), 38);
    let mut checked = 0;
    for (op, row) in q["operations"]
        .as_array()
        .unwrap()
        .iter()
        .zip(r["results"].as_array().unwrap())
    {
        assert_eq!(op["id"], row["id"]);
        let native = &row["value"];
        assert_eq!(native["success"], true);
        for mode in 0..3 {
            let label = format!("{}/input-{mode}", op["id"]);
            let mut app = test_app();
            let sources = setup(&mut app, op, native);
            let points = op["source"] == "points";
            compare(
                &normalized(snapshot(&app, &sources, points)),
                &normalized(native["before"].clone()),
                &label,
            );
            let copy = if op["copy"] == true { "Yes" } else { "No" };
            let target = format!(
                "{} \"{}\"",
                op["mode"].as_str().unwrap(),
                op["target"].as_str().unwrap()
            );
            enter(
                &mut app,
                &if mode == 0 {
                    format!("RemapCPlane Copy={copy} {target}")
                } else {
                    format!("RemapCPlane Copy={copy}")
                },
            );
            if op["postselected"] == true {
                assert!(app.object_prompt.is_some(), "{label}");
                for id in &sources {
                    enter(&mut app, &format!("SelID {id}"));
                }
                enter(&mut app, "");
            }
            if mode == 1 {
                enter(&mut app, &target);
            }
            if mode == 2 {
                enter(&mut app, op["mode"].as_str().unwrap());
                enter(&mut app, &format!("\"{}\"", op["target"].as_str().unwrap()));
            }
            if let Some(retry) = op["retry_target"].as_str() {
                assert!(app.remap_cplane_session.is_some(), "{label}");
                assert!(!app.document.can_undo(), "{label}");
                enter(&mut app, retry);
            }
            assert!(
                app.active_command.is_none(),
                "{label}: {:?}",
                app.command_log
            );
            assert!(app.remap_cplane_session.is_none(), "{label}");
            compare(
                &normalized(snapshot(&app, &sources, points)),
                &normalized(native["after"].clone()),
                &format!("{label}/after"),
            );
            assert_eq!(app.document.undo_label(), Some("RemapCPlane"));
            app.execute_command("Undo");
            compare(
                &normalized(snapshot(&app, &sources, points)),
                &normalized(native["undo"].clone()),
                &format!("{label}/undo"),
            );
            app.execute_command("Redo");
            compare(
                &normalized(snapshot(&app, &sources, points)),
                &normalized(native["redo"].clone()),
                &format!("{label}/redo"),
            );
            checked += 1;
        }
    }
    assert_eq!(checked, 114);
    eprintln!("REMAP_CPLANE_NATIVE cases={checked}");
}

#[test]
fn remap_viewport_pick_keeps_the_source_plane_and_ignores_object_selection() {
    let mut app = test_app();
    app.active_viewport = 0;
    let source = app
        .document
        .add_geometry(Geometry::Point(point(2., 3., 4.)))
        .unwrap();
    app.document
        .select_objects_direct([source], SelectionMode::Replace)
        .unwrap();
    app.document.clear_history().unwrap();
    let target = Frame3::try_from_directions(
        point(10., 20., 30.),
        Vector3::try_new(0., 1., 0.).unwrap(),
        Vector3::try_new(0., 0., 1.).unwrap(),
        Tolerance::DEFAULT,
    )
    .unwrap();
    app.viewports[2].set_construction_plane(target);
    let cameras = app
        .viewports
        .iter()
        .map(|v| v.camera_snapshot())
        .collect::<Vec<_>>();
    enter(&mut app, "RemapCPlane");
    app.apply_selection_click(SelectionClick {
        object_id: None,
        mode: SelectionMode::Replace,
    });
    assert!(app.document.is_selected(source));
    // A transparent plane edit must not change the captured source frame.
    assert!(app.try_run_plane_command("'CPlane World Front"));
    assert!(app.remap_cplane_session.is_some());
    app.active_viewport = 2;
    assert!(app.handle_viewport_action(ViewportOutput {
        source_viewport_click: true,
        selection_click: Some(SelectionClick {
            object_id: None,
            mode: SelectionMode::Replace
        }),
        ..Default::default()
    }));
    let Geometry::Point(actual) = app.document.object(source).unwrap().geometry() else {
        panic!()
    };
    assert_eq!(*actual, point(14., 22., 33.));
    assert_eq!(app.viewports[2].construction_plane(), target);
    assert_eq!(
        app.viewports
            .iter()
            .map(|v| v.camera_snapshot())
            .collect::<Vec<_>>(),
        cameras
    );
    assert_eq!(app.document.undo_label(), Some("RemapCPlane"));
    app.document.undo().unwrap();
    assert_eq!(
        app.document.object(source).unwrap().geometry(),
        &Geometry::Point(point(2., 3., 4.))
    );
}

#[test]
fn remap_cplane_missing_names_and_cancellation_preserve_redo_and_copy_defaults() {
    let mut app = test_app();
    app.active_viewport = 0;
    let id = app
        .document
        .add_geometry(Geometry::Point(point(1., 2., 3.)))
        .unwrap();
    app.document
        .select_objects_direct([id], SelectionMode::Replace)
        .unwrap();
    app.document
        .add_geometry(Geometry::Point(point(4., 5., 6.)))
        .unwrap();
    app.document.undo().unwrap();
    let before = app.document.objects().cloned().collect::<Vec<_>>();
    enter(&mut app, "RemapCPlane Copy=Yes");
    for text in [
        "CPlane=missing",
        "View=missing",
        "Copy=maybe",
        "TargetFrame=NaN,0,0;1,0,0;0,1,0",
    ] {
        enter(&mut app, text);
        assert!(app.remap_cplane_session.is_some());
        assert_eq!(app.document.objects().cloned().collect::<Vec<_>>(), before);
        assert!(app.document.can_redo());
    }
    app.cancel_current_prompt_or_selection();
    assert!(app.remap_cplane_session.is_none());
    assert_eq!(app.document.objects().cloned().collect::<Vec<_>>(), before);
    assert!(app.document.is_selected(id));
    assert!(app.document.can_redo());
    assert_eq!(app.commands.copy_default("RemapCPlane"), Some(false));
    for selector in ["CPlane", "View"] {
        enter(&mut app, &format!("RemapCPlane {selector}=missing"));
        assert!(app.remap_cplane_session.is_some());
        assert_eq!(app.document.objects().cloned().collect::<Vec<_>>(), before);
        assert!(app.document.can_redo());
        enter(&mut app, "Front");
        assert!(app.remap_cplane_session.is_none());
        assert_eq!(
            app.document.object(id).unwrap().geometry(),
            &Geometry::Point(point(1., -3., 2.))
        );
        app.document.undo().unwrap();
        assert_eq!(app.document.objects().cloned().collect::<Vec<_>>(), before);
    }
}

#[test]
fn remap_cplane_replays_native_copy_option_cancellation() {
    let q: Value = serde_json::from_str(include_str!(
        "../../../tools/rhino_oracle/fixtures/remap_cplane_copy.json"
    ))
    .unwrap();
    let r: Value = serde_json::from_str(include_str!(
        "../../../tools/rhino_oracle/observations/remap_cplane_copy.json"
    ))
    .unwrap();
    assert_eq!(q["operations"].as_array().unwrap().len(), 2);
    assert_eq!(r["results"].as_array().unwrap().len(), 2);
    for (op, row) in q["operations"]
        .as_array()
        .unwrap()
        .iter()
        .zip(r["results"].as_array().unwrap())
    {
        assert_eq!(op["id"], row["id"]);
        let native = &row["value"];
        let label = op["id"].as_str().unwrap();
        let mut app = test_app();
        let sources = setup(&mut app, op, native);
        enter(&mut app, "RemapCPlane");
        enter(
            &mut app,
            if op["setup_copy"] == true {
                "Copy=Yes"
            } else {
                "Copy=No"
            },
        );
        enter(&mut app, "Cancel");
        compare(
            &normalized(snapshot(&app, &sources, true)),
            &normalized(native["setup"]["after"].clone()),
            label,
        );
        enter(&mut app, "RemapCPlane CPlane Front");
        compare(
            &normalized(snapshot(&app, &sources, true)),
            &normalized(native["after"].clone()),
            label,
        );
        app.execute_command("Undo");
        compare(
            &normalized(snapshot(&app, &sources, true)),
            &normalized(native["undo"].clone()),
            label,
        );
        app.execute_command("Redo");
        compare(
            &normalized(snapshot(&app, &sources, true)),
            &normalized(native["redo"].clone()),
            label,
        );
    }
    eprintln!("REMAP_CPLANE_COPY cases=2");
}
