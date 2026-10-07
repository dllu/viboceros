use super::*;
use serde_json::Value;
fn enter(app: &mut VibocerosApp, text: &str) {
    app.command_input = text.into();
    app.run_command();
}
fn point_value(v: &Value) -> Point3 {
    Point3::try_from(serde_json::from_value::<[f64; 3]>(v.clone()).unwrap()).unwrap()
}

#[test]
fn midpoint_controller_replays_native_instant_numeric_radius_and_history() {
    let q: Value = serde_json::from_str(include_str!(
        "../../../tools/rhino_oracle/observations/subcurve_midpoint.json"
    ))
    .unwrap();
    for recipe in q["results"].as_array().unwrap() {
        let v = &recipe["value"];
        let case = recipe["id"]
            .as_str()
            .unwrap()
            .strip_prefix("midpoint_")
            .unwrap();
        let mut app = test_app();
        let d = &v["before"][0]["definition"];
        let c = viboceros_geometry::NurbsCurve::try_new_rational(
            d["degree"].as_u64().unwrap() as usize,
            d["control_points"]
                .as_array()
                .unwrap()
                .iter()
                .map(|c| {
                    viboceros_geometry::WeightedPoint3::try_new(
                        point_value(&c["point"]),
                        c["weight"].as_f64().unwrap(),
                    )
                    .unwrap()
                })
                .collect(),
            serde_json::from_value(d["knots"].clone()).unwrap(),
        )
        .unwrap();
        let source = app.document.add_geometry(Geometry::NurbsCurve(c)).unwrap();
        let original = app.document.object(source).unwrap().geometry().clone();
        let output = app.document.add_layer("output", ColorRgb::BLACK).unwrap();
        app.document.set_current_layer(output).unwrap();
        app.document.clear_history().unwrap();
        if case != "postselect" {
            app.document.select_command_results([source]).unwrap();
        }
        enter(
            &mut app,
            &format!(
                "SubCrv FromMidpoint=Yes Copy={} Mode={}",
                if v["copy"] == true { "Yes" } else { "No" },
                if case.starts_with("mark_") {
                    "MarkEnds"
                } else {
                    "Shorten"
                }
            ),
        );
        if case == "postselect" {
            enter(&mut app, &source.to_string());
        }
        assert!(app.subcurve_prompt.as_ref().unwrap().from_midpoint);
        assert!(app.accept_drafting_point(point_value(&v["start"])));
        if case.starts_with("point_") || case == "quadratic_point" {
            let p = match case {
                "point_forward" => point(3., 4.5, 0.),
                "point_backward" => point(1., 1.5, 0.),
                _ => point_value(&v["confirmation"]),
            };
            assert!(app.accept_drafting_point(p), "{case}");
        } else {
            enter(
                &mut app,
                if case == "zero" {
                    "0"
                } else {
                    v["length_token"].as_str().unwrap()
                },
            );
        }
        assert!(app.subcurve_prompt.is_none(), "{case}");
        assert!(app.active_command.is_none(), "{case}");
        assert!(app.point_constraint.is_none());
        assert_eq!(
            app.document.objects().len(),
            v["after"].as_array().unwrap().len(),
            "{case}"
        );
        for (actual, expected) in app.document.objects().zip(v["after"].as_array().unwrap()) {
            assert_eq!(
                app.document.is_selected(actual.id()),
                expected["selected"].as_bool().unwrap(),
                "{case}"
            );
            if expected.get("point").is_some() {
                let Geometry::Point(p) = actual.geometry() else {
                    panic!()
                };
                assert!(
                    p.distance_to(point_value(&expected["point"])).unwrap() < 1e-6,
                    "{case}"
                );
                assert_eq!(actual.attributes().layer_id(), output);
                assert!(actual.group_ids().is_empty());
            } else {
                let c = actual.geometry().curve_ref().unwrap().to_nurbs().unwrap();
                for s in expected["samples"].as_array().unwrap() {
                    let p = point_value(s);
                    let t = c.closest_parameter(p, app.document.tolerance()).unwrap();
                    assert!(
                        c.evaluate(t).unwrap().distance_to(p).unwrap() < 1e-6,
                        "{case}"
                    );
                }
                assert!(
                    c.evaluate(*c.domain().start())
                        .unwrap()
                        .distance_to(point_value(&expected["samples"][0]))
                        .unwrap()
                        < 1e-6,
                    "{case} start"
                );
                assert!(
                    c.evaluate(*c.domain().end())
                        .unwrap()
                        .distance_to(point_value(&expected["samples"][32]))
                        .unwrap()
                        < 1e-6,
                    "{case} end"
                );
            }
        }
        if v["copy"] == true || case.starts_with("mark_") || v["success"] == false {
            assert_eq!(app.document.object(source).unwrap().geometry(), &original);
        }
        if v["success"] == false {
            assert!(!app.document.can_undo());
            continue;
        }
        let count = app.document.objects().len();
        enter(&mut app, "Undo");
        assert_eq!(app.document.objects().len(), 1);
        assert_eq!(app.document.selected_object_count(), 0);
        enter(&mut app, "Redo");
        assert_eq!(app.document.objects().len(), count);
    }
}

#[test]
fn midpoint_option_changes_and_invalid_radius_are_pending_without_edits() {
    let mut app = test_app();
    enter(&mut app, "Line 0,0 4,6");
    let id = app.document.objects().next().unwrap().id();
    app.document.select_command_results([id]).unwrap();
    app.document.clear_history().unwrap();
    enter(&mut app, "SubCrv");
    enter(&mut app, "FromMidpoint=Maybe");
    assert!(!app.subcurve_prompt.as_ref().unwrap().from_midpoint);
    enter(&mut app, "FromMidpoint=Yes");
    enter(&mut app, "2,3");
    enter(&mut app, "20");
    assert!(app.subcurve_prompt.is_some());
    assert!(!app.document.can_undo());
    enter(&mut app, "Mode=MarkEnds");
    enter(&mut app, "2mm");
    assert!(app.subcurve_prompt.is_none());
    assert_eq!(app.document.objects().len(), 3);
    assert!(
        app.document
            .objects()
            .skip(1)
            .all(|o| matches!(o.geometry(), Geometry::Point(_)))
    );
}
