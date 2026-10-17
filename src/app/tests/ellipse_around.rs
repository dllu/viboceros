use super::*;
fn enter(app: &mut VibocerosApp, text: &str) {
    app.command_input = text.into();
    app.run_command();
}
#[test]
fn ellipse_around_native_frontend_sources_edges_relative_points_and_history() {
    assert_native_profiles(&[
        (
            include_str!("../../../tools/rhino_oracle/fixtures/ellipse_around_command.json"),
            include_str!(
                "../../../tools/rhino_oracle/observations/ellipse_around_command_rhino_20261017.json"
            ),
            8,
        ),
        (
            include_str!("../../../tools/rhino_oracle/fixtures/ellipse_around_extended.json"),
            include_str!(
                "../../../tools/rhino_oracle/observations/ellipse_around_extended_rhino_20261017.json"
            ),
            19,
        ),
    ]);
}
pub(super) fn assert_native_profiles(profiles: &[(&str, &str, usize)]) {
    assert_native_profiles_for("Ellipse", profiles);
}
pub(super) fn assert_native_profiles_for(command: &str, profiles: &[(&str, &str, usize)]) {
    use viboceros_oracle::EllipseCommandAction as Action;
    for &(q, n, count) in profiles {
        let q: viboceros_oracle::ProbeRequest = serde_json::from_str(q).unwrap();
        let n: serde_json::Value = serde_json::from_str(n).unwrap();
        assert_eq!(q.operations.len(), count);
        for (op, row) in q.operations.iter().zip(n["results"].as_array().unwrap()) {
            let (id, f) = match op {
                viboceros_oracle::Operation::EllipseCommand { id, fixture }
                | viboceros_oracle::Operation::ArcTangent { id, fixture }
                | viboceros_oracle::Operation::ArcCommand { id, fixture }
                | viboceros_oracle::Operation::CircleTangent { id, fixture }
                | viboceros_oracle::Operation::CircleDeform { id, fixture }
                | viboceros_oracle::Operation::CircleAround { id, fixture } => (id, fixture),
                _ => panic!(),
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
            if f.preselect_all {
                for id in &guides {
                    app.document.select_object(*id, SelectionMode::Add).unwrap();
                }
            } else if f.preselect {
                app.document
                    .select_object(guides[0], SelectionMode::Replace)
                    .unwrap();
            }
            app.document.clear_history().unwrap();
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
                enter(&mut app, command);
                super::ellipse::actions(&mut app, setup);
                let output = app
                    .document
                    .objects()
                    .filter(|o| !guides.contains(&o.id()))
                    .map(|o| o.id())
                    .collect::<Vec<_>>();
                app.document.delete_objects(output).unwrap();
                app.document.clear_history().unwrap();
            }
            enter(&mut app, command);
            let mut initial_arc_cursor = false;
            for (action_index, action) in f.actions.iter().enumerate() {
                if !initial_arc_cursor
                    && matches!(op, viboceros_oracle::Operation::ArcCommand { .. })
                    && matches!(action, Action::Hover { .. } | Action::Click { .. })
                {
                    let field = if matches!(action, Action::Hover { .. }) {
                        "motion"
                    } else {
                        "clicks"
                    };
                    let c = row["value"]["input_trace"][field]
                        .as_array()
                        .unwrap()
                        .iter()
                        .find(|c| c["action"] == action_index)
                        .unwrap();
                    if c["initial_projected_point"].is_array() {
                        let p: [f64; 3] =
                            serde_json::from_value(c["initial_projected_point"].clone()).unwrap();
                        app.handle_viewport_action(ViewportOutput {
                            drafting_hover: Some(Point3::try_from(p).unwrap()),
                            ..Default::default()
                        });
                    }
                    initial_arc_cursor = true;
                }
                match action {
                    Action::Click { click } => {
                        let c = row["value"]["input_trace"]["clicks"]
                            .as_array()
                            .unwrap()
                            .iter()
                            .find(|c| c["action"] == action_index)
                            .unwrap();
                        assert_eq!(
                            serde_json::from_value::<[f64; 3]>(c["aim"].clone()).unwrap(),
                            *click
                        );
                        let p: [f64; 3] =
                            serde_json::from_value(c["projected_point"].clone()).unwrap();
                        let p = Point3::try_from(p).unwrap();
                        assert!(
                            app.handle_viewport_action(ViewportOutput {
                                drafting_hover: Some(p),
                                picked_point: Some(p),
                                ..Default::default()
                            }),
                            "{id}"
                        );
                    }
                    Action::Hover { hover } => {
                        let c = row["value"]["input_trace"]["motion"]
                            .as_array()
                            .unwrap()
                            .iter()
                            .find(|c| c["action"] == action_index)
                            .unwrap();
                        assert_eq!(
                            serde_json::from_value::<[f64; 3]>(c["aim"].clone()).unwrap(),
                            *hover
                        );
                        assert_eq!(c["acknowledged"], true);
                        let p: [f64; 3] =
                            serde_json::from_value(c["projected_point"].clone()).unwrap();
                        app.handle_viewport_action(ViewportOutput {
                            drafting_hover: Some(Point3::try_from(p).unwrap()),
                            ..Default::default()
                        });
                    }
                    Action::Pick { curve, edge, .. } => {
                        if matches!(
                            op,
                            viboceros_oracle::Operation::CircleTangent { .. }
                                | viboceros_oracle::Operation::ArcTangent { .. }
                        ) {
                            // Input calibration uses native click rays, never expected geometry.
                            let clicks = row["value"]["input_trace"]["clicks"].as_array().unwrap();
                            let click =
                                clicks.iter().find(|c| c["action"] == action_index).unwrap();
                            let point: [f64; 3] =
                                serde_json::from_value(click["projected_point"].clone()).unwrap();
                            assert!(
                                app.pick_tangent_at(
                                    guides[*curve],
                                    *edge,
                                    Point3::try_from(point).unwrap()
                                ),
                                "{id}"
                            );
                            continue;
                        }
                        let output = if let Some(edge) = edge {
                            ViewportOutput {
                                edge_click: Some(vec![crate::viewport::EdgePick {
                                    object: guides[*curve],
                                    edge: *edge,
                                }]),
                                ..Default::default()
                            }
                        } else {
                            ViewportOutput {
                                selection_click: Some(crate::viewport::SelectionClick {
                                    object_id: Some(guides[*curve]),
                                    mode: SelectionMode::Replace,
                                }),
                                ..Default::default()
                            }
                        };
                        assert!(app.handle_viewport_action(output), "{id}");
                    }
                    Action::Point([x, y, z]) => enter(&mut app, &format!("w{x},{y},{z}")),
                    Action::Relative {
                        relative: [x, y, z],
                    } => enter(&mut app, &format!("r{x},{y},{z}")),
                    Action::Number(value) => enter(&mut app, &value.to_string()),
                    Action::Option(word) => enter(&mut app, word),
                }
                assert!(app.command_input.is_empty(), "{id}: {:?}", app.command_log);
            }
            assert!(
                app.ellipse_session.is_none()
                    && app.tangent_session.is_none()
                    && app.arc_tilted_session.is_none()
                    && app.arc_deform_session.is_none()
                    && app.arc_length_session.is_none()
                    && app.circle_around_session.is_none()
                    && app.circle_deform_session.is_none(),
                "{id}: {:?}",
                app.command_log
            );
            for (stage, replay) in [("after", None), ("undo", Some(false)), ("redo", Some(true))] {
                if let Some(redo) = replay {
                    let label = if redo {
                        app.document.redo()
                    } else {
                        app.document.undo()
                    }
                    .unwrap();
                    assert_eq!(
                        label.as_deref(),
                        (!row["value"]["after_script"]["outputs"]
                            .as_array()
                            .unwrap()
                            .is_empty())
                        .then_some(command)
                    );
                }
                let reference = if stage == "after" {
                    "after_script"
                } else {
                    stage
                };
                super::catenary::assert_model_excluding(
                    &app,
                    &row["value"][reference],
                    id,
                    &guides,
                );
                for (i, source) in sources.iter().enumerate() {
                    let current = app.document.object(source.id()).unwrap();
                    assert_eq!(current, source, "{id}");
                    assert!(
                        source
                            .geometry_snapshot()
                            .shares_storage_with(current.geometry_snapshot()),
                        "{id}"
                    );
                    assert_eq!(
                        app.document.is_selected(source.id()),
                        row["value"][reference]["guide_selected"][i]
                            .as_bool()
                            .unwrap(),
                        "{id} {stage}"
                    );
                }
            }
            if let Some(prompts) = row["value"]["preference_input_trace"]["prompts"].as_array() {
                let prompt = prompts[1].as_str().unwrap();
                let number = |key: &str| {
                    prompt
                        .split_once(key)
                        .unwrap()
                        .1
                        .split_whitespace()
                        .next()
                        .unwrap()
                        .parse::<usize>()
                        .unwrap()
                };
                let d = app.commands.arc_deformation();
                assert_eq!(d.degree, number("Degree="), "{id}: saved degree");
                assert_eq!(d.point_count, number("PointCount="), "{id}: saved count");
            }
            assert_eq!(
                app.document.undo().unwrap().is_some(),
                !row["value"]["after_script"]["outputs"]
                    .as_array()
                    .unwrap()
                    .is_empty()
            );
            assert!(app.document.undo().unwrap().is_none());
        }
    }
}

#[test]
fn ellipse_around_revalidates_guides_before_and_after_center_without_creating_helpers() {
    fn p(x: f64, y: f64, z: f64) -> Point3 {
        Point3::try_new(x, y, z).unwrap()
    }
    for deformable in [false, true] {
        for centered in [false, true] {
            for change in 0..5 {
                let mut app = test_app();
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
                enter(&mut app, "Ellipse AroundCurve");
                if deformable {
                    enter(&mut app, "Deformable");
                }
                assert!(app.picking_ellipse_curve());
                assert!(app.pick_ellipse_curve(Some(id), None));
                if centered {
                    enter(&mut app, "w2,3,5");
                    assert_eq!(app.last_point, Some(p(0., 0., 5.)));
                    assert!(
                        app.ellipse_session
                            .as_ref()
                            .unwrap()
                            .input
                            .reference()
                            .is_none()
                    );
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
                app.validate_ellipse_curve();
                assert!(app.picking_ellipse_curve(), "{centered} {change}");
                assert_eq!(
                    app.ellipse_session.as_ref().unwrap().input.deformable(),
                    deformable
                );
                assert!(app.point_filter.is_none());
                assert!(app.point_constraint.is_none());
                assert!(
                    app.ellipse_session
                        .as_ref()
                        .unwrap()
                        .input
                        .markers()
                        .is_empty()
                );
                assert!(app.document.undo_label().is_none());
                assert!(app.document.objects().all(|o| o.id() == id));
                enter(&mut app, "Cancel");
                assert!(app.ellipse_session.is_none());
                assert!(app.document.undo_label().is_none());
            }
        }
    }
}
#[test]
fn ellipse_around_parameter_pick_and_filtered_axes_share_preview_and_one_history_step() {
    fn p(x: f64, y: f64, z: f64) -> Point3 {
        Point3::try_new(x, y, z).unwrap()
    }
    let mut app = test_app();
    let source = app
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
    enter(&mut app, "Ellipse AroundCurve");
    enter(&mut app, &format!("Curve={source}"));
    let domain = app
        .ellipse_session
        .as_ref()
        .unwrap()
        .cursor_curve()
        .unwrap()
        .domain();
    enter(&mut app, ".z");
    assert!(app.handle_viewport_action(ViewportOutput {
        edge_parameter: Some(0.5 * (*domain.start() + *domain.end())),
        ..Default::default()
    }));
    assert!(app.ellipse_session.as_ref().unwrap().input.requests_curve());
    assert!(app.last_point.is_none());
    assert!(app.point_filter.is_some());
    assert!(app.handle_viewport_action(ViewportOutput {
        edge_parameter: Some(0.5 * (*domain.start() + *domain.end())),
        ..Default::default()
    }));
    assert_eq!(app.last_point, Some(p(0., 0., 5.)));
    for word in ["w3,0,8", ".y", "w0,2,0"] {
        enter(&mut app, word);
        assert!(app.ellipse_session.is_some());
    }
    let input = app.ellipse_session.as_ref().unwrap().input;
    let preview = input.preview(p(1., 2., 9.)).unwrap();
    assert!(app.handle_viewport_action(ViewportOutput {
        picked_point: Some(p(1., 90., 9.)),
        ..Default::default()
    }));
    assert!(app.ellipse_session.is_none());
    let Geometry::Ellipse(actual) = app
        .document
        .objects()
        .find(|o| o.id() != source)
        .unwrap()
        .geometry()
    else {
        panic!()
    };
    for (a, b) in actual
        .quadrants()
        .unwrap()
        .into_iter()
        .zip(preview.ellipse.quadrants().unwrap())
    {
        assert!(a.distance_to(b).unwrap() < 1e-12);
    }
    assert_eq!(app.last_point, Some(p(1., 2., 5.)));
    assert_eq!(app.document.undo().unwrap().as_deref(), Some("Ellipse"));
    assert_eq!(app.document.objects().len(), 1);
    assert!(!app.document.is_selected(source));
    assert!(app.document.undo().unwrap().is_none());
}

#[test]
fn ellipse_around_cancellation_and_mode_switch_release_only_command_picks() {
    fn p(x: f64, y: f64, z: f64) -> Point3 {
        Point3::try_new(x, y, z).unwrap()
    }
    for preselected in [false, true] {
        for picks in 0..3 {
            let mut app = test_app();
            let source = app
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
            if preselected {
                app.document
                    .select_object(source, SelectionMode::Replace)
                    .unwrap();
            }
            app.document.clear_history().unwrap();
            enter(&mut app, "Ellipse AroundCurve");
            if !preselected {
                app.pick_ellipse_curve(Some(source), None);
            }
            if picks > 0 {
                enter(&mut app, "w0,0,5");
            }
            if picks > 1 {
                enter(&mut app, "w3,0,5");
            }
            enter(&mut app, "Cancel");
            assert!(app.ellipse_session.is_none());
            assert_eq!(app.document.objects().len(), 1);
            assert_eq!(app.document.is_selected(source), preselected);
            assert!(app.document.undo_label().is_none());
        }
    }
    let mut app = test_app();
    let source = app
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
    enter(&mut app, "Ellipse AroundCurve");
    app.pick_ellipse_curve(Some(source), None);
    assert!(app.document.is_selected(source));
    enter(&mut app, "Default");
    assert!(!app.document.is_selected(source));
    assert_eq!(
        app.ellipse_session.as_ref().unwrap().input.mode(),
        viboceros_command::ellipse_input::Mode::Default
    );
    assert!(app.document.undo_label().is_none());
}
