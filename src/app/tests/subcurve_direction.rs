use super::*;
use serde_json::Value;

pub(super) fn enter(app: &mut VibocerosApp, input: &str) {
    app.command_input = input.into();
    app.run_command();
}
pub(super) fn position(v: &Value) -> Point3 {
    Point3::try_from(serde_json::from_value::<[f64; 3]>(v.clone()).unwrap()).unwrap()
}
pub(super) fn geometry(row: &Value) -> Geometry {
    let d = &row["definition"];
    let controls = d["control_points"]
        .as_array()
        .unwrap()
        .iter()
        .map(|c| {
            viboceros_geometry::WeightedPoint3::try_new(
                position(&c["point"]),
                c["weight"].as_f64().unwrap(),
            )
            .unwrap()
        })
        .collect();
    if row["kind"] == "curve" {
        Geometry::NurbsCurve(
            viboceros_geometry::NurbsCurve::try_new_rational(
                d["degree"].as_u64().unwrap() as usize,
                controls,
                serde_json::from_value(d["knots"].clone()).unwrap(),
            )
            .unwrap(),
        )
    } else {
        Geometry::NurbsSurface(
            viboceros_geometry::NurbsSurface::try_new_rational(
                d["degree"][0].as_u64().unwrap() as usize,
                d["degree"][1].as_u64().unwrap() as usize,
                d["control_count"][0].as_u64().unwrap() as usize,
                d["control_count"][1].as_u64().unwrap() as usize,
                controls,
                serde_json::from_value(d["knots_u"].clone()).unwrap(),
                serde_json::from_value(d["knots_v"].clone()).unwrap(),
            )
            .unwrap(),
        )
    }
}
pub(super) fn compare(app: &VibocerosApp, rows: &Value, case: &str) {
    assert_eq!(
        app.document.objects().len(),
        rows.as_array().unwrap().len(),
        "{case}"
    );
    for (actual, expected) in app.document.objects().zip(rows.as_array().unwrap()) {
        assert_eq!(
            app.document.is_selected(actual.id()),
            expected["selected"].as_bool().unwrap(),
            "{case}"
        );
        if let Some(p) = expected.get("point") {
            let Geometry::Point(actual) = actual.geometry() else {
                panic!("{case}")
            };
            assert!(actual.distance_to(position(p)).unwrap() < 1e-6, "{case}");
        } else if expected["kind"] == "curve" {
            let curve = actual.geometry().curve_ref().unwrap();
            for p in expected["samples"].as_array().unwrap() {
                let p = position(p);
                let t = curve
                    .closest_parameter(p, app.document.tolerance())
                    .unwrap();
                assert!(
                    curve.evaluate(t).unwrap().distance_to(p).unwrap() < 1e-6,
                    "{case}"
                );
            }
            assert!(
                curve
                    .start_point()
                    .unwrap()
                    .distance_to(position(&expected["samples"][0]))
                    .unwrap()
                    < 1e-6,
                "{case} start"
            );
            assert!(
                curve
                    .end_point()
                    .unwrap()
                    .distance_to(position(&expected["samples"][32]))
                    .unwrap()
                    < 1e-6,
                "{case} end"
            );
        } else {
            assert_eq!(actual.geometry(), &geometry(expected), "{case}");
        }
    }
}

#[test]
fn subcurve_direction_replays_native_hover_lock_numeric_completion_and_history() {
    let q: Value = serde_json::from_str(include_str!(
        "../../../tools/rhino_oracle/observations/subcurve_direction.json"
    ))
    .unwrap();
    for r in q["results"].as_array().unwrap() {
        let v = &r["value"];
        let case = v["case"].as_str().unwrap();
        let mut app = test_app();
        let ids = v["before"]
            .as_array()
            .unwrap()
            .iter()
            .map(|row| app.document.add_geometry(geometry(row)).unwrap())
            .collect::<Vec<_>>();
        let originals = app
            .document
            .objects()
            .map(|o| (o.id(), o.geometry().clone()))
            .collect::<Vec<_>>();
        app.document.clear_history().unwrap();
        let inline = case.starts_with("inline_");
        if inline {
            enter(&mut app, "CreateUVCrv");
            enter(&mut app, &ids[1].to_string());
            enter(&mut app, "SubCrv");
            enter(&mut app, &ids[0].to_string());
        } else {
            app.document.select_command_results([ids[0]]).unwrap();
            enter(
                &mut app,
                &format!(
                    "SubCrv Copy=Yes FromMidpoint=No Mode={}",
                    if case.starts_with("mark_") {
                        "MarkEnds"
                    } else {
                        "Shorten"
                    }
                ),
            );
        }
        assert!(app.accept_drafting_point(position(&v["start"])));
        app.handle_viewport_action(ViewportOutput {
            drafting_hover: Some(position(&v["aim"])),
            ..Default::default()
        });
        assert!(!app.document.can_undo());
        enter(
            &mut app,
            if case == "direction_option" {
                "Direction=Locked"
            } else {
                "D"
            },
        );
        if case == "unlock_point" {
            enter(&mut app, "Direction=Free");
        }
        if case.ends_with("numeric") {
            enter(
                &mut app,
                if case.starts_with("closed_") {
                    "8"
                } else {
                    "2"
                },
            );
        } else {
            let p = if case == "same_side_point" {
                position(&v["aim"])
            } else {
                position(&v["endpoint"])
            };
            assert!(app.accept_drafting_point(p), "{case}");
        }
        if !v["finish_prompts"].as_array().unwrap().is_empty() {
            let pending = app.subcurve_prompt.as_ref().unwrap();
            assert_eq!(pending.source, Some(ids[0]), "{case}");
            assert!(pending.start.is_some(), "{case}");
            assert_eq!(pending.length, Some(8.), "{case}");
            assert!(pending.locked_forward.is_none(), "{case}");
            assert!(!app.document.can_undo(), "{case}");
            enter(&mut app, "");
        }
        assert!(app.active_command.is_none(), "{case}");
        if inline {
            enter(&mut app, "");
        }
        assert!(app.subcurve_prompt.is_none(), "{case}");
        assert!(app.intersection_prompt.is_none(), "{case}");
        compare(&app, &v["after"], case);
        for (id, g) in &originals {
            assert_eq!(app.document.object(*id).unwrap().geometry(), g, "{case}");
        }
        if v["success"] == false {
            assert!(!app.document.can_undo(), "{case}");
        } else {
            enter(&mut app, "Undo");
            compare(&app, &v["undo"], case);
            enter(&mut app, "Redo");
            compare(&app, &v["redo"], case);
        }
    }
}

#[test]
fn subcurve_direction_requires_hover_is_transient_and_retries_invalid_lengths() {
    let mut app = test_app();
    enter(&mut app, "Line 0,0 4,6");
    let id = app.document.objects().next().unwrap().id();
    app.document.select_command_results([id]).unwrap();
    app.document.clear_history().unwrap();
    enter(&mut app, "SubCrv Copy=Yes");
    enter(&mut app, "2,3");
    enter(&mut app, "D");
    assert!(
        app.subcurve_prompt
            .as_ref()
            .unwrap()
            .locked_forward
            .is_none()
    );
    app.handle_viewport_action(ViewportOutput {
        drafting_hover: Some(point(3.2, 4.8, 0.)),
        ..Default::default()
    });
    enter(&mut app, "Direction=Maybe");
    assert!(
        app.subcurve_prompt
            .as_ref()
            .unwrap()
            .locked_forward
            .is_none()
    );
    enter(&mut app, "_D");
    assert_eq!(
        app.subcurve_prompt.as_ref().unwrap().locked_forward,
        Some(true)
    );
    enter(&mut app, "20");
    assert!(app.subcurve_prompt.is_some());
    assert!(!app.document.can_undo());
    app.cancel_current_prompt_or_selection();
    app.document.select_command_results([id]).unwrap();
    enter(&mut app, "SubCrv");
    enter(&mut app, "2,3");
    enter(&mut app, "2");
    assert_eq!(app.subcurve_prompt.as_ref().unwrap().length, Some(2.));
    assert!(
        app.subcurve_prompt
            .as_ref()
            .unwrap()
            .locked_forward
            .is_none()
    );
    assert!(!app.document.can_undo());
}

#[test]
fn inline_subcurve_direction_remains_captured_after_hover_moves_and_parent_owns_history() {
    for command in ["ApplyCrv", "CreateUVCrv"] {
        let mut app = test_app();
        enter(&mut app, "SrfPt 0,0 4,0 4,6 0,6");
        enter(&mut app, "Line 0,0 4,6");
        let ids = app.document.objects().map(|o| o.id()).collect::<Vec<_>>();
        let original = app.document.object(ids[1]).unwrap().geometry().clone();
        app.document.clear_history().unwrap();
        enter(&mut app, command);
        if command == "CreateUVCrv" {
            enter(&mut app, &ids[0].to_string());
        }
        enter(&mut app, "SubCrv");
        enter(&mut app, &ids[1].to_string());
        enter(&mut app, "2,3");
        app.handle_viewport_action(ViewportOutput {
            drafting_hover: Some(point(0.8, 1.2, 0.)),
            ..Default::default()
        });
        enter(&mut app, "D");
        app.handle_viewport_action(ViewportOutput {
            drafting_hover: Some(point(3.2, 4.8, 0.)),
            ..Default::default()
        });
        enter(&mut app, "-2mm");
        let p = app.intersection_prompt.as_ref().unwrap();
        assert!(p.uv_subcurves.pending.is_none(), "{command}");
        let range = &p.uv_subcurves.ranges[0];
        let curve = app
            .document
            .object(ids[1])
            .unwrap()
            .geometry()
            .curve_ref()
            .unwrap();
        assert!(
            curve
                .evaluate(range.parameters[0])
                .unwrap()
                .distance_to(point(0.8905996075495418, 1.3358994113243128, 0.))
                .unwrap()
                < 1e-6,
            "{command}"
        );
        assert!(
            curve
                .evaluate(range.parameters[1])
                .unwrap()
                .distance_to(point(2., 3., 0.))
                .unwrap()
                < 1e-6,
            "{command}"
        );
        assert!(!app.document.can_undo());
        assert_eq!(app.document.object(ids[1]).unwrap().geometry(), &original);
        enter(&mut app, "");
        if command == "ApplyCrv" {
            enter(&mut app, &ids[0].to_string());
        }
        assert!(app.intersection_prompt.is_none(), "{command}");
        let count = app.document.objects().len();
        assert!(count > 2, "{command}");
        assert_eq!(app.document.undo_label(), Some(command));
        enter(&mut app, "Undo");
        assert_eq!(app.document.objects().len(), 2);
        enter(&mut app, "Redo");
        assert_eq!(app.document.objects().len(), count);
    }
}
