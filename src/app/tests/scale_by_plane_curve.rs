use super::grip_transform::{compare, geometry};
use super::scale_by_plane::{frame, object_plane_snapshot, p};
use super::*;
use serde_json::Value;
use viboceros_document::SelectionMode;
use viboceros_geometry::{Circle3, CircularArc3, CurveSegment3, Ellipse3, PolyCurve3};

fn enter(app: &mut VibocerosApp, input: &str) {
    app.command_input = input.into();
    app.run_command();
}
fn curve(value: &Value) -> Geometry {
    if let Some(arc) = value.get("arc") {
        let f = frame(&arc["plane"]);
        let circle = Circle3::try_from_frame(
            f.origin(),
            arc["radius"].as_f64().unwrap(),
            f.x_axis(),
            f.z_axis(),
            Tolerance::DEFAULT,
        )
        .unwrap();
        let c = CircularArc3::try_from_circle_angles(
            circle,
            arc["angles"][0].as_f64().unwrap()..=arc["angles"][1].as_f64().unwrap(),
        )
        .unwrap();
        return Geometry::Arc(
            c.try_reparameterized(
                value["curve"]["domain"][0].as_f64().unwrap()
                    ..=value["curve"]["domain"][1].as_f64().unwrap(),
            )
            .unwrap(),
        );
    }
    if let Some(segments) = value.get("segments") {
        let segments = segments
            .as_array()
            .unwrap()
            .iter()
            .map(|row| match curve(row) {
                Geometry::Arc(arc) => CurveSegment3::Arc(arc),
                Geometry::Line(line) => CurveSegment3::Line(line),
                Geometry::NurbsCurve(nurbs) => CurveSegment3::NurbsCurve(nurbs),
                _ => unreachable!(),
            })
            .collect::<Vec<_>>();
        return Geometry::PolyCurve(
            PolyCurve3::try_with_segment_domains(
                segments,
                serde_json::from_value(value["parameters"].clone()).unwrap(),
            )
            .unwrap(),
        );
    }
    let g = geometry(
        &serde_json::json!({"source": if value["kind"]=="LineCurve" {"line"} else {"rational"}}),
        value,
    );
    if let Geometry::Line(line) = g {
        return Geometry::Line(
            line.try_reparameterized(
                value["curve"]["domain"][0].as_f64().unwrap()
                    ..=value["curve"]["domain"][1].as_f64().unwrap(),
            )
            .unwrap(),
        );
    }
    g
}

#[test]
fn native_curve_classes_keep_cplane_frames_and_plane_scale_history() {
    let q: Value = serde_json::from_str(include_str!(
        "../../../tools/rhino_oracle/fixtures/scale_by_plane_curve.json"
    ))
    .unwrap();
    let r: Value = serde_json::from_str(include_str!(
        "../../../tools/rhino_oracle/observations/scale_by_plane_curve.json"
    ))
    .unwrap();
    assert_eq!(q["operations"].as_array().unwrap().len(), 64);
    assert_eq!(r["results"].as_array().unwrap().len(), 64);
    let mut replays = 0;
    for (op, row) in q["operations"]
        .as_array()
        .unwrap()
        .iter()
        .zip(r["results"].as_array().unwrap())
    {
        assert_eq!(op["id"], row["id"]);
        let native = &row["value"];
        let label = op["id"].as_str().unwrap();
        for incremental in [false, true] {
            let mut app = test_app();
            app.active_viewport = 1;
            enter(&mut app, "CPlane World Top");
            let initial = app.viewports[1].construction_plane();
            app.document
                .begin_transaction("Curve frame sources")
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
            let target_geometry = if op["target"] == "ellipse" {
                let f = frame(&native["target_frame"]);
                let e = Ellipse3::try_new(
                    f.origin(),
                    3.,
                    2.,
                    f.x_axis(),
                    f.y_axis(),
                    Tolerance::DEFAULT,
                )
                .unwrap();
                Geometry::Ellipse(if op["reverse"].as_bool().unwrap() {
                    e.reversed()
                } else {
                    e
                })
            } else {
                curve(&native["target"])
            };
            let target = app.document.add_geometry(target_geometry.clone()).unwrap();
            app.document.commit_transaction().unwrap();
            if incremental {
                enter(&mut app, "CPlane Object");
                assert!(
                    app.accept_plane_prompt_object(target),
                    "{label}: {:?}",
                    app.command_log
                );
            } else {
                enter(&mut app, &format!("CPlane Object {target}"));
            }
            let actual = app.viewports[1].construction_plane();
            let expected = frame(&native["cplane"]);
            for (a, b) in actual
                .origin()
                .to_array()
                .into_iter()
                .zip(expected.origin().to_array())
            {
                assert!((a - b).abs() < 1e-9, "{label}: origin {a} vs {b}");
            }
            for (a, b) in actual.axes().into_iter().zip(expected.axes()) {
                assert!(
                    a.as_vector()
                        .to_array()
                        .into_iter()
                        .zip(b.as_vector().to_array())
                        .all(|(a, b)| (a - b).abs() < 1e-9),
                    "{label}: {actual:?} vs {expected:?}"
                );
            }
            enter(&mut app, "CPlane Undo");
            assert_eq!(app.viewports[1].construction_plane(), initial);
            enter(&mut app, "CPlane Redo");
            assert_eq!(app.viewports[1].construction_plane(), actual);
            enter(&mut app, "CPlane World Top");
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
            if incremental {
                enter(&mut app, "ScaleByPlane Plane=Object");
                assert!(app.accept_scale_by_plane_object(target, None), "{label}");
                for pick in picks {
                    enter(&mut app, &pick);
                }
            } else {
                enter(
                    &mut app,
                    &format!("ScaleByPlane Plane=Object {target} {}", picks.join(" ")),
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
            replays += 1;
        }
    }
    assert_eq!(replays, 128);
}

#[test]
fn native_point_object_cplanes_retain_axes_in_complete_and_picked_input() {
    let q: Value = serde_json::from_str(include_str!(
        "../../../tools/rhino_oracle/fixtures/scale_by_plane_object.json"
    ))
    .unwrap();
    let r: Value = serde_json::from_str(include_str!(
        "../../../tools/rhino_oracle/observations/scale_by_plane_object.json"
    ))
    .unwrap();
    let mut replays = 0;
    for (op, row) in q["operations"]
        .as_array()
        .unwrap()
        .iter()
        .zip(r["results"].as_array().unwrap())
    {
        if op["target"] != "point" {
            continue;
        }
        let native = &row["value"];
        for incremental in [false, true] {
            let mut app = test_app();
            app.active_viewport = 1;
            let initial = frame(&native["active_plane"]);
            app.viewports[1].set_construction_plane(initial);
            let target_geometry = Geometry::Point(p(&native["target"]["point"]));
            let target = app.document.add_geometry(target_geometry.clone()).unwrap();
            let undo_before = app.document.undo_label().map(str::to_owned);
            if incremental {
                enter(&mut app, "CPlane Object");
                assert!(app.accept_plane_prompt_object(target));
            } else {
                enter(&mut app, &format!("CPlane Object {target}"));
            }
            let expected = frame(&native["cplane"]);
            assert_eq!(app.viewports[1].construction_plane(), expected);
            assert_eq!(
                app.document.object(target).unwrap().geometry(),
                &target_geometry
            );
            assert_eq!(app.document.undo_label(), undo_before.as_deref());
            enter(&mut app, "CPlane Undo");
            assert_eq!(app.viewports[1].construction_plane(), initial);
            enter(&mut app, "CPlane Redo");
            assert_eq!(app.viewports[1].construction_plane(), expected);
            enter(&mut app, "Undo");
            assert!(app.document.object(target).is_none());
            assert_eq!(app.viewports[1].construction_plane(), expected);
            enter(&mut app, "Redo");
            assert_eq!(
                app.document.object(target).unwrap().geometry(),
                &target_geometry
            );
            assert_eq!(app.viewports[1].construction_plane(), expected);
            replays += 1;
        }
    }
    assert_eq!(replays, 20);
}
