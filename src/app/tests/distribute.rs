use super::*;

fn enter(app: &mut VibocerosApp, text: &str) {
    app.command_input = text.to_owned();
    app.run_command();
}
fn setup() -> VibocerosApp {
    let mut app = test_app();
    for command in ["Point 0,0,0", "Point 3,0,0", "Point 10,0,0", "SelAll"] {
        enter(&mut app, command);
    }
    app
}

#[test]
fn bare_distribution_collects_points_with_correction_and_one_undo_step() {
    let mut app = setup();
    enter(&mut app, "Distribute Mode=Center");
    let session = app.distribute_session.as_ref().unwrap();
    assert!(!session.wants_point());
    enter(&mut app, "Direction");
    assert!(app.distribute_session.as_ref().unwrap().wants_point());
    enter(&mut app, "w0,0,0");
    let history = app.document.undo_label().map(str::to_owned);
    enter(&mut app, "w0,0,0");
    assert_eq!(
        app.distribute_session.as_ref().unwrap().anchor(),
        Some(point(0., 0., 0.))
    );
    assert_eq!(app.document.undo_label(), history.as_deref());
    enter(&mut app, "w1,0,0");
    assert!(app.active_command.is_none());
    assert_eq!(
        app.document
            .object(app.document.selected_object_ids().nth(1).unwrap())
            .unwrap()
            .geometry(),
        &Geometry::Point(point(5., 0., 0.))
    );
    assert_eq!(app.document.undo_label(), Some("Distribute"));
    enter(&mut app, "Undo");
    assert_eq!(
        app.document.objects().nth(1).unwrap().geometry(),
        &Geometry::Point(point(3., 0., 0.))
    );
    enter(&mut app, "Redo");
    assert_eq!(
        app.document.objects().nth(1).unwrap().geometry(),
        &Geometry::Point(point(5., 0., 0.))
    );
}

#[test]
fn picked_and_typed_spatial_directions_share_settings_and_geometry() {
    let mut typed = setup();
    let mut picked = setup();
    enter(&mut typed, "Distribute Direction Mode=Center Spacing=2");
    enter(&mut typed, "w1,2,3");
    enter(&mut typed, "w3,5,10");
    assert!(picked.try_start_interactive_command("Distribute Direction Mode=Center Spacing=2"));
    assert!(picked.accept_drafting_point(point(1., 2., 3.)));
    assert!(picked.accept_drafting_point(point(3., 5., 10.)));
    assert!(typed.active_command.is_none() && picked.active_command.is_none());
    assert_eq!(
        typed
            .document
            .objects()
            .map(|o| o.geometry())
            .collect::<Vec<_>>(),
        picked
            .document
            .objects()
            .map(|o| o.geometry())
            .collect::<Vec<_>>()
    );
}

#[test]
fn explicit_axis_dispatch_and_cancellation_do_not_confuse_the_point_prompt() {
    let mut app = setup();
    enter(&mut app, "Distribute XAxis Mode=Center Spacing=3");
    assert!(app.active_command.is_none());
    assert_eq!(
        app.document.objects().nth(2).unwrap().geometry(),
        &Geometry::Point(point(6., 0., 0.))
    );
    enter(&mut app, "Distribute Direction");
    enter(&mut app, "w0,0,0");
    let before = app.document.objects().cloned().collect::<Vec<_>>();
    let history = app.document.undo_label().map(str::to_owned);
    app.cancel_interactive_command(true);
    assert_eq!(app.document.objects().cloned().collect::<Vec<_>>(), before);
    assert_eq!(app.document.undo_label(), history.as_deref());
    assert!(!app.try_start_interactive_command("Distribute Direction Mode=Wrong"));
    assert!(!app.try_start_interactive_command("Distribute Direction Spacing=NaN"));
}

#[test]
fn insufficient_groups_are_rejected_before_collecting_direction_points() {
    let mut app = setup();
    enter(&mut app, "Group Together");
    let original = app.document.objects().cloned().collect::<Vec<_>>();
    let history = app.document.undo_label().map(str::to_owned);
    enter(&mut app, "Distribute");
    assert!(app.active_command.is_none());
    assert_eq!(
        app.document.objects().cloned().collect::<Vec<_>>(),
        original
    );
    assert_eq!(app.document.undo_label(), history.as_deref());
    assert!(
        app.command_log
            .iter()
            .any(|line| line.contains("at least three"))
    );
}

const STATIONS: [[f64; 3]; 4] = [[0., 0., 0.], [3., 8., 1.], [15., 2., 4.], [23., -5., 10.]];

fn spatial_app() -> VibocerosApp {
    let mut app = test_app();
    app.active_viewport = 0;
    app.viewports[0] = Viewport::new(ViewKind::Top);
    app.maximized_viewport = Some(0);
    app.ortho = false;
    app.osnap = false;
    app.smart_track = false;
    app
}

fn add_stations(app: &mut VibocerosApp) -> Vec<ObjectId> {
    STATIONS
        .into_iter()
        .map(|station| {
            app.document
                .add_geometry(Geometry::Point(Point3::try_from(station).unwrap()))
                .unwrap()
        })
        .collect()
}

fn check_locations(
    mut actual: Vec<[f64; 3]>,
    mut expected: Vec<[f64; 3]>,
    case: &str,
    order: bool,
) {
    if !order {
        let less = |a: &[f64; 3], b: &[f64; 3]| a.partial_cmp(b).unwrap();
        actual.sort_by(less);
        expected.sort_by(less);
    }
    assert_eq!(actual.len(), expected.len(), "{case}");
    for (a, b) in actual.iter().zip(expected) {
        assert!(
            a.iter().zip(b).all(|(a, b)| (a - b).abs() <= 1e-8),
            "{case}: {a:?} != {b:?}"
        );
    }
}

#[derive(serde::Deserialize)]
struct Replay {
    name: String,
    inputs: Vec<String>,
    points: Vec<[f64; 3]>,
    grips: bool,
    #[serde(default)]
    postselected: bool,
    #[serde(default)]
    shape: String,
    initial: Option<Vec<[f64; 3]>>,
}

#[test]
fn staged_axis_direction_and_measured_spacing_replay_headless_rhino() {
    use viboceros_document::ControlPointId;
    let cases: Vec<Replay> = serde_json::from_str(include_str!("distribute/rhino.json")).unwrap();
    for case in cases {
        let mut app = spatial_app();
        let initial = case.initial.unwrap_or_else(|| STATIONS.to_vec());
        let ids = if case.grips {
            let points = initial
                .iter()
                .map(|&p| Point3::try_from(p).unwrap())
                .collect();
            let tolerance = app.document.tolerance();
            let geometry = match case.shape.as_str() {
                "mesh" | "subd" => {
                    use viboceros_geometry::{MeshFace, SubD, TriangleMesh};
                    let mesh = TriangleMesh::try_new_faces(
                        points,
                        vec![MeshFace::Quad([0, 1, 2, 3])],
                        tolerance,
                    )
                    .unwrap();
                    if case.shape == "subd" {
                        Geometry::SubD(
                            SubD::try_from_mesh(&mesh, Default::default(), tolerance).unwrap(),
                        )
                    } else {
                        Geometry::Mesh(mesh)
                    }
                }
                _ => Geometry::Polyline(
                    viboceros_geometry::Polyline3::try_new(points, tolerance).unwrap(),
                ),
            };
            let id = app.document.add_geometry(geometry).unwrap();
            app.document.enable_control_points([id]).unwrap();
            app.document
                .select_control_points(
                    (0..4).map(|index| ControlPointId { object: id, index }),
                    SelectionMode::Replace,
                )
                .unwrap();
            vec![id]
        } else {
            let ids = add_stations(&mut app);
            if !case.postselected {
                app.document
                    .select_objects_direct(ids.iter().copied(), SelectionMode::Replace)
                    .unwrap();
            }
            ids
        };
        app.document.clear_history().unwrap();

        for input in &case.inputs {
            if input == "@select" {
                app.select_prompt_objects(ids.iter().copied(), SelectionMode::Add);
                enter(&mut app, "");
            } else {
                enter(&mut app, input);
            }
        }
        assert!(
            app.command_line_idle(),
            "{}: {:?}",
            case.name,
            app.command_log
        );
        let locations = |app: &VibocerosApp| {
            assert_eq!(
                app.document.objects().map(|o| o.id()).collect::<Vec<_>>(),
                ids
            );
            if case.grips {
                assert_eq!(app.document.selected_control_points().count(), 4);
                app.document
                    .control_point_locations(ids[0])
                    .unwrap()
                    .iter()
                    .map(|p| p.to_array())
                    .collect::<Vec<_>>()
            } else {
                if !case.postselected {
                    assert_eq!(app.document.selected_object_count(), 4);
                }
                app.document
                    .objects()
                    .map(|object| {
                        let Geometry::Point(p) = object.geometry() else {
                            panic!("expected point object")
                        };
                        p.to_array()
                    })
                    .collect::<Vec<_>>()
            }
        };
        if case.postselected {
            assert_eq!(
                app.document.selected_object_ids().collect::<Vec<_>>(),
                [ids[0], ids[3]]
            );
        }
        check_locations(locations(&app), case.points.clone(), &case.name, case.grips);
        assert_eq!(app.document.undo_label(), Some("Distribute"));
        app.document.undo().unwrap();
        if case.postselected {
            assert_eq!(app.document.selected_object_count(), 0);
        }
        check_locations(locations(&app), initial, &case.name, case.grips);
        assert!(!app.document.can_undo());
        app.document.redo().unwrap();
        if case.postselected {
            assert_eq!(app.document.selected_object_count(), 0);
        }
        check_locations(locations(&app), case.points, &case.name, case.grips);
    }
}

#[test]
fn command_first_collects_more_units_and_cancel_rolls_back_option_edits() {
    let mut app = spatial_app();
    let ids = add_stations(&mut app);
    app.document.clear_history().unwrap();
    enter(&mut app, "Distribute");
    app.select_prompt_objects(ids[..2].iter().copied(), SelectionMode::Add);
    enter(&mut app, "");
    assert!(app.selecting_distribution_sources());
    app.select_prompt_objects(ids[2..].iter().copied(), SelectionMode::Add);
    enter(&mut app, "");
    assert!(!app.selecting_distribution_sources());
    for input in ["Mode", "Center", "Spacing", "4", "XAxis"] {
        enter(&mut app, input);
    }
    assert!(app.command_line_idle(), "{:?}", app.command_log);
    assert_eq!(
        app.document.selected_object_ids().collect::<Vec<_>>(),
        ids[..1]
    );
    let settings = app.distribute_settings;
    assert_eq!(settings.mode, viboceros_command::DistributionMode::Center);
    assert_eq!(settings.spacing, Some(4.));
    let geometry_before_cancel = app.document.objects().cloned().collect::<Vec<_>>();
    let history = app.document.undo_label().map(str::to_owned);
    app.document
        .select_objects_direct(ids, SelectionMode::Replace)
        .unwrap();
    for input in [
        "Distribute",
        "Mode=Gap",
        "Spacing",
        "Automatic",
        "Direction",
        "w0,0,0",
        "Cancel",
    ] {
        enter(&mut app, input);
    }
    assert_eq!(app.distribute_settings, settings);
    assert_eq!(
        app.document.objects().cloned().collect::<Vec<_>>(),
        geometry_before_cancel
    );
    assert_eq!(app.document.undo_label(), history.as_deref());
}

#[test]
fn preview_follows_the_viewport_and_matches_the_accepted_layout() {
    use super::circle_preview::{screen_point, viewport_click, viewport_frame};
    let mut app = spatial_app();
    app.grid_snap = true;
    let ids = add_stations(&mut app);
    app.document
        .select_objects_direct(ids.iter().copied(), SelectionMode::Replace)
        .unwrap();
    app.document.clear_history().unwrap();
    let ctx = egui::Context::default();
    viewport_frame(&mut app, &ctx, Vec::new());
    viewport_frame(&mut app, &ctx, Vec::new());
    app.viewports[0]
        .zoom_extents(&app.document, Default::default())
        .unwrap();
    enter(&mut app, "Distribute Direction");
    enter(&mut app, "w0,0,0");
    let mut preview_layout = None;
    for endpoint in [point(8., 0., 0.), point(6., 4., 0.)] {
        viewport_frame(&mut app, &ctx, Vec::new());
        let screen = screen_point(&app, endpoint);
        viewport_frame(&mut app, &ctx, vec![egui::Event::PointerMoved(screen)]);
        let layout = app
            .distribute_session
            .as_ref()
            .unwrap()
            .layout_preview()
            .unwrap();
        let next = layout.translations().collect::<Vec<_>>();
        assert!(!next.is_empty());
        assert!(preview_layout.as_ref().is_none_or(|old| old != &next));
        preview_layout = Some(next);
    }
    let original = app.document.objects().cloned().collect::<Vec<_>>();
    assert!(!app.document.can_undo());
    let target = screen_point(&app, point(6., 4., 0.));
    viewport_click(&mut app, &ctx, target);
    assert!(app.command_line_idle(), "{:?}", app.command_log);
    for (id, transform) in preview_layout.unwrap() {
        let Geometry::Point(original_point) =
            original.iter().find(|o| o.id() == id).unwrap().geometry()
        else {
            panic!()
        };
        assert_eq!(
            app.document.object(id).unwrap().geometry(),
            &Geometry::Point(transform.transform_point(*original_point).unwrap())
        );
    }
    app.document.undo().unwrap();
    assert_eq!(
        app.document.objects().cloned().collect::<Vec<_>>(),
        original
    );
}

#[test]
fn component_distribution_moves_unwelded_mesh_aliases_as_one_station() {
    use viboceros_command::component_selection::{
        ComponentAction, ComponentPick, ComponentSelectionKind,
    };
    use viboceros_geometry::{MeshFace, TriangleMesh};
    let mut app = spatial_app();
    let raw = [
        point(0., 0., 0.),
        point(3., -2., 0.),
        point(18., 3., 0.),
        point(14., 10., 0.),
        point(0., 0., 0.),
        point(18., 3., 0.),
    ];
    let mesh = TriangleMesh::try_new_faces(
        raw.to_vec(),
        vec![MeshFace::Triangle([0, 1, 2]), MeshFace::Triangle([4, 5, 3])],
        app.document.tolerance(),
    )
    .unwrap();
    let topology = mesh.topology_vertex_points();
    let id = app.document.add_geometry(Geometry::Mesh(mesh)).unwrap();
    let component_selection = app.commands.component_selection();
    component_selection
        .apply(
            &app.document,
            (0..topology.len())
                .map(|index| ComponentPick {
                    object: id,
                    kind: ComponentSelectionKind::MeshVertex,
                    index,
                })
                .collect(),
            ComponentAction::Add,
        )
        .unwrap();
    app.document.clear_history().unwrap();
    for input in ["Distribute", "Spacing", "4", "XAxis"] {
        enter(&mut app, input);
    }
    assert!(app.command_line_idle(), "{:?}", app.command_log);
    let Geometry::Mesh(mesh) = app.document.object(id).unwrap().geometry() else {
        panic!()
    };
    assert_eq!(
        mesh.vertices().iter().map(|p| p.x()).collect::<Vec<_>>(),
        [0., 4., 12., 8., 0., 12.]
    );
    assert_eq!(mesh.vertices()[2], mesh.vertices()[5]);
    assert!(
        app.commands
            .component_selection()
            .live_picks(&app.document)
            .is_empty()
    );
    app.document.undo().unwrap();
    let Geometry::Mesh(mesh) = app.document.object(id).unwrap().geometry() else {
        panic!()
    };
    assert_eq!(mesh.vertices(), raw);
}
