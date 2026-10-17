use super::*;
use viboceros_command::spiral_input::{Kind, Mode, Slot};
fn enter(a: &mut VibocerosApp, s: &str) {
    a.command_input = s.to_owned();
    a.run_command();
}
fn point(x: f64, y: f64, z: f64) -> Point3 {
    Point3::try_new(x, y, z).unwrap()
}
#[test]
fn spiral_frontend_units_option_getters_cancellation_and_history() {
    let mut a = test_app();
    enter(&mut a, "Helix");
    enter(&mut a, "0,0,0");
    enter(&mut a, "0,0,10");
    enter(&mut a, "Turns");
    enter(&mut a, "3/2");
    assert_eq!(
        a.spiral_session.as_ref().unwrap().input.options().turns,
        1.5
    );
    enter(&mut a, "Pitch");
    enter(&mut a, "0");
    assert_eq!(
        a.spiral_session.as_ref().unwrap().input.options().turns,
        1.5
    );
    enter(&mut a, "2mm");
    assert_eq!(
        a.spiral_session.as_ref().unwrap().input.options().mode,
        Mode::Pitch
    );
    enter(&mut a, "Diameter");
    enter(&mut a, "6mm");
    assert_eq!(a.document.objects().len(), 0);
    assert_eq!(
        a.spiral_session.as_ref().unwrap().input.slot(),
        Slot::StartRadius
    );
    assert!(a.accept_drafting_point(point(0., 8., 0.)));
    assert!(a.spiral_session.is_none());
    assert_eq!(a.document.undo_label(), Some("Helix"));
    let Geometry::NurbsCurve(c) = a.document.objects().next().unwrap().geometry() else {
        panic!()
    };
    assert!(
        c.control_points()[0]
            .point()
            .distance_to(point(0., 3., 0.))
            .unwrap()
            < 1e-12
    );
    assert_eq!(c.domain(), 0.0..=5.0);
    let settings = a.commands.spiral_options(Kind::Helix);
    enter(&mut a, "Undo");
    assert_eq!(a.document.objects().len(), 0);
    assert_eq!(a.commands.spiral_options(Kind::Helix), settings);
    assert_ne!(a.commands.spiral_options(Kind::Spiral), settings);
    enter(&mut a, "Helix");
    enter(&mut a, "0,0,0");
    enter(&mut a, "0,0,10");
    enter(&mut a, "Turns=2");
    enter(&mut a, "Cancel");
    assert!(a.spiral_session.is_none());
    assert_eq!(a.commands.spiral_options(Kind::Helix), settings);
    assert_eq!(a.document.objects().len(), 0);
    enter(&mut a, "Spiral");
    enter(&mut a, "Line");
    assert!(a.spiral_session.is_none());
    assert!(matches!(
        a.active_command,
        Some(InteractiveCommand::Line { .. })
    ));
}
#[test]
fn spiral_frontend_enter_uses_hover_direction_without_preview_edits() {
    let mut a = test_app();
    enter(&mut a, "Helix");
    enter(&mut a, "0,0,0");
    enter(&mut a, "0,0,10");
    enter(&mut a, "2");
    let s = a.spiral_session.as_mut().unwrap();
    s.cursor = Some(point(0., 7., 0.));
    let before = s.input;
    assert!(s.input.preview(point(0., 7., 0.)).is_some());
    assert_eq!(s.input, before);
    assert_eq!(a.document.objects().len(), 0);
    assert!(a.document.undo_label().is_none());
    enter(&mut a, "Enter");
    assert!(a.spiral_session.is_none());
    let Geometry::NurbsCurve(c) = a.document.objects().next().unwrap().geometry() else {
        panic!()
    };
    assert_eq!(c.control_points()[0].point(), point(0., 2., 0.));
}

#[test]
fn spiral_frontend_native_getter_workflows_match_full_control_layout_and_undo() {
    fn actions(a: &mut VibocerosApp, values: &serde_json::Value) {
        for v in values.as_array().unwrap() {
            let text = if let Some(s) = v.as_str() {
                s.to_owned()
            } else if v.is_number() {
                v.to_string()
            } else {
                let p = v.as_array().unwrap();
                format!("w{},{},{}", p[0], p[1], p[2])
            };
            enter(a, &text);
            assert!(a.command_input.is_empty(), "{text}: {:?}", a.command_log);
        }
    }
    for (q, n) in [
        (
            include_str!("../../../tools/rhino_oracle/fixtures/spiral_command.json"),
            include_str!(
                "../../../tools/rhino_oracle/observations/spiral_command_rhino_20261017.json"
            ),
        ),
        (
            include_str!("../../../tools/rhino_oracle/fixtures/spiral_preferences.json"),
            include_str!(
                "../../../tools/rhino_oracle/observations/spiral_preferences_rhino_20261017.json"
            ),
        ),
        (
            include_str!("../../../tools/rhino_oracle/fixtures/spiral_defaults.json"),
            include_str!(
                "../../../tools/rhino_oracle/observations/spiral_defaults_rhino_20261017.json"
            ),
        ),
    ] {
        let q: serde_json::Value = serde_json::from_str(q).unwrap();
        let n: serde_json::Value = serde_json::from_str(n).unwrap();
        for (q, n) in q["operations"]
            .as_array()
            .unwrap()
            .iter()
            .zip(n["results"].as_array().unwrap())
        {
            assert_eq!(q["id"], n["id"]);
            let id = q["id"].as_str().unwrap();
            let mut a = test_app();
            if let Some(plane) = q.get("construction_plane") {
                let [p, x, y]: [[f64; 3]; 3] = serde_json::from_value(plane.clone()).unwrap();
                a.viewports[a.active_viewport].set_construction_plane(
                    Frame3::try_from_directions(
                        Point3::try_from(p).unwrap(),
                        viboceros_geometry::Vector3::try_from(x).unwrap(),
                        viboceros_geometry::Vector3::try_from(y).unwrap(),
                        a.document.tolerance(),
                    )
                    .unwrap(),
                );
            }
            let command = if q["op"] == "helix_command" {
                "Helix"
            } else {
                "Spiral"
            };
            if let Some(setup) = q.get("setup") {
                enter(&mut a, command);
                actions(&mut a, setup);
                assert!(a.spiral_session.is_none());
                a.document.undo().unwrap();
                a.document.clear_history().unwrap();
            }
            enter(&mut a, command);
            actions(&mut a, &q["actions"]);
            assert!(a.spiral_session.is_none(), "{id}: {:?}", a.command_log);
            assert_eq!(a.document.objects().len(), 1, "{id}");
            let object = a.document.objects().next().unwrap();
            assert!(!a.document.is_selected(object.id()));
            let Geometry::NurbsCurve(c) = object.geometry() else {
                panic!("{id}")
            };
            let native = &n["value"]["after"]["outputs"][0]["curve"];
            assert_eq!(
                c.degree(),
                native["degree"].as_u64().unwrap() as usize,
                "{id}"
            );
            let knots: Vec<f64> = serde_json::from_value(native["knots"].clone()).unwrap();
            assert_eq!(knots.len(), c.knots().len(), "{id}");
            for (a, b) in c.knots().iter().zip(knots) {
                assert!((a - b).abs() <= 1e-6, "{id}");
            }
            let controls = native["control_points"].as_array().unwrap();
            assert_eq!(c.control_points().len(), controls.len(), "{id}");
            for (p, q) in c.control_points().iter().zip(controls) {
                let expected: [f64; 3] = serde_json::from_value(q["point"].clone()).unwrap();
                assert!(
                    p.point()
                        .distance_to(Point3::try_from(expected).unwrap())
                        .unwrap()
                        <= 1e-6,
                    "{id}: {:?} != {expected:?}",
                    p.point()
                );
                assert!((p.weight() - q["weight"].as_f64().unwrap()).abs() <= 1e-6);
            }
            assert_eq!(a.document.undo().unwrap().as_deref(), Some(command));
            assert_eq!(a.document.objects().len(), 0);
            assert!(a.document.undo().unwrap().is_none());
            assert_eq!(a.document.redo().unwrap().as_deref(), Some(command));
            assert_eq!(a.document.objects().len(), 1);
        }
    }
}

#[test]
fn spiral_frontend_flat_preview_and_commit_keep_the_initial_plane() {
    let mut a = test_app();
    let plane = Frame3::try_from_directions(
        point(4., 5., 6.),
        viboceros_geometry::Vector3::try_new(1., 0., 0.).unwrap(),
        viboceros_geometry::Vector3::try_new(0., 0., 1.).unwrap(),
        a.document.tolerance(),
    )
    .unwrap();
    a.viewports[a.active_viewport].set_construction_plane(plane);
    enter(&mut a, "Spiral Flat");
    enter(&mut a, "w1,2,3");
    enter(&mut a, "w2,2,3");
    enter(&mut a, "Turns=2");
    let preview = a
        .spiral_session
        .as_ref()
        .unwrap()
        .input
        .preview(point(5., 2., 3.))
        .unwrap();
    assert_eq!(a.document.objects().len(), 0);
    a.viewports[a.active_viewport]
        .set_construction_plane(viboceros_command::CommandContext::default().construction_plane);
    enter(&mut a, "4");
    assert!(a.spiral_session.is_none());
    let Geometry::NurbsCurve(c) = a.document.objects().next().unwrap().geometry() else {
        panic!()
    };
    assert_eq!(c, &preview);
    assert!(
        c.control_points()
            .iter()
            .all(|p| (p.point().y() - 2.).abs() < 1e-12)
    );
}
