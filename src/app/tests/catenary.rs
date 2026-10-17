use super::*;
use viboceros_command::catenary_input::{Mode, Slot};
fn enter(app: &mut VibocerosApp, text: &str) {
    app.command_input = text.into();
    app.run_command();
}
fn p(x: f64, y: f64, z: f64) -> Point3 {
    Point3::try_new(x, y, z).unwrap()
}
fn actions(app: &mut VibocerosApp, actions: &[viboceros_oracle::CatenaryCommandAction]) {
    use viboceros_oracle::CatenaryCommandAction as Action;
    for action in actions {
        match action {
            Action::Click { .. } => panic!("physical click scenarios use the snap-event replay"),
            Action::Option(s) => enter(app, s),
            Action::Number(n) => enter(app, &n.to_string()),
            Action::Point([x, y, z]) => enter(app, &format!("w{x},{y},{z}")),
            Action::Relative {
                relative: [x, y, z],
            } => enter(app, &format!("r{x},{y},{z}")),
        }
        assert!(
            app.command_input.is_empty(),
            "{action:?}: {:?}",
            app.command_log
        );
    }
}
pub(super) fn assert_model(app: &VibocerosApp, expected: &serde_json::Value, id: &str) {
    assert_model_excluding(app, expected, id, &[]);
}
pub(super) fn assert_model_excluding(
    app: &VibocerosApp,
    expected: &serde_json::Value,
    id: &str,
    guides: &[ObjectId],
) {
    let objects = app
        .document
        .objects()
        .filter(|o| !guides.contains(&o.id()))
        .collect::<Vec<_>>();
    let outputs = expected["outputs"].as_array().unwrap();
    assert_eq!(objects.len(), outputs.len(), "{id}");
    for (object, expected) in objects.iter().zip(outputs) {
        assert_eq!(
            app.document.is_selected(object.id()),
            expected["selected"].as_bool().unwrap(),
            "{id}"
        );
        if let Some(point) = expected.get("point") {
            let Geometry::Point(actual) = object.geometry() else {
                panic!("{id}")
            };
            let point: [f64; 3] = serde_json::from_value(point.clone()).unwrap();
            assert!(
                actual
                    .distance_to(Point3::try_from(point).unwrap())
                    .unwrap()
                    <= 1e-6,
                "{id}"
            );
            continue;
        }
        let curve = object
            .geometry()
            .nurbs_curve_representation()
            .unwrap()
            .unwrap();
        let n = &expected["curve"];
        assert_eq!(
            curve.degree(),
            n["degree"].as_u64().unwrap() as usize,
            "{id}"
        );
        assert_eq!(
            curve.is_closed().unwrap(),
            expected["closed"].as_bool().unwrap(),
            "{id}"
        );
        let knots: Vec<f64> = serde_json::from_value(n["knots"].clone()).unwrap();
        assert_eq!(curve.knots().len(), knots.len(), "{id}");
        for (a, b) in curve.knots().iter().zip(knots) {
            assert!((a - b).abs() <= 1e-6, "{id}: {a} != {b}");
        }
        let points = n["control_points"].as_array().unwrap();
        assert_eq!(curve.control_points().len(), points.len(), "{id}");
        for (a, b) in curve.control_points().iter().zip(points) {
            let point: [f64; 3] = serde_json::from_value(b["point"].clone()).unwrap();
            assert!(
                a.point()
                    .distance_to(Point3::try_from(point).unwrap())
                    .unwrap()
                    <= 1e-6,
                "{id}: {:?} != {point:?}",
                a.point()
            );
            assert!(
                (a.weight() - b["weight"].as_f64().unwrap()).abs() <= 1e-6,
                "{id}"
            );
        }
        let domain: [f64; 2] = serde_json::from_value(n["domain"].clone()).unwrap();
        assert!((curve.domain().start() - domain[0]).abs() <= 1e-6, "{id}");
        assert!((curve.domain().end() - domain[1]).abs() <= 1e-6, "{id}");
        let length = object
            .geometry()
            .curve_ref()
            .unwrap()
            .length(app.document.tolerance())
            .unwrap();
        assert!(
            (length - expected["length"].as_f64().unwrap()).abs() <= 1e-6,
            "{id}"
        );
    }
}
#[test]
fn catenary_frontend_snap_inputs_match_native_geometry_and_preserve_sources_and_history() {
    for (q, n, count) in [
        (
            include_str!("../../../tools/rhino_oracle/fixtures/catenary_snap_inputs.json"),
            include_str!(
                "../../../tools/rhino_oracle/observations/catenary_snap_inputs_rhino_20261017.json"
            ),
            3,
        ),
        // Documented End completion is compared to the equivalent native Enter
        // construction: the captured native endpoint clicks fail on Rhino 8.32.
        (
            include_str!("../../../tools/rhino_oracle/fixtures/catenary_end_apex_snap.json"),
            include_str!(
                "../../../tools/rhino_oracle/observations/catenary_end_apex_enter_rhino_20261017.json"
            ),
            6,
        ),
    ] {
        let q: viboceros_oracle::ProbeRequest = serde_json::from_str(q).unwrap();
        let n: serde_json::Value = serde_json::from_str(n).unwrap();
        assert_eq!(q.operations.len(), count);
        for (op, row) in q.operations.iter().zip(n["results"].as_array().unwrap()) {
            let viboceros_oracle::Operation::CatenaryCommand { id, fixture: f } = op else {
                panic!()
            };
            assert_eq!(id, row["id"].as_str().unwrap());
            let mut app = test_app();
            let guides = f
                .source_geometries(app.document.tolerance())
                .unwrap()
                .into_iter()
                .map(|g| app.document.add_geometry(g).unwrap())
                .collect::<Vec<_>>();
            let sources = guides
                .iter()
                .map(|id| app.document.object(*id).unwrap().clone())
                .collect::<Vec<_>>();
            app.document.clear_history().unwrap();
            enter(&mut app, "Catenary");
            for action in &f.actions {
                if let viboceros_oracle::CatenaryCommandAction::Click { click } = action {
                    let snap = f
                        .resolve_click(&app.document, Point3::try_from(*click).unwrap())
                        .unwrap();
                    let before = app.catenary_session.as_ref().unwrap().input;
                    let preview = before
                        .preview_snapped(snap.point(), Some(snap.kind()))
                        .unwrap();
                    assert!(app.handle_viewport_action(ViewportOutput {
                        picked_point: Some(snap.point()),
                        picked_snap: Some(snap),
                        ..Default::default()
                    }));
                    assert!(
                        app.catenary_session.is_none(),
                        "{id}: {:?}",
                        app.command_log
                    );
                    let output = app
                        .document
                        .objects()
                        .filter(|o| !guides.contains(&o.id()))
                        .find(|o| o.geometry().curve_ref().is_some())
                        .unwrap();
                    let expected = match preview.curve {
                        viboceros_geometry::CatenaryCurve::Smooth(curve) => {
                            Geometry::NurbsCurve(curve)
                        }
                        viboceros_geometry::CatenaryCurve::Polyline(polyline) => {
                            Geometry::Polyline(polyline)
                        }
                    };
                    assert_eq!(output.geometry(), &expected, "{id}");
                } else {
                    actions(&mut app, std::slice::from_ref(action));
                }
            }
            let options = app.commands.catenary_options();
            assert_eq!(options.mode, Mode::ThroughPoint);
            assert_model_excluding(&app, &row["value"]["after"], id, &guides);
            assert_eq!(app.document.undo().unwrap().as_deref(), Some("Catenary"));
            assert_model_excluding(&app, &row["value"]["undo"], id, &guides);
            assert!(app.document.undo().unwrap().is_none());
            assert_eq!(app.document.redo().unwrap().as_deref(), Some("Catenary"));
            assert_model_excluding(&app, &row["value"]["redo"], id, &guides);
            assert_eq!(app.commands.catenary_options(), options);
            for source in &sources {
                let actual = app.document.object(source.id()).unwrap();
                assert_eq!(actual, source, "{id}");
                assert!(
                    actual
                        .geometry_snapshot()
                        .shares_storage_with(source.geometry_snapshot())
                );
            }
        }
    }
}
fn endpoint_app() -> VibocerosApp {
    let mut app = test_app();
    enter(&mut app, "Catenary");
    for word in ["w0,0,0", "w10,-2,0", "w0,-1,0", "MarkApex=Yes"] {
        enter(&mut app, word);
    }
    app
}
fn end_snap(app: &mut VibocerosApp, endpoint: Point3) -> viboceros_drafting::ObjectSnap {
    let line = viboceros_geometry::LineSegment::try_new(
        p(endpoint.x(), endpoint.y() + 3., endpoint.z() + 4.),
        endpoint,
        app.document.tolerance(),
    )
    .unwrap();
    app.document.add_geometry(Geometry::Line(line)).unwrap();
    app.document.clear_history().unwrap();
    viboceros_drafting::ObjectSnapCache::default()
        .nearest_axis_aligned_with_modes(
            &app.document,
            viboceros_geometry::PointCloudProjection::Xy,
            endpoint,
            [0., 0.],
            1.,
            viboceros_drafting::ObjectSnapModes::NONE
                .with(viboceros_drafting::ObjectSnapKind::End, true),
        )
        .unwrap()
        .unwrap()
}
#[test]
fn catenary_filtered_coordinates_and_typed_retries_do_not_inherit_end_snap_meaning() {
    let mut app = endpoint_app();
    let end = p(10., -2., 0.);
    let snap = end_snap(&mut app, end);
    let before = app.catenary_session.as_ref().unwrap().input;
    assert!(before.preview_snapped(end, Some(snap.kind())).is_some());
    assert_eq!(app.catenary_session.as_ref().unwrap().input, before);
    enter(&mut app, "w10,-2,0");
    assert_eq!(app.catenary_session.as_ref().unwrap().input, before);
    assert!(app.document.undo_label().is_none());
    enter(&mut app, "Cancel");
    assert!(app.catenary_session.is_none());
    assert_eq!(app.commands.catenary_options(), Default::default());
    // A .y filter turns a different source End into the captured endpoint.
    // The resulting coordinates must lose the source's End provenance.
    let mut app = endpoint_app();
    let snap = end_snap(&mut app, p(10., 7., 0.));
    enter(&mut app, ".y");
    enter(&mut app, "w0,-2,0");
    assert!(app.handle_viewport_action(ViewportOutput {
        picked_point: Some(snap.point()),
        picked_snap: Some(snap),
        ..Default::default()
    }));
    assert_eq!(
        app.catenary_session.as_ref().unwrap().input.slot(),
        Slot::Constraint
    );
    assert_eq!(app.document.objects().len(), 1);
    assert!(app.document.undo_label().is_none());
}
#[test]
fn catenary_frontend_native_modes_relative_bases_outputs_and_actual_history() {
    for (q, n) in [
        (
            include_str!("../../../tools/rhino_oracle/fixtures/catenary_command.json"),
            include_str!(
                "../../../tools/rhino_oracle/observations/catenary_command_rhino_20261017.json"
            ),
        ),
        (
            include_str!("../../../tools/rhino_oracle/fixtures/catenary_preferences.json"),
            include_str!(
                "../../../tools/rhino_oracle/observations/catenary_preferences_rhino_20261017.json"
            ),
        ),
        (
            include_str!("../../../tools/rhino_oracle/fixtures/catenary_directions.json"),
            include_str!(
                "../../../tools/rhino_oracle/observations/catenary_directions_rhino_20261017.json"
            ),
        ),
        (
            include_str!("../../../tools/rhino_oracle/fixtures/catenary_final.json"),
            include_str!(
                "../../../tools/rhino_oracle/observations/catenary_final_rhino_20261017.json"
            ),
        ),
    ] {
        let q: viboceros_oracle::ProbeRequest = serde_json::from_str(q).unwrap();
        let n: serde_json::Value = serde_json::from_str(n).unwrap();
        for (op, native) in q.operations.iter().zip(n["results"].as_array().unwrap()) {
            let viboceros_oracle::Operation::CatenaryCommand { id, fixture: f } = op else {
                panic!()
            };
            let mut app = test_app();
            app.last_point = f.prior_point.map(|p| Point3::try_from(p).unwrap());
            if let Some([o, x, y]) = f.construction_plane {
                app.viewports[app.active_viewport].set_construction_plane(
                    Frame3::try_from_directions(
                        Point3::try_from(o).unwrap(),
                        viboceros_geometry::Vector3::try_from(x).unwrap(),
                        viboceros_geometry::Vector3::try_from(y).unwrap(),
                        app.document.tolerance(),
                    )
                    .unwrap(),
                );
            }
            if let Some(setup) = &f.setup {
                enter(&mut app, "Catenary");
                actions(&mut app, setup);
                assert!(
                    app.catenary_session.is_none(),
                    "{id}: {:?}",
                    app.command_log
                );
                app.document.undo().unwrap();
                app.document.clear_history().unwrap();
            }
            assert_model(&app, &native["value"]["before"], id);
            enter(&mut app, "Catenary");
            actions(&mut app, &f.actions);
            assert!(
                app.catenary_session.is_none(),
                "{id}: {:?}",
                app.command_log
            );
            assert_model(&app, &native["value"]["after"], id);
            let options = app.commands.catenary_options();
            assert_eq!(app.document.undo().unwrap().as_deref(), Some("Catenary"));
            assert_model(&app, &native["value"]["undo"], id);
            assert_eq!(app.commands.catenary_options(), options);
            assert!(app.document.undo().unwrap().is_none());
            assert_eq!(app.document.redo().unwrap().as_deref(), Some("Catenary"));
            assert_model(&app, &native["value"]["redo"], id);
            assert_eq!(app.commands.catenary_options(), options);
        }
    }
}
#[test]
fn catenary_frontend_units_retries_preview_cancel_and_command_interruption() {
    let mut app = test_app();
    enter(&mut app, "Catenary");
    for word in [
        "w0,0,0",
        "w10,0,0",
        "w0,0,-1",
        "Mode",
        "Parameter",
        "PointCount",
        "3",
    ] {
        enter(&mut app, word);
    }
    assert_eq!(
        app.catenary_session
            .as_ref()
            .unwrap()
            .input
            .options()
            .point_count,
        20
    );
    enter(&mut app, "8");
    enter(&mut app, "MarkApex");
    let before = app.catenary_session.as_ref().unwrap().input;
    let preview = before.preview(p(0., 0., 4.)).unwrap();
    assert_eq!(app.catenary_session.as_ref().unwrap().input, before);
    assert_eq!(app.document.objects().len(), 0);
    assert!(app.document.undo_label().is_none());
    enter(&mut app, "-4mm");
    assert!(app.catenary_session.is_none());
    assert_eq!(app.document.objects().len(), 2);
    let actual = app
        .document
        .objects()
        .find_map(|o| o.geometry().curve_ref())
        .unwrap()
        .to_nurbs()
        .unwrap();
    assert_eq!(actual, preview.curve.to_nurbs().unwrap());
    let options = app.commands.catenary_options();
    enter(&mut app, "Undo");
    assert_eq!(app.document.objects().len(), 0);
    enter(&mut app, "Catenary");
    enter(&mut app, "Mode=Apex");
    enter(&mut app, "Cancel");
    assert!(app.catenary_session.is_none());
    assert_eq!(app.commands.catenary_options(), options);
    enter(&mut app, "Catenary");
    enter(&mut app, "PointCount");
    enter(&mut app, "Line");
    assert!(app.catenary_session.is_none());
    assert!(matches!(
        app.active_command,
        Some(InteractiveCommand::Line { .. })
    ));
}
#[test]
fn catenary_frontend_enter_cancellation_and_oblique_endpoint_apex_are_distinct() {
    for (mode, end) in [
        ("Length", p(10., 0., 0.)),
        ("Parameter", p(10., 0., 0.)),
        ("Apex", p(10., 0., -2.)),
        ("ThroughPoint", p(10., 0., 0.)),
        ("ThroughPoint", p(10., 0., 2.)),
    ] {
        let mut app = test_app();
        enter(&mut app, &format!("Catenary Mode={mode}"));
        assert!(app.accept_drafting_point(p(0., 0., 0.)));
        assert!(app.accept_drafting_point(end));
        assert!(app.accept_drafting_point(p(0., 0., -1.)));
        enter(&mut app, "Enter");
        assert!(app.catenary_session.is_none(), "{mode} {end:?}");
        assert_eq!(app.document.objects().len(), 0);
        assert!(app.document.undo_label().is_none());
    }
    let mut app = test_app();
    enter(&mut app, "Catenary");
    for word in ["w0,0,0", "w10,0,-2", "w0,0,-1", "MarkApex=Yes"] {
        enter(&mut app, word);
    }
    enter(&mut app, "w10,0,-2");
    assert_eq!(
        app.catenary_session.as_ref().unwrap().input.slot(),
        Slot::Constraint
    );
    assert_eq!(app.document.objects().len(), 0);
    enter(&mut app, "Enter");
    assert!(app.catenary_session.is_none());
    assert_eq!(app.commands.catenary_options().mode, Mode::ThroughPoint);
    assert_eq!(app.document.objects().len(), 2);
}
