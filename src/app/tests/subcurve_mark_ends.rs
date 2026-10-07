use super::*;
use serde_json::Value;
fn enter(app: &mut VibocerosApp, text: &str) {
    app.command_input = text.into();
    app.run_command();
}
fn p(v: &Value) -> Point3 {
    Point3::try_from(serde_json::from_value::<[f64; 3]>(v.clone()).unwrap()).unwrap()
}

#[test]
fn mark_ends_controller_replays_all_native_modes_points_and_history() {
    let capture: Value = serde_json::from_str(include_str!(
        "../../../tools/rhino_oracle/observations/subcurve_mark_ends.json"
    ))
    .unwrap();
    for recipe in capture["results"].as_array().unwrap() {
        let v = &recipe["value"];
        let case = recipe["id"]
            .as_str()
            .unwrap()
            .strip_prefix("mark_ends_")
            .unwrap();
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
                        p(&c["point"]),
                        c["weight"].as_f64().unwrap(),
                    )
                    .unwrap()
                })
                .collect(),
            serde_json::from_value(d["knots"].clone()).unwrap(),
        )
        .unwrap();
        let source = app
            .document
            .add_geometry(Geometry::NurbsCurve(curve))
            .unwrap();
        let before = app.document.object(source).unwrap().clone();
        let output = app.document.add_layer("output", ColorRgb::BLACK).unwrap();
        app.document.set_current_layer(output).unwrap();
        app.document.clear_history().unwrap();
        if case != "postselect" {
            app.document.select_command_results([source]).unwrap();
        }
        enter(
            &mut app,
            &format!(
                "SubCrv Mode=MarkEnds Copy={}",
                if v["copy"] == true { "Yes" } else { "No" }
            ),
        );
        if case == "postselect" {
            enter(&mut app, &source.to_string());
        }
        assert!(app.accept_drafting_point(p(&v["start"])));
        if case == "zero" {
            enter(&mut app, "0");
        } else if case == "no_confirmation" {
            enter(&mut app, "2");
            enter(&mut app, "");
        } else if case.starts_with("point_") {
            assert!(app.accept_drafting_point(if case == "point_forward" {
                point(3., 4.5, 0.)
            } else {
                point(1., 1.5, 0.)
            }));
        } else {
            enter(&mut app, v["length_token"].as_str().unwrap());
            assert!(!app.document.can_undo());
            assert!(app.accept_drafting_point(p(&v["confirmation"])));
        }
        assert!(app.subcurve_prompt.is_none());
        assert!(app.active_command.is_none());
        assert_eq!(app.document.object(source).unwrap(), &before);
        let outputs = app
            .document
            .objects()
            .filter(|o| o.id() != source)
            .collect::<Vec<_>>();
        let expected = v["after"]
            .as_array()
            .unwrap()
            .iter()
            .filter(|o| o["source"].is_null())
            .collect::<Vec<_>>();
        assert_eq!(outputs.len(), expected.len(), "{case}");
        for (o, e) in outputs.iter().zip(expected) {
            let Geometry::Point(point) = o.geometry() else {
                panic!()
            };
            assert!(point.distance_to(p(&e["point"])).unwrap() < 1e-6, "{case}");
            assert_eq!(o.attributes().layer_id(), output);
            assert!(o.group_ids().is_empty());
        }
        assert_eq!(app.document.selected_object_count(), 0);
        if v["success"] == false {
            assert!(!app.document.can_undo());
            continue;
        }
        enter(&mut app, "Undo");
        assert_eq!(app.document.objects().len(), 1);
        enter(&mut app, "Redo");
        assert_eq!(app.document.objects().len(), 3);
        assert_eq!(app.document.selected_object_count(), 0);
    }
}

#[test]
fn mark_ends_mode_changes_during_input_without_geometry_or_history() {
    let mut app = test_app();
    enter(&mut app, "Line 0,0 4,6");
    let source = app.document.objects().next().unwrap().id();
    app.document.select_command_results([source]).unwrap();
    app.document.clear_history().unwrap();
    enter(&mut app, "SubCrv");
    enter(&mut app, "Mode=Invalid");
    assert_eq!(
        app.subcurve_prompt.as_ref().unwrap().mode,
        viboceros_command::subcurve_input::SubcurveMode::Shorten
    );
    enter(&mut app, "1,1.5");
    enter(&mut app, "Mode=MarkEnds");
    enter(&mut app, "Copy=No");
    assert_eq!(app.document.objects().len(), 1);
    assert!(!app.document.can_undo());
    enter(&mut app, "3,4.5");
    assert_eq!(app.document.objects().len(), 3);
    assert!(
        app.document
            .objects()
            .skip(1)
            .all(|o| matches!(o.geometry(), Geometry::Point(_)))
    );
}
