use super::grip_transform::{compare, geometry, snapshot};
use super::*;
use serde_json::Value;
use viboceros_document::{ControlPointId, SelectionMode};
use viboceros_geometry::{Brep, NurbsSurface, Vector3};

fn enter(app: &mut VibocerosApp, text: &str) {
    app.command_input = text.into();
    app.run_command();
}
pub(super) fn p(v: &Value) -> Point3 {
    Point3::try_from(serde_json::from_value::<[f64; 3]>(v.clone()).unwrap()).unwrap()
}
pub(super) fn frame(v: &Value) -> Frame3 {
    Frame3::try_from_directions(
        p(&v["origin"]),
        Vector3::try_from(serde_json::from_value::<[f64; 3]>(v["x_axis"].clone()).unwrap())
            .unwrap(),
        Vector3::try_from(serde_json::from_value::<[f64; 3]>(v["y_axis"].clone()).unwrap())
            .unwrap(),
        Tolerance::DEFAULT,
    )
    .unwrap()
}
fn plane_points(v: &Value) -> [String; 3] {
    let f = frame(v);
    [
        f.origin(),
        f.origin().translated(f.x_axis().as_vector()).unwrap(),
        f.origin().translated(f.y_axis().as_vector()).unwrap(),
    ]
    .map(|point| format!("w{}", format_model_point(point)))
}
fn normalized(mut value: Value, rigid_grips: bool) -> Value {
    for row in value.as_array_mut().unwrap() {
        row.as_object_mut().unwrap().remove("center");
        // Rhino retains cursor-dependent temporary grip locations for Rigid,
        // without editing the geometry. These macros prescribe no cursor at
        // this point, so they witness geometry/history, not that display.
        if rigid_grips {
            for grip in row["grips"].as_array_mut().unwrap() {
                grip.as_object_mut().unwrap().remove("point");
            }
        }
    }
    value
}
fn checked(
    app: &VibocerosApp,
    source: ObjectId,
    target: Option<ObjectId>,
    expected: &Value,
    label: &str,
    rigid_grips: bool,
) {
    let mut actual = snapshot(app, source, None);
    if let Some(target) = target
        && let Some(index) = app.document.objects().position(|obj| obj.id() == target)
    {
        actual.as_array_mut().unwrap().remove(index);
    }
    compare(
        &normalized(actual, rigid_grips),
        &normalized(expected.clone(), rigid_grips),
        label,
    );
}

pub(super) fn object_plane_snapshot(
    app: &VibocerosApp,
    sources: &[ObjectId],
    target: ObjectId,
) -> Value {
    serde_json::json!(
        app.document
            .objects()
            .filter(|o| o.id() != target)
            .map(|o| {
                let Geometry::Point(point) = o.geometry() else {
                    panic!("unexpected plane scale source")
                };
                serde_json::json!({"role": if sources.contains(&o.id()) {"source"} else {"output"},
            "name": o.attributes().name(), "selected": app.document.is_selected(o.id()),
            "point": point.to_array()})
            })
            .collect::<Vec<_>>()
    )
}

#[test]
fn scale_by_plane_replays_native_object_frames_and_rejected_targets() {
    let q: Value = serde_json::from_str(include_str!(
        "../../../tools/rhino_oracle/fixtures/scale_by_plane_object.json"
    ))
    .unwrap();
    let r: Value = serde_json::from_str(include_str!(
        "../../../tools/rhino_oracle/observations/scale_by_plane_object.json"
    ))
    .unwrap();
    let operations = q["operations"].as_array().unwrap();
    let results = r["results"].as_array().unwrap();
    assert_eq!(operations.len(), results.len());
    for (op, row) in operations.iter().zip(results) {
        assert_eq!(op["id"], row["id"]);
        let label = op["id"].as_str().unwrap();
        let native = &row["value"];
        let kind = op["target"].as_str().unwrap();
        let accepted = native["object_accepted"].as_bool().unwrap();
        for incremental in [false, true]
            .into_iter()
            .filter(|incremental| accepted || *incremental)
        {
            let mut app = test_app();
            app.active_viewport = 1;
            app.viewports[1].set_construction_plane(frame(&native["active_plane"]));
            app.document
                .begin_transaction("ScaleByPlane Object sources")
                .unwrap();
            let sources: Vec<_> = native["before"]
                .as_array()
                .unwrap()
                .iter()
                .map(|source| {
                    let id = app
                        .document
                        .add_geometry(Geometry::Point(p(&source["point"])))
                        .unwrap();
                    app.document
                        .set_object_names([(id, Some(source["name"].as_str().unwrap().into()))])
                        .unwrap();
                    id
                })
                .collect();
            let target_geometry = if kind == "point" {
                Geometry::Point(p(&native["target"]["point"]))
            } else {
                let source = match kind {
                    "line" | "polyline" | "mesh" => kind,
                    "surface" | "skew_surface" | "warped_surface" => "surface",
                    _ => "rational",
                };
                geometry(&serde_json::json!({"source": source}), &native["target"])
            };
            let target = app.document.add_geometry(target_geometry.clone()).unwrap();
            app.document.commit_transaction().unwrap();
            app.document
                .select_objects_direct(sources.iter().copied(), SelectionMode::Add)
                .unwrap();
            compare(
                &object_plane_snapshot(&app, &sources, target),
                &native["before"],
                label,
            );
            let picks = ["origin", "reference", "destination"]
                .map(|key| format!("w{}", format_model_point(p(&native[key]))));
            let copy = op["copy"].as_bool().unwrap();
            if !accepted {
                assert!(
                    app.commands
                        .execute(
                            &mut app.document,
                            &format!(
                                "ScaleByPlane Plane=Object {target} {} Rigid=No Copy=No",
                                picks.join(" ")
                            )
                        )
                        .is_err(),
                    "{label}"
                );
                compare(
                    &object_plane_snapshot(&app, &sources, target),
                    &native["before"],
                    label,
                );
            }
            if incremental || !accepted {
                enter(&mut app, "ScaleByPlane Rigid=No");
                enter(&mut app, if copy { "Copy=Yes" } else { "Copy=No" });
                enter(&mut app, "Plane=Object");
                assert_eq!(
                    app.accept_scale_by_plane_object(target, None),
                    accepted,
                    "{label}"
                );
                if accepted {
                    for pick in picks {
                        enter(&mut app, &pick);
                    }
                    if copy {
                        enter(&mut app, "");
                    }
                } else {
                    enter(&mut app, "Cancel");
                }
            } else {
                enter(
                    &mut app,
                    &format!(
                        "ScaleByPlane Plane=Object {target} {} Rigid=No Copy={}",
                        picks.join(" "),
                        if copy { "Yes" } else { "No" }
                    ),
                );
            }
            assert!(
                app.active_command.is_none(),
                "{label}: {:?}",
                app.command_log
            );
            compare(
                &object_plane_snapshot(&app, &sources, target),
                &native["after"],
                label,
            );
            assert_eq!(
                app.document.object(target).unwrap().geometry(),
                &target_geometry,
                "{label}"
            );
            enter(&mut app, "Cancel");
            compare(
                &object_plane_snapshot(&app, &sources, target),
                &native["after_script"],
                label,
            );
            enter(&mut app, "Undo");
            compare(
                &object_plane_snapshot(&app, &sources, target),
                &native["undo"],
                label,
            );
            enter(&mut app, "Redo");
            compare(
                &object_plane_snapshot(&app, &sources, target),
                &native["redo"],
                label,
            );
        }
    }
}

#[test]
fn scale_by_plane_replays_native_planes_geometry_grips_copy_and_external_history() {
    let mut count = 0;
    for (fixture, observed, n) in [
        (
            include_str!("../../../tools/rhino_oracle/fixtures/scale_by_plane.json"),
            include_str!("../../../tools/rhino_oracle/observations/scale_by_plane.json"),
            58,
        ),
        (
            include_str!("../../../tools/rhino_oracle/fixtures/scale_by_plane_boundary.json"),
            include_str!("../../../tools/rhino_oracle/observations/scale_by_plane_boundary.json"),
            48,
        ),
    ] {
        let q: Value = serde_json::from_str(fixture).unwrap();
        let r: Value = serde_json::from_str(observed).unwrap();
        assert_eq!(q["operations"].as_array().unwrap().len(), n);
        assert_eq!(r["results"].as_array().unwrap().len(), n);
        for (op, row) in q["operations"]
            .as_array()
            .unwrap()
            .iter()
            .zip(r["results"].as_array().unwrap())
        {
            assert_eq!(op["id"], row["id"]);
            let native = &row["value"];
            let label = op["id"].as_str().unwrap();
            let plane_choice = op["plane"].as_str().unwrap();
            let selection = op["selection"].as_str().unwrap();
            let copy = op["copy"].as_bool().unwrap();
            let rigid = op["rigid"].as_bool().unwrap();
            let rigid_grips = rigid && matches!(selection, "grips" | "parent");
            for incremental in [false, true] {
                let mut app = test_app();
                app.active_viewport = 1;
                app.viewports[1].set_construction_plane(frame(&native["active_plane"]));
                app.document
                    .begin_transaction("ScaleByPlane sources")
                    .unwrap();
                let geom = if op["source"] == "point" {
                    Geometry::Point(p(&native["before"][0]["point"]))
                } else {
                    geometry(op, &native["before"][0])
                };
                let source = app.document.add_geometry(geom).unwrap();
                app.document
                    .set_object_names([(source, Some("plane-scale source".into()))])
                    .unwrap();
                let target = if plane_choice == "Object" {
                    let f = frame(&native["plane"]);
                    let points = [(-2., -2.), (-2., 2.), (2., -2.), (2., 2.)]
                        .into_iter()
                        .map(|(u, v)| f.point_at([u, v, 0.]).unwrap())
                        .collect();
                    Some(
                        app.document
                            .add_geometry(Geometry::Brep(
                                Brep::try_surface_face(
                                    NurbsSurface::try_new(
                                        1,
                                        1,
                                        2,
                                        2,
                                        points,
                                        vec![0., 0., 1., 1.],
                                        vec![0., 0., 1., 1.],
                                    )
                                    .unwrap(),
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
                if matches!(selection, "grips" | "parent") {
                    app.document.enable_control_points([source]).unwrap();
                    app.document
                        .select_control_points(
                            [0, 2].map(|index| ControlPointId {
                                object: source,
                                index,
                            }),
                            SelectionMode::Add,
                        )
                        .unwrap();
                }
                if matches!(selection, "objects" | "parent") {
                    app.document
                        .select_objects_direct([source], SelectionMode::Add)
                        .unwrap();
                }
                checked(&app, source, target, &native["before"], label, rigid_grips);
                let picks = ["origin", "reference", "target"]
                    .map(|key| format!("w{}", format_model_point(p(&native[key]))));
                if incremental {
                    enter(&mut app, "ScaleByPlane");
                    if selection == "post" {
                        enter(&mut app, &format!("SelID {source}"));
                        enter(&mut app, "");
                    }
                    enter(&mut app, if copy { "Copy=Yes" } else { "Copy=No" });
                    enter(&mut app, if rigid { "Rigid=Yes" } else { "Rigid=No" });
                    enter(&mut app, &format!("Plane={plane_choice}"));
                    match plane_choice {
                        "3Point" => {
                            for point in plane_points(&native["plane"]) {
                                enter(&mut app, &point);
                            }
                        }
                        "Object" => {
                            assert!(app.accept_scale_by_plane_object(target.unwrap(), None));
                        }
                        "FromView" => {
                            assert!(app.accept_scale_by_plane_view(1));
                        }
                        _ => {}
                    }
                    for point in picks {
                        enter(&mut app, &point);
                    }
                    if copy {
                        enter(&mut app, "");
                    }
                    assert!(
                        app.active_command.is_none(),
                        "{label}: {:?}",
                        app.command_log
                    );
                } else {
                    let definition = match plane_choice {
                        "3Point" => {
                            format!("Plane=3Point {}", plane_points(&native["plane"]).join(" "))
                        }
                        "Object" => format!("Plane=Object {}", target.unwrap()),
                        _ => format!("Plane={plane_choice}"),
                    };
                    let selection = if selection == "post" {
                        format!("PickedSources={source}")
                    } else {
                        String::new()
                    };
                    enter(
                        &mut app,
                        &format!(
                            "ScaleByPlane {definition} {} Copy={} Rigid={} {selection}",
                            picks.join(" "),
                            if copy { "Yes" } else { "No" },
                            if rigid { "Yes" } else { "No" }
                        ),
                    );
                }
                checked(&app, source, target, &native["after"], label, rigid_grips);
                enter(&mut app, "Cancel");
                checked(
                    &app,
                    source,
                    target,
                    &native["after_script"],
                    label,
                    rigid_grips,
                );
                enter(&mut app, "Undo");
                checked(&app, source, target, &native["undo"], label, rigid_grips);
                enter(&mut app, "Redo");
                checked(&app, source, target, &native["redo"], label, rigid_grips);
                count += 1;
            }
        }
    }
    assert_eq!(count, 212);
}

#[test]
fn scale_by_plane_object_copy_reuses_sources_after_selection_cleanup() {
    let mut app = test_app();
    app.document
        .begin_transaction("Object copy sources")
        .unwrap();
    let source = app
        .document
        .add_geometry(Geometry::Point(point(2., 3., 4.)))
        .unwrap();
    let target = app
        .document
        .add_geometry(Geometry::Point(point(7., 8., 9.)))
        .unwrap();
    app.document.commit_transaction().unwrap();
    app.document
        .select_objects_direct([source], SelectionMode::Add)
        .unwrap();
    enter(&mut app, "ScaleByPlane Plane=Object Copy=Yes");
    assert!(app.accept_scale_by_plane_object(target, None));
    for pick in ["w1,2,3", "w3,5,8", "w5,11,1", "w7,14,1", ""] {
        enter(&mut app, pick);
    }
    assert!(app.active_command.is_none(), "{:?}", app.command_log);
    assert_eq!(
        app.document.object(source).unwrap().geometry(),
        &Geometry::Point(point(2., 3., 4.))
    );
    assert_eq!(
        app.document.object(target).unwrap().geometry(),
        &Geometry::Point(point(7., 8., 9.))
    );
    assert_eq!(
        app.document
            .objects()
            .skip(2)
            .map(|o| o.geometry().clone())
            .collect::<Vec<_>>(),
        [
            Geometry::Point(point(3., 5., 4.)),
            Geometry::Point(point(4., 6., 4.))
        ]
    );
    assert_eq!(app.document.selected_object_ids().count(), 0);
    enter(&mut app, "Undo");
    assert_eq!(app.document.objects().count(), 2);
    enter(&mut app, "Redo");
    assert_eq!(app.document.objects().count(), 4);
}

#[test]
fn scale_by_plane_preview_copy_repeats_target_and_view_pick_keeps_sources() {
    let mut app = test_app();
    let source = app
        .document
        .add_geometry(Geometry::Point(point(2., 3., 4.)))
        .unwrap();
    app.document.select_all();
    enter(&mut app, "ScaleByPlane Copy=Yes");
    enter(&mut app, "Plane");
    enter(&mut app, "FromView");
    assert!(app.picking_scale_by_plane_view());
    assert!(app.accept_scale_by_plane_view(1));
    enter(&mut app, "w1,2,3");
    enter(&mut app, "w3,5,3");
    let preview = app.affine_preview().unwrap();
    let map = preview
        .definition
        .transform_at(
            frame(
                &serde_json::json!({"origin":[0.,0.,0.],"x_axis":[1.,0.,0.],"y_axis":[0.,1.,0.]}),
            ),
            point(5., 11., 3.),
            Tolerance::DEFAULT,
        )
        .unwrap();
    assert!(
        map.transform_point(point(2., 3., 4.))
            .unwrap()
            .distance_to(point(3., 5., 4.))
            .unwrap()
            < 1e-12
    );
    assert_eq!(
        app.document.object(source).unwrap().geometry(),
        &Geometry::Point(point(2., 3., 4.))
    );
    enter(&mut app, "w5,11,3");
    enter(&mut app, "w7,14,3");
    enter(&mut app, "");
    let outputs = app
        .document
        .objects()
        .skip(1)
        .map(|o| o.geometry().clone())
        .collect::<Vec<_>>();
    assert_eq!(
        outputs,
        vec![
            Geometry::Point(point(3., 5., 4.)),
            Geometry::Point(point(4., 6., 4.))
        ]
    );
    enter(&mut app, "Undo");
    assert_eq!(app.document.objects().count(), 1);
    enter(&mut app, "ScaleByPlane");
    assert!(
        matches!(app.active_command,Some(InteractiveCommand::ScaleByPlane(p)) if p.options.plane==viboceros_command::scale_by_plane::PlaneChoice::ActiveCPlane)
    );
}

#[test]
fn scale_by_plane_invalid_frames_and_view_clicks_preserve_sources_and_option_memory() {
    let mut app = test_app();
    let source = app
        .document
        .add_geometry(Geometry::Point(point(2., 3., 4.)))
        .unwrap();
    let peer = app
        .document
        .add_geometry(Geometry::Point(point(8., 0., 0.)))
        .unwrap();
    app.document
        .select_objects_direct([source], SelectionMode::Add)
        .unwrap();
    enter(&mut app, "ScaleByPlane Plane=3Point");
    enter(&mut app, "w0,0,0");
    enter(&mut app, "w0,0,0");
    assert!(
        matches!(app.active_command,Some(InteractiveCommand::ScaleByPlane(p)) if p.plane_points==[Some(point(0.,0.,0.)),None])
    );
    enter(&mut app, "w1,0,0");
    enter(&mut app, "w2,0,0");
    assert!(
        matches!(app.active_command,Some(InteractiveCommand::ScaleByPlane(p)) if p.plane.is_none())
    );
    enter(&mut app, "w0,1,0");
    enter(&mut app, "Plane=Unknown");
    assert!(
        matches!(app.active_command,Some(InteractiveCommand::ScaleByPlane(p)) if p.plane.is_some() && p.origin.is_none())
    );
    enter(&mut app, "Plane=Object");
    assert!(!app.accept_scale_by_plane_object(peer, Some(0)));
    assert!(app.document.is_selected(source));
    assert!(!app.document.is_selected(peer));
    enter(&mut app, "Plane=FromView");
    app.active_viewport = 2;
    let selected_plane = app.viewports[2].construction_plane();
    app.handle_viewport_action(ViewportOutput {
        source_viewport_click: true,
        selection_click: Some(SelectionClick {
            object_id: Some(peer),
            mode: SelectionMode::Replace,
        }),
        ..Default::default()
    });
    assert!(app.document.is_selected(source));
    assert!(!app.document.is_selected(peer));
    assert!(
        matches!(app.active_command,Some(InteractiveCommand::ScaleByPlane(p)) if p.plane==Some(selected_plane))
    );
    enter(&mut app, "Rigid");
    enter(&mut app, "Rigid=Maybe");
    enter(&mut app, "Cancel");
    assert_eq!(
        app.commands.rigid_option_default("ScaleByPlane"),
        Some(true)
    );
    enter(&mut app, "ScaleByPlane");
    assert!(
        matches!(app.active_command,Some(InteractiveCommand::ScaleByPlane(p)) if p.options.rigid && p.options.plane==viboceros_command::scale_by_plane::PlaneChoice::ActiveCPlane)
    );
    enter(&mut app, "w1,2,3");
    enter(&mut app, "w3,5,3");
    assert_eq!(
        app.active_command.unwrap().anchor(),
        Some(point(1., 2., 3.))
    );
    enter(&mut app, "Plane=WorldTop");
    assert!(
        matches!(app.active_command,Some(InteractiveCommand::ScaleByPlane(p)) if p.plane==Some(selected_plane))
    );
    assert_eq!(
        app.document.object(source).unwrap().geometry(),
        &Geometry::Point(point(2., 3., 4.))
    );
}
