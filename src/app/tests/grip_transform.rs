use super::*;
use serde_json::Value;
use viboceros_document::{ControlPointId, SelectionMode};

fn enter(app: &mut VibocerosApp, value: &str) {
    app.command_input = value.into();
    app.run_command();
}
use crate::deformation_test_support::grips as shared;
pub(super) use shared::{compare, geometry};
pub(super) fn snapshot(app: &VibocerosApp, source: ObjectId, extra: Option<ObjectId>) -> Value {
    shared::snapshot(&app.document, source, extra)
}

#[test]
fn grip_transform_replays_native_geometry_selection_display_copy_and_history() {
    let q: Value = serde_json::from_str(include_str!(
        "../../../tools/rhino_oracle/fixtures/grip_transform.json"
    ))
    .unwrap();
    let r: Value = serde_json::from_str(include_str!(
        "../../../tools/rhino_oracle/observations/grip_transform.json"
    ))
    .unwrap();
    assert_eq!(q["operations"].as_array().unwrap().len(), 32);
    assert_eq!(r["results"].as_array().unwrap().len(), 32);
    let mut verified = 0;
    for (op, row) in q["operations"]
        .as_array()
        .unwrap()
        .iter()
        .zip(r["results"].as_array().unwrap())
    {
        for incremental in [false, true] {
            let mut app = test_app();
            let native = &row["value"];
            app.document
                .begin_transaction("Grip transform sources")
                .unwrap();
            let source = app
                .document
                .add_geometry(geometry(op, &native["before"][0]))
                .unwrap();
            app.document
                .set_object_names([(source, Some("grip source".into()))])
                .unwrap();
            let extra = if op["point"] == true {
                Some(
                    app.document
                        .add_geometry(Geometry::Point(point(8., 0., 0.)))
                        .unwrap(),
                )
            } else {
                None
            };
            app.document.commit_transaction().unwrap();
            app.document.enable_control_points([source]).unwrap();
            app.document
                .select_control_points(
                    op["selected"]
                        .as_array()
                        .unwrap()
                        .iter()
                        .map(|i| ControlPointId {
                            object: source,
                            index: i.as_u64().unwrap() as usize,
                        }),
                    SelectionMode::Add,
                )
                .unwrap();
            if op["parent"] == true {
                app.document
                    .select_objects_direct([source], SelectionMode::Add)
                    .unwrap();
            }
            if let Some(id) = extra {
                app.document
                    .select_objects_direct([id], SelectionMode::Add)
                    .unwrap();
            }
            compare(
                &snapshot(&app, source, extra),
                &native["before"],
                op["id"].as_str().unwrap(),
            );
            let (name, arguments) = match op["command"].as_str().unwrap() {
                "move" => ("Move", vec!["0,0,0", "1,2,3"]),
                "zero" => ("Move", vec!["0,0,0", "0,0,0"]),
                "rotate" => ("Rotate", vec!["0,0,0", "30"]),
                "scale" => ("Scale", vec!["0,0,0", "2"]),
                "mirror" => ("Mirror", vec!["0,0,0", "0,1,0"]),
                "copy" => ("Copy", vec!["0,0,0", "1,2,3"]),
                _ => unreachable!(),
            };
            if incremental || op["inputs"] == "all" {
                enter(
                    &mut app,
                    &format!(
                        "{name}{}",
                        if matches!(name, "Rotate" | "Scale" | "Mirror") {
                            " Copy=No"
                        } else {
                            ""
                        }
                    ),
                );
                if op["inputs"] == "all" {
                    enter(&mut app, "SelAll");
                    enter(&mut app, "");
                }
                for arg in &arguments {
                    enter(&mut app, arg);
                }
                if name == "Copy" {
                    enter(&mut app, "");
                }
            } else {
                enter(
                    &mut app,
                    &format!(
                        "{name} {}{}",
                        arguments.join(" "),
                        if matches!(name, "Rotate" | "Scale" | "Mirror") {
                            " Copy=No"
                        } else {
                            ""
                        }
                    ),
                );
            }
            compare(
                &snapshot(&app, source, extra),
                &native["after_script"],
                &format!("{} {incremental}: {:?}", op["id"], app.command_log),
            );
            assert!(app.active_command.is_none());
            if op["off"] == true {
                enter(&mut app, "PointsOff");
                compare(
                    &snapshot(&app, source, extra),
                    &native["points_off"],
                    "PointsOff",
                );
            }
            for phase in ["undo", "redo"] {
                enter(&mut app, phase);
                compare(
                    &snapshot(&app, source, extra),
                    &native[phase],
                    &format!("{} {incremental} {phase}", op["id"]),
                );
            }
            verified += 1;
        }
    }
    assert_eq!(verified, 64);
}

#[test]
fn repeated_grip_copies_freeze_sources_and_commit_one_history_entry() {
    let mut app = test_app();
    let points = [
        point(2., 0., 0.),
        point(0., 2., 0.),
        point(-2., 0., 0.),
        point(0., -2., 0.),
    ];
    let source = app
        .document
        .add_geometry(Geometry::NurbsCurve(
            NurbsCurve::try_clamped_uniform(2, points.to_vec()).unwrap(),
        ))
        .unwrap();
    app.document.enable_control_points([source]).unwrap();
    app.document
        .select_control_points(
            [ControlPointId {
                object: source,
                index: 0,
            }],
            SelectionMode::Add,
        )
        .unwrap();
    for input in ["Copy", "0,0,0", "1,2,3", "4,5,6", ""] {
        enter(&mut app, input);
    }
    assert!(app.active_command.is_none(), "{:?}", app.command_log);
    let owners = app.document.objects().map(|o| o.id()).collect::<Vec<_>>();
    assert_eq!(owners.len(), 3);
    assert_eq!(app.document.selected_control_points().count(), 3);
    for (i, id) in owners.into_iter().enumerate() {
        let Geometry::NurbsCurve(c) = app.document.object(id).unwrap().geometry() else {
            panic!("curve")
        };
        let expected = [point(2., 0., 0.), point(3., 2., 3.), point(6., 5., 6.)][i];
        assert_eq!(c.control_points()[0].point(), expected);
        assert_eq!(
            c.control_points()[1..]
                .iter()
                .map(|p| p.point())
                .collect::<Vec<_>>(),
            points[1..]
        );
    }
    assert_eq!(app.document.undo_label(), Some("Copy"));
    enter(&mut app, "Undo");
    assert_eq!(app.document.objects().len(), 1);
    assert_eq!(app.document.selected_control_points().count(), 1);
    enter(&mut app, "Redo");
    assert_eq!(app.document.objects().len(), 3);
    assert_eq!(app.document.selected_control_points().count(), 3);
}

#[test]
fn command_first_move_accepts_multiple_mouse_grips_without_selecting_parent() {
    use super::circle_fit_grips::{button, frame};
    let mut app = test_app();
    let points = [
        point(2., 0., 0.),
        point(0., 2., 0.),
        point(-2., 0., 0.),
        point(0., -2., 0.),
    ];
    let source = app
        .document
        .add_geometry(Geometry::NurbsCurve(
            NurbsCurve::try_clamped_uniform(2, points.to_vec()).unwrap(),
        ))
        .unwrap();
    app.document.enable_control_points([source]).unwrap();
    enter(&mut app, "Move");
    assert!(app.object_prompt.is_some());
    assert!(app.control_point_picking_available());
    let context = egui::Context::default();
    let (_, rect) = frame(&mut app, &context, vec![]);
    for index in [0, 2] {
        let screen = app.viewports[0].project(points[index], rect).unwrap();
        frame(
            &mut app,
            &context,
            vec![
                egui::Event::PointerMoved(screen),
                button(screen, true, egui::Modifiers::NONE),
            ],
        );
        let (output, _) = frame(
            &mut app,
            &context,
            vec![button(screen, false, egui::Modifiers::NONE)],
        );
        assert!(output.selection_click.is_none());
        assert_eq!(
            output.control_point_selection.as_ref().unwrap().picks,
            [ControlPointId {
                object: source,
                index
            }]
        );
        assert!(app.handle_viewport_action(output));
    }
    assert_eq!(app.document.selected_object_count(), 0);
    assert_eq!(app.document.selected_control_points().count(), 2);
    for input in ["", "0,0,0", "1,2,3"] {
        enter(&mut app, input);
    }
    assert!(app.active_command.is_none());
    let Geometry::NurbsCurve(c) = app.document.object(source).unwrap().geometry() else {
        panic!("curve")
    };
    assert_eq!(c.control_points()[0].point(), point(3., 2., 3.));
    assert_eq!(c.control_points()[1].point(), points[1]);
    assert_eq!(c.control_points()[2].point(), point(-1., 2., 3.));
    assert_eq!(c.control_points()[3].point(), points[3]);
    assert_eq!(app.document.selected_control_points().count(), 2);
}

#[test]
fn automatic_scale_center_uses_selected_grips_even_when_parent_selected() {
    let mut app = test_app();
    let points = [
        point(2., 0., 0.),
        point(0., 2., 0.),
        point(-2., 0., 0.),
        point(0., -2., 0.),
    ];
    let source = app
        .document
        .add_geometry(Geometry::NurbsCurve(
            NurbsCurve::try_clamped_uniform(2, points.to_vec()).unwrap(),
        ))
        .unwrap();
    app.document.enable_control_points([source]).unwrap();
    app.document
        .select_control_points(
            [0, 1].map(|index| ControlPointId {
                object: source,
                index,
            }),
            SelectionMode::Add,
        )
        .unwrap();
    app.document
        .select_objects_direct([source], SelectionMode::Add)
        .unwrap();
    for input in ["Scale Copy=No", "", "2"] {
        enter(&mut app, input);
    }
    assert!(app.active_command.is_none(), "{:?}", app.command_log);
    let Geometry::NurbsCurve(c) = app.document.object(source).unwrap().geometry() else {
        panic!("curve")
    };
    assert_eq!(c.control_points()[0].point(), point(3., -1., 0.));
    assert_eq!(c.control_points()[1].point(), point(-1., 3., 0.));
    assert_eq!(c.control_points()[2].point(), points[2]);
    assert_eq!(c.control_points()[3].point(), points[3]);
}
