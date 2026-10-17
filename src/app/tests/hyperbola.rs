use super::*;
use viboceros_command::hyperbola::Slot;
use viboceros_geometry::Vector3;
fn enter(app: &mut VibocerosApp, text: &str) {
    app.command_input = text.into();
    app.run_command();
}
fn actions(app: &mut VibocerosApp, values: &serde_json::Value) {
    for value in values.as_array().unwrap() {
        let text = if let Some(word) = value.as_str() {
            word.to_owned()
        } else if value.is_number() {
            value.to_string()
        } else {
            let p = value.as_array().unwrap();
            format!("w{},{},{}", p[0], p[1], p[2])
        };
        enter(app, &text);
        assert!(
            app.command_input.is_empty(),
            "input was not cleared: {text}"
        );
    }
}
#[test]
fn hyperbola_frontend_native_modes_options_history_and_projective_controls() {
    let fixtures: serde_json::Value = serde_json::from_str(include_str!(
        "../../../tools/rhino_oracle/fixtures/hyperbola_command.json"
    ))
    .unwrap();
    let native: serde_json::Value = serde_json::from_str(include_str!(
        "../../../tools/rhino_oracle/observations/hyperbola_command_rhino_20261017.json"
    ))
    .unwrap();
    for (f, n) in fixtures["operations"]
        .as_array()
        .unwrap()
        .iter()
        .zip(native["results"].as_array().unwrap())
    {
        assert_eq!(f["id"], n["id"]);
        let id = f["id"].as_str().unwrap();
        let mut app = test_app();
        if let Some(points) = f.get("construction_plane") {
            let p: [[f64; 3]; 3] = serde_json::from_value(points.clone()).unwrap();
            let frame = Frame3::try_from_directions(
                Point3::try_from(p[0]).unwrap(),
                Vector3::try_from(p[1]).unwrap(),
                Vector3::try_from(p[2]).unwrap(),
                app.document.tolerance(),
            )
            .unwrap();
            app.viewports[app.active_viewport].set_construction_plane(frame);
        }
        let command = "Hyperbola";
        if let Some(setup) = f.get("setup") {
            enter(&mut app, command);
            actions(&mut app, setup);
            assert!(app.hyperbola_session.is_none(), "{id} setup");
            app.document.undo().unwrap();
            app.document.clear_history().unwrap();
        }
        enter(&mut app, command);
        actions(&mut app, &f["actions"]);
        assert!(
            app.hyperbola_session.is_none(),
            "{id}: {:?}",
            app.command_log
        );
        let expected = n["value"]["after"]["outputs"].as_array().unwrap();
        let actual = app.document.objects().collect::<Vec<_>>();
        assert_eq!(actual.len(), expected.len(), "{id}");
        for (o, n) in actual.iter().zip(expected) {
            assert_eq!(
                app.document.is_selected(o.id()),
                n["selected"].as_bool().unwrap()
            );
            if let Geometry::Point(p) = o.geometry() {
                let q: [f64; 3] = serde_json::from_value(n["point"].clone()).unwrap();
                assert!(
                    p.distance_to(Point3::try_from(q).unwrap()).unwrap() <= 1e-6,
                    "{id}"
                );
            } else {
                let Geometry::NurbsCurve(c) = o.geometry() else {
                    panic!("{id}")
                };
                let expected = &n["curve"];
                assert_eq!(c.degree(), expected["degree"].as_u64().unwrap() as usize);
                let knots: Vec<f64> = serde_json::from_value(expected["knots"].clone()).unwrap();
                assert_eq!(c.knots(), knots);
                assert_eq!(
                    c.control_points().len(),
                    expected["control_points"].as_array().unwrap().len()
                );
                assert_eq!(c.domain(), 0.0..=1.0);
                for (p, n) in c
                    .control_points()
                    .iter()
                    .zip(expected["control_points"].as_array().unwrap())
                {
                    let q: [f64; 3] = serde_json::from_value(n["point"].clone()).unwrap();
                    assert!(
                        p.point().distance_to(Point3::try_from(q).unwrap()).unwrap() <= 1e-6,
                        "{id}: {:?} != {q:?}",
                        p.point()
                    );
                    assert!((p.weight() - n["weight"].as_f64().unwrap()).abs() <= 1e-6);
                }
            }
        }
        drop(actual);
        assert_eq!(app.document.undo().unwrap().as_deref(), Some(command));
        assert_eq!(app.document.objects().count(), 0);
        assert!(app.document.undo().unwrap().is_none());
        assert_eq!(app.document.redo().unwrap().as_deref(), Some(command));
        assert_eq!(app.document.objects().count(), expected.len());
    }
}
#[test]
fn hyperbola_numeric_coefficients_support_units_defaults_invalid_retry_cancel_and_one_edit() {
    let mut app = test_app();
    enter(&mut app, "Units Millimeters Scale=No");
    app.document.clear_history().unwrap();
    enter(&mut app, "Hyperbola FromCoefficient");
    enter(&mut app, "0,0,0");
    enter(&mut app, "1,0,0");
    enter(&mut app, "A");
    assert!(!app.accept_drafting_point(point(5., 2., 0.)));
    enter(&mut app, "0");
    enter(&mut app, "-2mm");
    enter(&mut app, "nan");
    assert_eq!(
        app.hyperbola_session.as_ref().unwrap().input.options().a,
        1.
    );
    enter(&mut app, "0.3cm");
    enter(&mut app, "B=4mm");
    enter(&mut app, "A");
    enter(&mut app, "");
    let input = app.hyperbola_session.as_ref().unwrap().input;
    assert_eq!(input.options().a, 3.);
    assert_eq!(input.options().b, 4.);
    assert_eq!(input.slot(), Slot::End);
    assert!(!app.accept_drafting_point(point(2., 2., 0.)));
    assert_eq!(app.hyperbola_session.as_ref().unwrap().input, input);
    enter(&mut app, "BothBranches");
    enter(&mut app, "MarkFoci");
    enter(&mut app, "ShowAsymptotes");
    let before = format!("{:?}", app.document);
    let input = app.hyperbola_session.as_ref().unwrap().input;
    let preview = input.preview(point(5., -20., 10.)).unwrap();
    assert_eq!(preview.curves.len(), 2);
    assert_eq!(preview.foci.len(), 2);
    assert!(preview.asymptotes.is_some());
    assert_eq!(before, format!("{:?}", app.document));
    enter(&mut app, "5,2,0");
    assert!(app.hyperbola_session.is_none());
    assert_eq!(app.document.objects().count(), 4);
    assert_eq!(app.document.undo().unwrap().as_deref(), Some("Hyperbola"));
    assert!(app.document.undo().unwrap().is_none());
    let saved = app.commands.hyperbola_options();
    enter(&mut app, "Hyperbola FromCoefficient");
    enter(&mut app, "0,0,0");
    enter(&mut app, "1,0,0");
    enter(&mut app, "A=2");
    enter(&mut app, "Cancel");
    assert_eq!(app.commands.hyperbola_options(), saved);
    assert_eq!(app.document.objects().count(), 0);
    assert!(app.document.undo_label().is_none());
    enter(&mut app, "Hyperbola FromCoefficient");
    enter(&mut app, "A");
    enter(&mut app, "Line 0,0,0 2,0,0");
    assert!(app.hyperbola_session.is_none());
    assert_eq!(app.document.undo_label(), Some("Line"));
}
#[test]
fn hyperbola_coefficient_preview_and_commit_keep_the_captured_plane_after_view_change() {
    let mut app = test_app();
    let front = Frame3::try_from_directions(
        point(0., 0., 0.),
        Vector3::try_new(1., 0., 0.).unwrap(),
        Vector3::try_new(0., 0., 1.).unwrap(),
        app.document.tolerance(),
    )
    .unwrap();
    app.viewports[0].set_construction_plane(front);
    app.active_viewport = 0;
    enter(&mut app, "Hyperbola FromCoefficient");
    enter(&mut app, "w0,0,0");
    enter(&mut app, "w1,0,0");
    enter(&mut app, "A=3");
    enter(&mut app, "B=4");
    let preview = app
        .hyperbola_session
        .as_ref()
        .unwrap()
        .input
        .preview(point(5., 10., -20.))
        .unwrap();
    app.active_viewport = 1;
    app.viewports[1]
        .set_construction_plane(viboceros_command::CommandContext::default().construction_plane);
    assert!(app.accept_drafting_point(point(5., 10., -20.)));
    let Geometry::NurbsCurve(curve) = app.document.objects().next().unwrap().geometry() else {
        panic!()
    };
    assert_eq!(curve, &preview.curves[0]);
    assert!(curve.control_points().iter().all(|p| p.point().y() == 0.));
}

#[test]
fn hyperbola_focus_prompt_mode_choices_cancel_without_changes_like_native() {
    let mut app = test_app();
    enter(&mut app, "Point 20,30,40");
    app.document.clear_history().unwrap();
    let original = app.document.objects().next().unwrap().geometry().clone();
    for mode in ["FromCoefficient", "FromFoci", "FromVertex"] {
        enter(&mut app, "Hyperbola");
        enter(&mut app, "0,0,0");
        enter(&mut app, mode);
        assert!(app.hyperbola_session.is_none());
        assert_eq!(app.document.objects().count(), 1);
        assert_eq!(app.document.objects().next().unwrap().geometry(), &original);
        assert!(app.document.undo_label().is_none());
    }
}

#[test]
fn hyperbola_native_focus_only_and_empty_outputs_keep_single_or_no_history_edit() {
    let request: serde_json::Value = serde_json::from_str(include_str!(
        "../../../tools/rhino_oracle/fixtures/hyperbola_boundaries.json"
    ))
    .unwrap();
    let native: serde_json::Value = serde_json::from_str(include_str!(
        "../../../tools/rhino_oracle/observations/hyperbola_boundaries_rhino_20261017.json"
    ))
    .unwrap();
    for (f, n) in request["operations"]
        .as_array()
        .unwrap()
        .iter()
        .zip(native["results"].as_array().unwrap())
    {
        let id = f["id"].as_str().unwrap();
        let mut app = test_app();
        enter(&mut app, "Hyperbola");
        for action in f["actions"].as_array().unwrap() {
            let text = if let Some(word) = action.as_str() {
                word.to_owned()
            } else {
                let p = action.as_array().unwrap();
                format!("w{},{},{}", p[0], p[1], p[2])
            };
            enter(&mut app, &text);
        }
        if n["value"]["command_returned"] == false {
            assert_eq!(app.document.objects().count(), 0, "{id}");
            assert!(app.document.undo_label().is_none());
            assert!(app.hyperbola_session.is_some());
            enter(&mut app, "Cancel");
            continue;
        }
        assert!(app.hyperbola_session.is_none(), "{id}");
        let expected = n["value"]["after"]["outputs"].as_array().unwrap();
        assert_eq!(app.document.objects().count(), expected.len(), "{id}");
        for (o, n) in app.document.objects().zip(expected) {
            if let Geometry::Point(p) = o.geometry() {
                let q: [f64; 3] = serde_json::from_value(n["point"].clone()).unwrap();
                assert!(p.distance_to(Point3::try_from(q).unwrap()).unwrap() <= 1e-6);
            } else {
                let Geometry::NurbsCurve(c) = o.geometry() else {
                    panic!()
                };
                assert!(c.evaluate(0.5).is_ok());
            }
        }
        if expected.is_empty() {
            assert!(app.document.undo_label().is_none());
        } else {
            assert_eq!(app.document.undo().unwrap().as_deref(), Some("Hyperbola"));
            assert_eq!(app.document.objects().count(), 0);
            assert!(app.document.undo().unwrap().is_none());
        }
    }
}
