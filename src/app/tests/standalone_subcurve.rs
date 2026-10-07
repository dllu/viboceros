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
fn standalone_numeric_source_picking_and_history_replay_all_native_cases() {
    let q: Value = serde_json::from_str(include_str!(
        "../../../tools/rhino_oracle/observations/standalone_subcurve.json"
    ))
    .unwrap();
    for r in q["results"].as_array().unwrap() {
        let v = &r["value"];
        let case = r["id"].as_str().unwrap().strip_prefix("subcurve_").unwrap();
        let mut app = test_app();
        let d = &v["before"][0]["definition"];
        let curve = viboceros_geometry::NurbsCurve::try_new_rational(
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
        let layer = app
            .document
            .add_layer(v["before"][0]["layer"].as_str().unwrap(), ColorRgb::BLACK)
            .unwrap();
        let source = app
            .document
            .add_geometry_with_attributes(
                Geometry::NurbsCurve(curve),
                viboceros_document::ObjectAttributes::on_layer(layer)
                    .with_name("source")
                    .try_with_user_text("source", "original")
                    .unwrap(),
            )
            .unwrap();
        app.document
            .add_group(Some("original".into()), [source])
            .unwrap();
        app.document.clear_history().unwrap();
        if case != "postselect" {
            app.document.select_command_results([source]).unwrap();
        }
        enter(
            &mut app,
            &format!(
                "SubCrv Copy={}",
                if v["copy"] == true { "Yes" } else { "No" }
            ),
        );
        if case == "postselect" {
            assert!(app.subcurve_prompt.as_ref().unwrap().source.is_none());
            app.apply_selection_click(SelectionClick {
                object_id: Some(source),
                mode: SelectionMode::Replace,
            });
        }
        assert!(app.accept_drafting_point(point_value(&v["start"])));
        if case == "zero" {
            enter(&mut app, "0");
        } else if case == "no_confirmation" {
            enter(&mut app, "2");
            enter(&mut app, "");
        } else if case.starts_with("point_") || case.starts_with("closed_point_") {
            let end = match case {
                "point_forward" => point(3., 4.5, 0.),
                "point_backward" => point(1., 1.5, 0.),
                _ => point_value(&v["confirmation"]),
            };
            assert!(app.accept_drafting_point(end));
        } else {
            enter(&mut app, v["length_token"].as_str().unwrap());
            assert!(app.point_constraint.is_none());
            assert!(!app.document.can_undo());
            assert!(
                app.accept_drafting_point(point_value(&v["confirmation"])),
                "{case}"
            );
        }
        assert!(app.subcurve_prompt.is_none(), "{case}");
        assert!(app.active_command.is_none(), "{case}");
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
            assert_eq!(actual.attributes().layer_id(), layer);
            assert_eq!(actual.attributes().name(), Some("source"));
            let curve = actual.geometry().curve_ref().unwrap().to_nurbs().unwrap();
            for s in expected["samples"].as_array().unwrap() {
                let p = point_value(s);
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
                    .evaluate(*curve.domain().start())
                    .unwrap()
                    .distance_to(point_value(&expected["samples"][0]))
                    .unwrap()
                    < 1e-6,
                "{case} start"
            );
            assert!(
                curve
                    .evaluate(*curve.domain().end())
                    .unwrap()
                    .distance_to(point_value(&expected["samples"][32]))
                    .unwrap()
                    < 1e-6,
                "{case} end"
            );
        }
        if v["success"] == false {
            assert!(!app.document.can_undo());
            continue;
        }
        enter(&mut app, "Undo");
        assert_eq!(app.document.objects().len(), 1);
        assert_eq!(app.document.selected_object_count(), 0);
        enter(&mut app, "Redo");
        assert_eq!(
            app.document.selected_object_count(),
            v["redo"]
                .as_array()
                .unwrap()
                .iter()
                .filter(|o| o["selected"] == true)
                .count()
        );
    }
}

#[test]
fn standalone_source_cancel_and_bad_lengths_do_not_edit_the_document() {
    let mut app = test_app();
    enter(&mut app, "Line 0,0 4,6");
    let source = app.document.objects().next().unwrap().id();
    app.document.clear_selection();
    app.document.clear_history().unwrap();
    enter(&mut app, "SubCrv");
    app.cancel_current_prompt_or_selection();
    assert!(app.subcurve_prompt.is_none());
    enter(&mut app, "SubCrv");
    enter(&mut app, &source.to_string());
    enter(&mut app, "1,1.5");
    for input in ["20", "NaN", "1/0"] {
        enter(&mut app, input);
        assert!(app.subcurve_prompt.as_ref().unwrap().length.is_none());
    }
    enter(&mut app, "2mm");
    assert_eq!(app.subcurve_prompt.as_ref().unwrap().length, Some(2.));
    app.cancel_current_prompt_or_selection();
    assert_eq!(app.document.objects().len(), 1);
    assert!(!app.document.can_undo());
    assert!(app.subcurve_prompt.is_none());
}
