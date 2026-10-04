use super::grip_transform::{compare, geometry, snapshot};
use super::*;
use serde_json::Value;
use viboceros_document::{ControlPointId, SelectionMode};
use viboceros_geometry::Vector3;

fn enter(app: &mut VibocerosApp, text: &str) {
    app.command_input = text.into();
    app.run_command();
}

fn coords(value: &Value) -> [f64; 3] {
    serde_json::from_value(value.clone()).unwrap()
}

#[test]
fn scale_nu_native_replays_numeric_reference_distance_grips_copy_and_history() {
    let q: Value = serde_json::from_str(include_str!(
        "../../../tools/rhino_oracle/fixtures/scale_nu.json"
    ))
    .unwrap();
    let r: Value = serde_json::from_str(include_str!(
        "../../../tools/rhino_oracle/observations/scale_nu.json"
    ))
    .unwrap();
    assert_eq!(q["operations"].as_array().unwrap().len(), 26);
    assert_eq!(r["results"].as_array().unwrap().len(), 26);
    let diagnostic = [5, 10, 11, 12, 20, 22];
    let mut compatible = 0;
    for (i, (op, row)) in q["operations"]
        .as_array()
        .unwrap()
        .iter()
        .zip(r["results"].as_array().unwrap())
        .enumerate()
    {
        for incremental in [false, true] {
            let mut app = test_app();
            let native = &row["value"];
            assert_eq!(row["id"], op["id"]);
            app.active_viewport = 1;
            app.viewports[1].set_construction_plane(
                Frame3::try_from_directions(
                    Point3::try_from(coords(&native["plane"]["origin"])).unwrap(),
                    Vector3::try_from(coords(&native["plane"]["x_axis"])).unwrap(),
                    Vector3::try_from(coords(&native["plane"]["y_axis"])).unwrap(),
                    Tolerance::DEFAULT,
                )
                .unwrap(),
            );
            for axis in 0..3 {
                assert!(app.commands.remember_axis_scale(
                    "ScaleNU",
                    axis,
                    native["seed"][axis].as_f64().unwrap()
                ));
            }
            app.document.begin_transaction("ScaleNU sources").unwrap();
            let is_point = op["source"] == "point";
            let source = app
                .document
                .add_geometry(if is_point {
                    Geometry::Point(point(2., 3., 4.))
                } else {
                    geometry(op, &native["before"][0])
                })
                .unwrap();
            app.document
                .set_object_names([(source, Some("grip source".into()))])
                .unwrap();
            let peer = (op["point"] == true).then(|| {
                app.document
                    .add_geometry(Geometry::Point(point(8., 0., 0.)))
                    .unwrap()
            });
            app.document.commit_transaction().unwrap();
            let all = op["selection"] == "all";
            if is_point || op["selection"] == "object" {
                if !all {
                    app.document
                        .select_objects_direct([source], SelectionMode::Add)
                        .unwrap();
                }
            } else {
                app.document.enable_control_points([source]).unwrap();
                if !all {
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
            }
            if op["parent"] == true {
                app.document
                    .select_objects_direct([source], SelectionMode::Add)
                    .unwrap();
            }
            if let Some(peer) = peer {
                app.document
                    .select_objects_direct([peer], SelectionMode::Add)
                    .unwrap();
            }
            compare(
                &snapshot(&app, source, peer),
                &native["before"],
                "ScaleNU baseline",
            );
            let tokens = native["macro"]
                .as_str()
                .unwrap()
                .split_whitespace()
                .collect::<Vec<_>>();
            let copy = op["copy"].as_bool().unwrap();
            if incremental || all || op["input"] == "repeat" {
                enter(&mut app, "ScaleNU");
                for token in &tokens[1..] {
                    enter(&mut app, token);
                }
            } else {
                let arguments = tokens[1..]
                    .iter()
                    .filter(|token| **token != "_Enter")
                    .map(|token| token.strip_prefix('w').unwrap_or(token))
                    .collect::<Vec<_>>();
                let mut script = format!("ScaleNU {}", arguments.join(" "));
                if op["input"] == "defaults" {
                    script.push_str(" 2 3 .5");
                }
                enter(&mut app, &script);
            }
            assert!(
                app.active_command.is_none(),
                "{}: {:?}",
                op["id"],
                app.command_log
            );
            let actual = snapshot(&app, source, peer);
            let context = format!(
                "{} incremental={incremental} copy={copy}: {:?}",
                op["id"], app.command_log
            );
            if diagnostic.contains(&i) {
                // Preserve the native script discrepancy, with an independent
                // axis-distance witness for our coordinate input behavior.
                let expected = match i {
                    11 => [2., 9., 4.],
                    12 => [2., 3., 12.],
                    _ => [6., 3., 4.],
                };
                compare(&actual[0]["point"], &serde_json::json!(expected), &context);
                assert!(
                    coords(&native["after_script"][0]["point"])
                        .into_iter()
                        .zip(expected)
                        .any(|(a, b)| (a - b).abs() > 1e-6)
                );
            } else {
                compare(&actual, &native["after_script"], &context);
                compatible += 1;
            }
            enter(&mut app, "Undo");
            compare(
                &snapshot(&app, source, peer),
                &native["undo"],
                &format!("Undo {context}"),
            );
            enter(&mut app, "Redo");
            if !diagnostic.contains(&i) {
                compare(
                    &snapshot(&app, source, peer),
                    &native["redo"],
                    &format!("Redo {context}"),
                );
            }
        }
    }
    assert_eq!(compatible, 40);
}

#[test]
fn scale_nu_partial_input_preview_defaults_retry_and_cancel() {
    let mut app = test_app();
    enter(&mut app, "Point 2,3,4");
    enter(&mut app, "SelAll");
    let source = app.document.selected_object_ids().next().unwrap();
    for input in ["ScaleNU", "w0,0,0", "2", "NaN"] {
        enter(&mut app, input);
    }
    assert_eq!(
        app.commands.axis_scale_defaults("ScaleNU"),
        Some([2., 1., 1.])
    );
    assert_eq!(
        app.document.object(source).unwrap().geometry(),
        &Geometry::Point(point(2., 3., 4.))
    );
    let preview = app.affine_preview().unwrap();
    assert_eq!(
        preview
            .last_transform
            .unwrap()
            .transform_point(point(2., 3., 4.))
            .unwrap(),
        point(4., 3., 4.)
    );
    let map = preview
        .definition
        .transform_at(
            preview.frame.unwrap(),
            point(0., 6., 0.),
            Tolerance::DEFAULT,
        )
        .unwrap();
    assert_eq!(
        map.transform_point(point(2., 3., 4.)).unwrap(),
        point(4., 3., 4.)
    );
    enter(&mut app, "Cancel");
    assert!(app.active_command.is_none());
    assert_eq!(
        app.commands.axis_scale_defaults("ScaleNU"),
        Some([2., 1., 1.])
    );
    for input in [
        "ScaleNU Copy=Yes WorldCoordinates",
        "w0,0,0",
        "",
        "0",
        "1",
        "4",
        "",
        "",
    ] {
        enter(&mut app, input);
    }
    // Enter at Y and Z accepts defaults during the second copy; Enter at the
    // next X prompt finishes the repeated session.
    assert!(app.active_command.is_some());
    enter(&mut app, "");
    assert!(app.active_command.is_none());
    assert_eq!(app.document.objects().count(), 3);
    enter(&mut app, "Undo");
    assert_eq!(app.document.objects().count(), 1);
}
