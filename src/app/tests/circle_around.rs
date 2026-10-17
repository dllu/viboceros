use super::*;
#[test]
fn circle_around_native_frontend_size_modes_source_selection_and_history() {
    super::ellipse_around::assert_native_profiles_for(
        "Circle",
        &[
            (
                include_str!("../../../tools/rhino_oracle/fixtures/circle_around_command.json"),
                include_str!(
                    "../../../tools/rhino_oracle/observations/circle_around_command_rhino_20261017.json"
                ),
                10,
            ),
            (
                include_str!("../../../tools/rhino_oracle/fixtures/circle_around_extended.json"),
                include_str!(
                    "../../../tools/rhino_oracle/observations/circle_around_extended_rhino_20261017.json"
                ),
                22,
            ),
            (
                include_str!("../../../tools/rhino_oracle/fixtures/circle_around_numeric.json"),
                include_str!(
                    "../../../tools/rhino_oracle/observations/circle_around_numeric_rhino_20261017.json"
                ),
                9,
            ),
            (
                include_str!("../../../tools/rhino_oracle/fixtures/circle_around_memory.json"),
                include_str!(
                    "../../../tools/rhino_oracle/observations/circle_around_memory_rhino_20261017.json"
                ),
                2,
            ),
        ],
    );
}

fn p(x: f64, y: f64, z: f64) -> Point3 {
    Point3::try_new(x, y, z).unwrap()
}
fn enter(app: &mut VibocerosApp, text: &str) {
    app.command_input = text.into();
    app.run_command();
}
fn guide(app: &mut VibocerosApp) -> ObjectId {
    let id = app
        .document
        .add_geometry(Geometry::Line(
            viboceros_geometry::LineSegment::try_new(
                p(0., 0., 0.),
                p(0., 0., 10.),
                app.document.tolerance(),
            )
            .unwrap(),
        ))
        .unwrap();
    app.document.clear_history().unwrap();
    id
}
#[test]
fn circle_around_parameter_filters_preview_seam_and_history_use_the_constrained_point() {
    let mut app = test_app();
    let source = guide(&mut app);
    let saved = app.document.object(source).unwrap().clone();
    enter(&mut app, "Circle AroundCurve");
    enter(&mut app, &format!("Curve={source}"));
    let domain = app
        .circle_around_session
        .as_ref()
        .unwrap()
        .cursor_curve()
        .unwrap()
        .domain();
    let middle = 0.5 * (*domain.start() + *domain.end());
    enter(&mut app, ".z");
    assert!(app.handle_viewport_action(ViewportOutput {
        edge_parameter: Some(middle),
        ..Default::default()
    }));
    assert!(app.circle_around_session.as_ref().unwrap().input.is_none());
    assert!(app.last_point.is_none());
    assert!(app.handle_viewport_action(ViewportOutput {
        edge_parameter: Some(middle),
        ..Default::default()
    }));
    assert_eq!(app.last_point, Some(p(0., 0., 5.)));
    assert!(
        app.circle_around_session
            .as_ref()
            .unwrap()
            .cursor_curve()
            .is_none()
    );
    for word in ["Diameter", ".y", "w0,3,0"] {
        enter(&mut app, word);
    }
    assert!(app.point_filter.is_some());
    let input = app.circle_around_session.as_ref().unwrap().input.unwrap();
    let preview = input.preview(p(4., 3., 50.)).unwrap();
    assert!(app.handle_viewport_action(ViewportOutput {
        picked_point: Some(p(4., 99., 50.)),
        ..Default::default()
    }));
    assert!(app.circle_around_session.is_none());
    assert_eq!(app.last_point, Some(p(4., 3., 5.)));
    let Geometry::Circle(actual) = app
        .document
        .objects()
        .find(|o| o.id() != source)
        .unwrap()
        .geometry()
    else {
        panic!()
    };
    assert_eq!(actual, &preview);
    assert_eq!(actual.radius(), 5.);
    assert_eq!(app.commands.circle_preferences().radius, 5.);
    assert!(app.commands.circle_preferences().diameter);
    assert!(!app.document.is_selected(source));
    assert_eq!(app.document.undo().unwrap().as_deref(), Some("Circle"));
    assert_eq!(app.document.objects().len(), 1);
    assert!(app.document.undo().unwrap().is_none());
    assert_eq!(app.document.redo().unwrap().as_deref(), Some("Circle"));
    assert_eq!(app.document.object(source).unwrap(), &saved);
}
#[test]
fn circle_around_stale_guides_clear_helpers_and_filters_before_and_after_center() {
    for deformable in [false, true] {
        for centered in [false, true] {
            for change in 0..5 {
                let mut app = test_app();
                let id = guide(&mut app);
                if deformable {
                    enter(&mut app, "Circle Deformable Degree=4 PointCount=12");
                    enter(&mut app, "AroundCurve");
                } else {
                    enter(&mut app, "Circle AroundCurve");
                }
                assert!(app.pick_circle_around_curve(Some(id), None));
                if centered {
                    enter(&mut app, "w2,3,5");
                    enter(&mut app, "Diameter");
                }
                enter(&mut app, ".y");
                assert!(app.point_filter.is_some());
                match change {
                    0 => {
                        app.document
                            .replace_object_geometries([(
                                id,
                                Geometry::Line(
                                    viboceros_geometry::LineSegment::try_new(
                                        p(0., 0., 0.),
                                        p(10., 0., 0.),
                                        app.document.tolerance(),
                                    )
                                    .unwrap(),
                                ),
                            )])
                            .unwrap();
                    }
                    1 => {
                        app.document.set_objects_visibility([id], false).unwrap();
                    }
                    2 => {
                        app.document.set_objects_locked([id], true).unwrap();
                    }
                    3 => {
                        app.document
                            .set_tolerance(Tolerance::try_new(2e-9, 1e-12, 1e-10).unwrap());
                    }
                    _ => {
                        app.document.delete_objects([id]).unwrap();
                    }
                }
                app.document.clear_history().unwrap();
                app.validate_circle_around_curve();
                assert!(app.picking_circle_around_curve());
                assert!(app.circle_around_session.as_ref().unwrap().input.is_none());
                if deformable {
                    assert_eq!(
                        app.circle_around_session
                            .as_ref()
                            .unwrap()
                            .output
                            .unwrap()
                            .deformation
                            .degree,
                        4
                    );
                }
                assert!(app.point_filter.is_none());
                assert!(app.point_constraint.is_none());
                assert!(app.document.objects().all(|o| o.id() == id));
                assert!(app.document.undo_label().is_none());
                enter(&mut app, "Cancel");
                assert!(app.circle_around_session.is_none());
                assert!(app.document.undo().unwrap().is_none());
            }
        }
    }
}

#[test]
fn circle_around_cancel_interrupt_and_size_errors_release_only_postselected_guides() {
    for preselect in [false, true] {
        for centered in [false, true] {
            let mut app = test_app();
            let id = guide(&mut app);
            if preselect {
                app.document
                    .select_object(id, SelectionMode::Replace)
                    .unwrap();
            }
            enter(&mut app, "Circle AroundCurve");
            if !preselect {
                app.pick_circle_around_curve(Some(id), None);
            }
            if centered {
                enter(&mut app, "w0,0,5");
                let before = app.circle_around_session.as_ref().unwrap().input.unwrap();
                for word in ["0", "-3", "NaN", "w0,0,9", "Orientation"] {
                    enter(&mut app, word);
                    assert_eq!(
                        app.circle_around_session.as_ref().unwrap().input,
                        Some(before)
                    );
                }
            }
            enter(&mut app, "Line");
            assert!(app.circle_around_session.is_none());
            assert_eq!(app.document.is_selected(id), preselect);
            assert!(app.document.objects().all(|o| o.id() == id));
            assert!(app.document.undo().unwrap().is_none());
        }
    }
}

#[test]
fn circle_around_area_uses_square_model_units_and_length_sizes_accept_units() {
    let mut app = test_app();
    let id = guide(&mut app);
    for word in [
        "Circle AroundCurve",
        &format!("Curve={id}"),
        "w0,0,5",
        "Area",
        "9mm",
    ] {
        enter(&mut app, word);
    }
    assert!(app.circle_around_session.is_some());
    assert_eq!(app.document.objects().len(), 1);
    enter(&mut app, "9*pi");
    assert!(app.circle_around_session.is_none());
    assert!((app.commands.circle_preferences().radius - 3.).abs() < 1e-12);
    app.document.undo().unwrap();
    for word in [
        "Circle AroundCurve",
        &format!("Curve={id}"),
        "w0,0,5",
        "Diameter",
        "6mm",
    ] {
        enter(&mut app, word);
    }
    assert!(app.circle_around_session.is_none());
    assert_eq!(app.commands.circle_preferences().radius, 3.);
}

#[test]
fn circle_around_native_cancellations_clear_command_picks_and_leave_history_empty() {
    use viboceros_oracle::CircleAroundAction as Action;
    let q: viboceros_oracle::ProbeRequest = serde_json::from_str(include_str!(
        "../../../tools/rhino_oracle/fixtures/circle_around_controls.json"
    ))
    .unwrap();
    let n: serde_json::Value = serde_json::from_str(include_str!(
        "../../../tools/rhino_oracle/observations/circle_around_controls_rhino_20261017.json"
    ))
    .unwrap();
    assert_eq!(q.operations.len(), 5);
    for (op, row) in q.operations.iter().zip(n["results"].as_array().unwrap()) {
        let viboceros_oracle::Operation::CircleAround { id, fixture: f } = op else {
            panic!()
        };
        let mut app = test_app();
        let source = guide(&mut app);
        if f.preselect {
            app.document
                .select_object(source, SelectionMode::Replace)
                .unwrap();
        }
        enter(&mut app, "Circle");
        for action in &f.actions {
            match action {
                Action::Option(w) => enter(&mut app, w),
                Action::Pick { edge: None, .. } => enter(&mut app, &format!("Curve={source}")),
                Action::Point([x, y, z]) => enter(&mut app, &format!("w{x},{y},{z}")),
                _ => panic!(),
            }
        }
        assert!(app.circle_around_session.is_none());
        assert!(app.active_command.is_none());
        assert_eq!(app.document.is_selected(source), f.preselect);
        super::catenary::assert_model_excluding(&app, &row["value"]["after_script"], id, &[source]);
        assert!(app.document.undo().unwrap().is_none());
    }
}
