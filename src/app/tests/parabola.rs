use super::*;
use viboceros_command::parabola::{Kind, Slot};
use viboceros_geometry::Vector3;
fn enter(app: &mut VibocerosApp, text: &str) {
    app.command_input = text.into();
    app.run_command();
}
fn actions(app: &mut VibocerosApp, values: &serde_json::Value) {
    for value in values.as_array().unwrap() {
        let text = if let Some(word) = value.as_str() {
            word.to_owned()
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
fn parabola_frontend_all_native_modes_orders_options_focus_and_history() {
    let fixtures: serde_json::Value = serde_json::from_str(include_str!(
        "../../../tools/rhino_oracle/fixtures/parabola_command.json"
    ))
    .unwrap();
    let native: serde_json::Value = serde_json::from_str(include_str!(
        "../../../tools/rhino_oracle/observations/parabola_command_rhino_20261017.json"
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
        let command = f["command"].as_str().unwrap();
        if let Some(setup) = f.get("setup") {
            enter(&mut app, command);
            actions(&mut app, setup);
            assert!(app.parabola_session.is_none(), "{id} setup");
            app.document.undo().unwrap();
            app.document.clear_history().unwrap();
        }
        enter(&mut app, command);
        actions(&mut app, &f["actions"]);
        assert!(
            app.parabola_session.is_none(),
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
fn parabola_preview_failed_points_and_cancel_keep_history_unchanged() {
    let mut app = test_app();
    enter(&mut app, "Point 20,30,40");
    let id = app.document.objects().next().unwrap().id();
    app.document
        .select_object(id, SelectionMode::Replace)
        .unwrap();
    app.document.clear_history().unwrap();
    let original = app.document.object(id).unwrap().geometry().clone();
    enter(&mut app, "Parabola Vertex");
    enter(&mut app, "0,0,0");
    assert!(!app.accept_drafting_point(point(0., 0., 0.)));
    assert_eq!(
        app.parabola_session.as_ref().unwrap().input.slot(),
        Slot::Focus
    );
    enter(&mut app, "0,0,1");
    let input = app.parabola_session.as_ref().unwrap().input;
    for cursor in [point(2., 0., 99.), point(2., 3., 4.)] {
        assert!(input.preview(cursor).is_some());
    }
    assert_eq!(app.document.objects().count(), 1);
    assert!(app.document.undo_label().is_none());
    enter(&mut app, "Half=Yes");
    enter(&mut app, "Cancel");
    assert!(app.parabola_session.is_none());
    assert_eq!(app.document.object(id).unwrap().geometry(), &original);
    assert_eq!(
        app.commands.parabola_options(Kind::Axis),
        Default::default()
    );
    enter(&mut app, "Parabola3Pt");
    enter(&mut app, "Mode");
    enter(&mut app, "Focus");
    enter(&mut app, "PickOrder");
    enter(&mut app, "EndsFirst");
    enter(&mut app, "-1,0,.25");
    enter(&mut app, "3,0,2.25");
    let input = app.parabola_session.as_ref().unwrap().input;
    assert_eq!(input.slot(), Slot::Focus);
    assert!(input.preview(point(0., 0., 1.)).is_some());
    enter(&mut app, "Point 1,2,3");
    assert!(app.parabola_session.is_none());
    assert_eq!(app.document.objects().count(), 2);
    assert_eq!(app.document.undo_label(), Some("Point"));
}
#[test]
fn parabola_three_point_bad_direction_can_retry_and_option_submenu_yields_to_command() {
    let mut app = test_app();
    enter(&mut app, "Parabola3Pt");
    for text in ["-1,0,.25", "1,0,.25", "3,0,2.25"] {
        enter(&mut app, text);
    }
    assert_eq!(
        app.active_command.unwrap().anchor(),
        Some(point(3., 0., 2.25))
    );
    assert!(!app.accept_drafting_point(point(1., 0., 1.25)));
    assert_eq!(
        app.parabola_session.as_ref().unwrap().input.slot(),
        Slot::OpeningDirection
    );
    enter(&mut app, "MarkFocus");
    enter(&mut app, "3,0,3.25");
    assert!(app.parabola_session.is_none());
    assert_eq!(app.document.objects().count(), 2);
    enter(&mut app, "Parabola3Pt");
    enter(&mut app, "Mode");
    enter(&mut app, "Line 0,0,0 2,0,0");
    assert!(app.parabola_session.is_none());
    assert_eq!(app.document.undo_label(), Some("Line"));
}

#[test]
fn parabola_vertex_construction_and_mode_answers_take_precedence_over_snap_keywords() {
    let mut app = test_app();
    enter(&mut app, "Parabola");
    enter(&mut app, "Vertex");
    assert_eq!(
        app.parabola_session
            .as_ref()
            .unwrap()
            .input
            .options()
            .construction,
        viboceros_command::parabola::Construction::Vertex
    );
    assert_eq!(app.one_shot_snap_label(), None);
    enter(&mut app, "0,0,0");
    enter(&mut app, "Vertex");
    assert_eq!(app.one_shot_snap_label(), Some("Vertex"));
    enter(&mut app, "Cancel");
    enter(&mut app, "Parabola3Pt");
    enter(&mut app, "Mode");
    enter(&mut app, "Vertex");
    assert_eq!(
        app.parabola_session.as_ref().unwrap().input.options().mode,
        viboceros_command::parabola::Mode::Vertex
    );
    assert_eq!(app.one_shot_snap_label(), None);
}
