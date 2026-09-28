use super::*;
use viboceros_geometry::Vector3;

fn enter(app: &mut VibocerosApp, text: &str) {
    app.command_input = text.to_owned();
    app.run_command();
}

#[test]
fn cplane_all_and_through_all_preserve_each_views_axes_and_history() {
    let mut app = test_app();
    enter(&mut app, "Line");
    enter(&mut app, "w1,2,3");
    let pending = app.active_command;
    let cameras = app
        .viewports
        .iter()
        .map(Viewport::camera_snapshot)
        .collect::<Vec<_>>();
    let axes = app
        .viewports
        .iter()
        .map(|view| view.construction_plane().axes())
        .collect::<Vec<_>>();
    enter(&mut app, "CPlane All w4,5,6");
    for (index, view) in app.viewports.iter().enumerate() {
        assert_eq!(view.construction_plane().origin(), point(4., 5., 6.));
        assert_eq!(view.construction_plane().axes(), axes[index]);
    }
    enter(&mut app, "CPlane Through All w7,8,9");
    let expected = [
        point(4., 5., 9.),
        point(4., 5., 9.),
        point(4., 8., 6.),
        point(7., 5., 6.),
    ];
    for (index, view) in app.viewports.iter().enumerate() {
        assert_eq!(view.construction_plane().origin(), expected[index]);
        assert_eq!(view.construction_plane().axes(), axes[index]);
    }
    assert_eq!(
        app.viewports
            .iter()
            .map(Viewport::camera_snapshot)
            .collect::<Vec<_>>(),
        cameras
    );
    assert_eq!(app.active_command, pending);
    assert_eq!(app.document.undo_label(), None);
    app.active_viewport = 2;
    enter(&mut app, "CPlane Undo");
    assert_eq!(
        app.viewports[2].construction_plane().origin(),
        point(4., 5., 6.)
    );
    assert_eq!(app.viewports[0].construction_plane().origin(), expected[0]);
    enter(&mut app, "CPlane Redo");
    assert_eq!(app.viewports[2].construction_plane().origin(), expected[2]);
}

#[test]
fn cplane_all_prompts_resolve_typed_world_points() {
    let mut app = test_app();
    enter(&mut app, "CPlane All");
    assert!(app.plane_prompt.is_some());
    enter(&mut app, "w3,4,5");
    assert!(app.plane_prompt.is_none());
    assert!(
        app.viewports
            .iter()
            .all(|view| view.construction_plane().origin() == point(3., 4., 5.))
    );
    enter(&mut app, "CPlane Through All");
    assert!(app.plane_prompt.is_some());
    enter(&mut app, "w7,8,9");
    assert!(app.plane_prompt.is_none());
    assert_eq!(
        app.viewports[0].construction_plane().origin(),
        point(3., 4., 9.)
    );
    assert_eq!(
        app.viewports[2].construction_plane().origin(),
        point(3., 8., 5.)
    );
    assert_eq!(
        app.viewports[3].construction_plane().origin(),
        point(7., 4., 5.)
    );
}

#[test]
fn cplane_view_aligns_only_the_active_plane_without_moving_its_camera() {
    let mut app = test_app();
    app.active_viewport = 1;
    let old = app.viewports[1].construction_plane();
    let other = app.viewports[0].construction_plane();
    let expected = app.viewports[1]
        .construction_plane_aligned_to_view()
        .unwrap();
    let camera = app.viewports[1].camera_snapshot();
    enter(&mut app, "CPlane View");
    assert_eq!(app.viewports[1].construction_plane(), expected);
    assert_eq!(app.viewports[1].camera_snapshot(), camera);
    assert_eq!(app.viewports[0].construction_plane(), other);
    enter(&mut app, "CPlane Undo");
    assert_eq!(app.viewports[1].construction_plane(), old);
    assert_eq!(app.viewports[1].camera_snapshot(), camera);
    enter(&mut app, "CPlane");
    enter(&mut app, "_View");
    assert!(app.plane_prompt.is_none());
    assert_eq!(app.viewports[1].construction_plane(), expected);
    assert_eq!(app.viewports[1].camera_snapshot(), camera);
}

#[test]
fn cplane_three_point_options_complete_after_one_direction_pick() {
    let mut app = test_app();
    let camera = app.viewports[0].camera_snapshot();
    enter(&mut app, "CPlane 3Point");
    enter(&mut app, "w1,2,3");
    enter(&mut app, "Vertical");
    enter(&mut app, "w1,2,6");
    assert!(app.plane_prompt.is_some());
    assert_eq!(
        app.viewports[0].construction_plane(),
        viboceros_command::construction_plane::WorldPlane::Top.frame()
    );
    enter(&mut app, "w4,5,7");
    assert!(app.plane_prompt.is_none());
    let vertical = app.viewports[0].construction_plane();
    assert_eq!(vertical.origin(), point(1., 2., 3.));
    assert_eq!(vertical.y_axis().as_vector().to_array(), [0., 0., 1.]);
    assert_eq!(app.viewports[0].camera_snapshot(), camera);
    enter(&mut app, "CPlane Undo");
    assert_eq!(
        app.viewports[0].construction_plane(),
        viboceros_command::construction_plane::WorldPlane::Top.frame()
    );

    enter(&mut app, "CPlane 3Point");
    enter(&mut app, "w1,2,3");
    enter(&mut app, "ZAxis");
    enter(&mut app, "w1,2,3");
    assert!(app.plane_prompt.is_some());
    enter(&mut app, "w1,5,3");
    assert!(app.plane_prompt.is_none());
    assert_eq!(
        app.viewports[0]
            .construction_plane()
            .x_axis()
            .as_vector()
            .to_array(),
        [0., 0., 1.]
    );
    assert_eq!(
        app.viewports[0]
            .construction_plane()
            .z_axis()
            .as_vector()
            .to_array(),
        [0., 1., 0.]
    );
    assert_eq!(app.viewports[0].camera_snapshot(), camera);
}

#[test]
fn cplane_rotate_accepts_picked_angle_references() {
    let mut app = test_app();
    let camera = app.viewports[0].camera_snapshot();
    enter(&mut app, "CPlane 3Point w2,0,3 w3,0,3 w2,1,3");
    enter(&mut app, "CPlane Rotate");
    for picked in [point(0., 0., 0.), point(0., 0., 1.), point(1., 0., 0.)] {
        assert!(app.plane_prompt.as_ref().unwrap().requests_point());
        app.accept_plane_prompt_point(picked);
    }
    assert!(app.plane_prompt.is_some());
    app.accept_plane_prompt_point(point(0., 1., 0.));
    assert!(app.plane_prompt.is_none());
    assert!(
        app.viewports[0]
            .construction_plane()
            .origin()
            .distance_to(point(0., 2., 3.))
            .unwrap()
            < 1e-14
    );
    assert_eq!(app.viewports[0].camera_snapshot(), camera);
    enter(&mut app, "CPlane Undo");
    assert_eq!(
        app.viewports[0].construction_plane().origin(),
        point(2., 0., 3.)
    );
}

#[test]
fn cplane_rotate_rejects_axis_aligned_references_without_losing_the_prompt() {
    let mut app = test_app();
    let original = app.viewports[0].construction_plane();
    enter(&mut app, "CPlane Rotate");
    app.accept_plane_prompt_point(point(0., 0., 0.));
    app.accept_plane_prompt_point(point(0., 0., 1.));
    assert!(!app.accept_plane_prompt_point(point(0., 0., 5.)));
    assert_eq!(app.plane_prompt.as_ref().unwrap().points.len(), 2);
    assert_eq!(app.viewports[0].construction_plane(), original);
    app.accept_plane_prompt_point(point(1., 0., 0.));
    assert!(!app.accept_plane_prompt_point(point(0., 0., -2.)));
    assert_eq!(app.plane_prompt.as_ref().unwrap().points.len(), 3);
    assert_eq!(app.viewports[0].construction_plane(), original);
    app.accept_plane_prompt_point(point(0., 1., 0.));
    assert!(app.plane_prompt.is_none());
    assert!(
        app.viewports[0]
            .construction_plane()
            .x_axis()
            .as_vector()
            .dot(Vector3::try_new(0., 1., 0.).unwrap())
            .unwrap()
            > 1.0 - 1e-14
    );
}

#[test]
fn cplane_object_uses_typed_ids_preselection_and_viewport_picks() {
    let mut app = test_app();
    let original = app.viewports[0].construction_plane();
    let camera = app.viewports[0].camera_snapshot();
    enter(&mut app, "Circle 2,3,4 5");
    let id = app.document.objects().next().unwrap().id();
    enter(&mut app, &format!("CPlane Object {id}"));
    assert_eq!(
        app.viewports[0].construction_plane().origin(),
        point(2., 3., 4.)
    );
    assert_eq!(app.viewports[0].camera_snapshot(), camera);
    enter(&mut app, "CPlane Undo");
    assert_eq!(app.viewports[0].construction_plane(), original);

    app.document
        .select_object(id, SelectionMode::Replace)
        .unwrap();
    enter(&mut app, "CPlane Object");
    assert!(app.plane_prompt.is_none());
    assert_eq!(
        app.viewports[0].construction_plane().origin(),
        point(2., 3., 4.)
    );
    assert_eq!(
        app.document.selected_object_ids().collect::<Vec<_>>(),
        vec![id]
    );

    enter(&mut app, "CPlane Undo");
    app.document.clear_selection();
    enter(&mut app, "CPlane Object");
    assert!(app.plane_prompt.as_ref().unwrap().requests_object());
    assert!(!app.plane_prompt.as_ref().unwrap().requests_point());
    let missing = "00000000-0000-0000-0000-000000000001".parse().unwrap();
    assert!(!app.accept_plane_prompt_object(missing));
    assert!(app.plane_prompt.is_some());
    assert_eq!(app.viewports[0].construction_plane(), original);
    app.apply_selection_click(SelectionClick {
        object_id: Some(id),
        mode: SelectionMode::Replace,
    });
    assert!(app.plane_prompt.is_none());
    assert_eq!(
        app.viewports[0].construction_plane().origin(),
        point(2., 3., 4.)
    );
    assert_eq!(app.document.selected_object_count(), 0);

    enter(&mut app, "CPlane Undo");
    enter(&mut app, "CPlane");
    enter(&mut app, "Object");
    assert!(app.plane_prompt.as_ref().unwrap().requests_object());
    app.apply_selection_click(SelectionClick {
        object_id: Some(id),
        mode: SelectionMode::Replace,
    });
    assert_eq!(
        app.viewports[0].construction_plane().origin(),
        point(2., 3., 4.)
    );
}

#[test]
fn cplane_object_pick_returns_to_the_suspended_polyline() {
    let mut app = test_app();
    enter(&mut app, "Circle 2,3,4 5");
    let id = app.document.objects().next().unwrap().id();
    enter(&mut app, "Polyline");
    enter(&mut app, "0,0");
    enter(&mut app, "1,0");
    let prior = app.curve_points.clone();
    enter(&mut app, "CPlane Object");
    assert!(app.plane_prompt.as_ref().unwrap().requests_object());
    assert_eq!(app.curve_points, prior);
    app.apply_selection_click(SelectionClick {
        object_id: Some(id),
        mode: SelectionMode::Replace,
    });
    assert_eq!(app.active_command, Some(InteractiveCommand::Polyline));
    assert_eq!(app.curve_points, prior);
    enter(&mut app, "0,1");
    enter(&mut app, "");
    let Geometry::Polyline(polyline) = app.document.objects().last().unwrap().geometry() else {
        panic!("expected a polyline");
    };
    assert_eq!(polyline.vertices()[0], point(0., 0., 0.));
    assert_eq!(polyline.vertices()[1], point(1., 0., 0.));
    assert_eq!(polyline.vertices()[2], point(2., 4., 4.));
}

#[test]
fn cplane_object_aligns_to_a_typed_or_picked_mesh_face() {
    let mut app = test_app();
    let mesh = TriangleMesh::try_new_faces(
        vec![
            point(0., 0., 0.),
            point(4., 0., 0.),
            point(0., 2., 0.),
            point(0., 0., 3.),
        ],
        vec![MeshFace::Triangle([0, 1, 2]), MeshFace::Triangle([0, 3, 1])],
        app.document.tolerance(),
    )
    .unwrap();
    let id = app.document.add_geometry(Geometry::Mesh(mesh)).unwrap();
    let original = app.viewports[0].construction_plane();
    let camera = app.viewports[0].camera_snapshot();
    enter(&mut app, &format!("CPlane Object {id} Face=1"));
    assert!(
        app.viewports[0]
            .construction_plane()
            .origin()
            .distance_to(point(4. / 3., 0., 1.))
            .unwrap()
            < 1e-14
    );
    enter(&mut app, "CPlane Undo");
    assert_eq!(app.viewports[0].construction_plane(), original);

    enter(&mut app, "CPlane Object");
    assert!(app.plane_prompt.as_ref().unwrap().requests_object());
    assert!(!app.accept_plane_prompt_object_face(id, 99));
    assert!(app.plane_prompt.is_some());
    assert_eq!(app.viewports[0].construction_plane(), original);
    assert!(app.handle_viewport_action(ViewportOutput {
        face_click: Some((id, 1)),
        ..Default::default()
    }));
    assert!(app.plane_prompt.is_none());
    assert_eq!(app.document.selected_object_count(), 0);
    assert_eq!(app.viewports[0].camera_snapshot(), camera);
    assert!(
        app.viewports[0]
            .construction_plane()
            .z_axis()
            .as_vector()
            .dot(Vector3::try_new(0., 1., 0.).unwrap())
            .unwrap()
            > 1.0 - 1e-14
    );
}

#[test]
fn cplane_all_settings_survive_prompts_and_apply_to_later_commands() {
    let mut app = test_app();
    enter(&mut app, "CPlane All=Yes");
    assert_eq!(app.cplane_options.origin_all, true);
    app.cancel_plane_prompt();
    enter(&mut app, "CPlane Through w1,2,3");
    assert_eq!(app.cplane_options.through_all, false);
    assert_eq!(
        app.viewports[0].construction_plane().origin(),
        point(0., 0., 3.)
    );
    assert_eq!(
        app.viewports[2].construction_plane().origin(),
        point(0., 0., 0.)
    );
    enter(&mut app, "CPlane w4,5,6");
    assert!(
        app.viewports
            .iter()
            .all(|view| view.construction_plane().origin() == point(4., 5., 6.))
    );
    enter(&mut app, "CPlane Through All=Yes");
    app.cancel_plane_prompt();
    enter(&mut app, "CPlane Through w7,8,9");
    assert_eq!(
        app.viewports[0].construction_plane().origin(),
        point(4., 5., 9.)
    );
    assert_eq!(
        app.viewports[2].construction_plane().origin(),
        point(4., 8., 6.)
    );
    enter(&mut app, "CPlane All=No w0,0,0");
    assert_eq!(app.cplane_options.origin_all, false);
    assert_eq!(
        app.viewports[0].construction_plane().origin(),
        point(0., 0., 0.)
    );
    assert_eq!(
        app.viewports[2].construction_plane().origin(),
        point(4., 8., 6.)
    );
    enter(&mut app, "CPlane Through All=No");
    app.cancel_plane_prompt();
    assert_eq!(app.cplane_options.through_all, false);
}

#[test]
fn cplane_through_all_rejects_unrepresentable_target_atomically() {
    let mut app = test_app();
    let extreme = app.viewports[3]
        .construction_plane()
        .with_origin(point(f64::MAX, 0., 0.));
    app.viewports[3].plane.set(extreme);
    let before = app
        .viewports
        .iter()
        .map(Viewport::construction_plane)
        .collect::<Vec<_>>();
    assert!(!app.apply_plane_action(
        viboceros_command::construction_plane::PlaneAction::SetThroughAll(point(-f64::MAX, 0., 0.)),
        0,
    ));
    assert_eq!(
        app.viewports
            .iter()
            .map(Viewport::construction_plane)
            .collect::<Vec<_>>(),
        before
    );
    assert!(app.command_log.back().unwrap().starts_with("Error:"));
}

#[test]
fn synchronize_cplanes_rotates_standard_planes_without_moving_cameras() {
    use viboceros_command::construction_plane::WorldPlane;

    let mut app = test_app();
    let source = Frame3::try_from_directions(
        point(7., 8., 9.),
        Vector3::try_from([0., 1., 0.]).unwrap(),
        Vector3::try_from([0., 0., 1.]).unwrap(),
        Tolerance::DEFAULT,
    )
    .unwrap();
    app.viewports[0].plane.set(source);
    let mut custom_view = Viewport::new(ViewKind::Right);
    custom_view.set_view_title("My detail");
    app.viewports.push(custom_view);
    let mut custom_plan = Viewport::new(ViewKind::Top);
    custom_plan.set_cplane_view(WorldPlane::Top);
    assert_eq!(custom_plan.synchronization_role(), None);
    app.viewports.push(custom_plan);
    let cameras = app
        .viewports
        .iter()
        .map(Viewport::camera_snapshot)
        .collect::<Vec<_>>();
    let old_planes = app
        .viewports
        .iter()
        .map(Viewport::construction_plane)
        .collect::<Vec<_>>();
    enter(&mut app, "SynchronizeCPlanes 1 SetView=No");
    let synchronized_top = app.viewports[0].construction_plane();
    let synchronized_front = app.viewports[2].construction_plane();
    assert_eq!(synchronized_top.origin(), source.origin());
    assert_eq!(
        synchronized_top.x_axis().as_vector(),
        source.y_axis().as_vector()
    );
    assert_eq!(
        synchronized_top.y_axis().as_vector(),
        source.z_axis().as_vector()
    );
    assert_eq!(app.viewports[1].construction_plane(), source);
    assert_eq!(
        app.viewports[2].construction_plane().origin(),
        source.origin()
    );
    assert_eq!(
        app.viewports[2].construction_plane().x_axis().as_vector(),
        source.x_axis().as_vector()
    );
    assert_eq!(
        app.viewports[2].construction_plane().y_axis().as_vector(),
        source.z_axis().as_vector()
    );
    assert_eq!(app.viewports[3].construction_plane(), synchronized_top);
    assert_eq!(app.viewports[4].construction_plane(), old_planes[4]);
    assert_eq!(app.viewports[5].construction_plane(), old_planes[5]);
    assert_eq!(
        app.viewports
            .iter()
            .map(Viewport::camera_snapshot)
            .collect::<Vec<_>>(),
        cameras
    );
    app.active_viewport = 2;
    enter(&mut app, "CPlane Undo");
    assert_eq!(app.viewports[2].construction_plane(), old_planes[2]);
    enter(&mut app, "CPlane Redo");
    assert_eq!(app.viewports[2].construction_plane(), synchronized_front);
    assert_eq!(
        app.viewports[2].synchronization_role(),
        Some(WorldPlane::Front)
    );
}

#[test]
fn synchronize_cplanes_set_view_records_camera_and_plane_history() {
    let mut app = test_app();
    enter(&mut app, "Line");
    enter(&mut app, "w1,2,3");
    let pending = app.active_command;
    let source = viboceros_command::construction_plane::WorldPlane::Right
        .frame()
        .with_origin(point(3., 4., 5.));
    app.viewports[0].plane.set(source);
    let old_camera = app.viewports[2].camera_snapshot();
    let perspective_camera = app.viewports[1].camera_snapshot();
    let old_plane = app.viewports[2].construction_plane();
    enter(&mut app, "SynchronizeCPlanes SetView=Yes");
    let new_camera = app.viewports[2].camera_snapshot();
    let new_plane = app.viewports[2].construction_plane();
    assert_ne!(new_camera, old_camera);
    assert_eq!(app.viewports[1].camera_snapshot(), perspective_camera);
    assert_ne!(new_plane, old_plane);
    assert_eq!(app.viewports[0].view_label(), "Top");
    assert_eq!(app.viewports[1].view_label(), "Perspective");
    assert_eq!(app.viewports[2].view_label(), "Front");
    assert_eq!(app.viewports[3].view_label(), "Right");
    assert_eq!(app.active_command, pending);
    assert_eq!(app.document.undo_label(), None);
    app.active_viewport = 2;
    enter(&mut app, "UndoView");
    assert_eq!(app.viewports[2].camera_snapshot(), old_camera);
    assert_eq!(app.viewports[2].construction_plane(), new_plane);
    enter(&mut app, "CPlane Undo");
    assert_eq!(app.viewports[2].construction_plane(), old_plane);
    enter(&mut app, "CPlane Redo");
    enter(&mut app, "RedoView");
    assert_eq!(app.viewports[2].construction_plane(), new_plane);
    assert_eq!(app.viewports[2].camera_snapshot(), new_camera);
    enter(&mut app, "SynchronizeCPlanes Missing");
    assert!(app.command_log.back().unwrap().starts_with("Error:"));
    assert_eq!(app.viewports[2].construction_plane(), new_plane);
    enter(&mut app, "SynchronizeCPlanes Top SetView=No");
    assert!(app.command_log.back().unwrap().starts_with("Synchronized"));
}

#[test]
fn synchronize_cplanes_uses_native_world_preset_role_for_parallel_source() {
    use viboceros_command::construction_plane::WorldPlane;

    let mut view = Viewport::new(ViewKind::Top);
    for direction in WorldPlane::ALL {
        view.plane.set(direction.frame());
        assert_eq!(
            view.synchronization_plane_role(),
            Some((WorldPlane::Top, direction))
        );
    }
}

#[test]
fn copy_cplane_and_grid_settings_to_all_keep_their_domains_separate() {
    let mut app = test_app();
    enter(&mut app, "Line");
    enter(&mut app, "w1,2,3");
    let pending = app.active_command;
    let cameras = app
        .viewports
        .iter()
        .map(Viewport::camera_snapshot)
        .collect::<Vec<_>>();
    let original_planes = app
        .viewports
        .iter()
        .map(Viewport::construction_plane)
        .collect::<Vec<_>>();
    let source_plane = viboceros_command::construction_plane::WorldPlane::Right
        .frame()
        .with_origin(point(7., 8., 9.));
    app.viewports[2].plane.set(source_plane);
    let source_grid = GridSettings {
        minor_spacing: 2.5,
        snap_spacing: 0.25,
        major_interval: 0,
        line_count: 30,
        show_grid: false,
        show_axes: false,
        show_world_axes: true,
    };
    app.viewports[2].set_grid_settings(source_grid);
    let original_grid = app.viewports[0].grid_settings();

    enter(&mut app, "CopyCPlaneToAll Front");
    assert!(
        app.viewports
            .iter()
            .all(|view| view.construction_plane() == source_plane)
    );
    assert_eq!(app.viewports[0].grid_settings(), original_grid);
    assert_eq!(app.active_command, pending);
    assert_eq!(app.active_viewport, 0);
    assert_eq!(
        app.viewports
            .iter()
            .map(Viewport::camera_snapshot)
            .collect::<Vec<_>>(),
        cameras
    );
    enter(&mut app, "CPlane Undo");
    assert_eq!(app.viewports[0].construction_plane(), original_planes[0]);
    enter(&mut app, "CPlane Redo");
    assert_eq!(app.viewports[0].construction_plane(), source_plane);

    enter(&mut app, "CopyCPlaneSettingsToAll 3");
    assert!(
        app.viewports
            .iter()
            .all(|view| view.grid_settings() == source_grid)
    );
    app.active_viewport = 1;
    app.viewports[1].set_grid_settings(original_grid);
    enter(&mut app, "CopyCPlaneSettingsToAll");
    assert!(
        app.viewports
            .iter()
            .all(|view| view.grid_settings() == original_grid)
    );
    assert_eq!(app.document.undo_label(), None);
    enter(&mut app, "CopyCPlaneToAll Missing");
    assert!(app.command_log.back().unwrap().starts_with("Error:"));
    assert_eq!(app.active_command, pending);
}

#[test]
fn cplane_edits_are_view_local_and_model_undo_does_not_change_them() {
    let mut app = test_app();
    for input in [
        "Point 1,2,3",
        "SelAll",
        "CPlane World Front",
        "CPlane 4,5,6",
        "CPlane Elevation 2",
    ] {
        enter(&mut app, input);
    }
    let frame = app.viewports[0].construction_plane();
    assert_eq!(app.viewports[0].kind(), ViewKind::Top);
    assert_eq!(frame.origin(), point(4., -8., 5.));
    assert_eq!(frame.z_axis().as_vector().to_array(), [0., -1., 0.]);
    assert_eq!(app.document.selected_object_count(), 1);
    assert_eq!(
        app.viewports[1].construction_plane().origin(),
        point(0., 0., 0.)
    );
    enter(&mut app, "Undo");
    assert_eq!(app.document.objects().len(), 0);
    assert_eq!(app.viewports[0].construction_plane(), frame);
    enter(&mut app, "CPlane Undo");
    assert_eq!(
        app.viewports[0].construction_plane().origin(),
        point(4., -6., 5.)
    );
    enter(&mut app, "CPlane Redo");
    assert_eq!(app.viewports[0].construction_plane(), frame);
    enter(&mut app, "Redo");
    assert_eq!(app.document.objects().len(), 1);
}

#[test]
fn nested_three_point_plane_prompt_retains_model_points_and_returns_to_them() {
    let mut app = test_app();
    for input in ["Polyline", "w1,2,3", "w4,5,6"] {
        enter(&mut app, input);
    }
    let pending = app.active_command;
    let points = app.curve_points.clone();
    let last = app.last_point;
    let latched = app.drafting_plane;
    for input in [
        "CPlane 3Point",
        "w10,20,30",
        "w10,21,30",
        "Snap",
        "w10,20,31",
    ] {
        enter(&mut app, input);
    }
    assert!(app.plane_prompt.is_none());
    assert_eq!(app.active_command, pending);
    assert_eq!(app.curve_points, points);
    assert_eq!(app.last_point, last);
    assert_eq!(app.drafting_plane, latched);
    assert_eq!(app.viewports[0].kind(), ViewKind::Top);
    assert_eq!(
        app.viewports[0].construction_plane().origin(),
        point(10., 20., 30.)
    );
    enter(&mut app, "2,3");
    enter(&mut app, "");
    let Geometry::Polyline(polyline) = app.document.objects().next().unwrap().geometry() else {
        panic!("polyline")
    };
    assert_eq!(
        polyline.vertices(),
        &[point(1., 2., 3.), point(4., 5., 6.), point(10., 22., 33.)]
    );
    enter(&mut app, "Undo");
    assert_eq!(app.document.objects().len(), 0);
    assert!(!app.document.can_undo());
}

#[test]
fn invalid_cplane_input_and_cancellation_retain_the_model_prompt_and_plane() {
    let mut app = test_app();
    for input in ["Circle", "0", "CPlane 3Point", "0", "w1,0,0"] {
        enter(&mut app, input);
    }
    let frame = app.viewports[0].construction_plane();
    for bad in ["w2,0,0", "nan,0,0", "not a point"] {
        enter(&mut app, bad);
        assert_eq!(app.command_input, bad);
        assert_eq!(app.plane_prompt.as_ref().unwrap().points.len(), 2);
        assert_eq!(app.viewports[0].construction_plane(), frame);
    }
    app.cancel_plane_prompt();
    assert!(app.active_command.is_some());
    assert_eq!(app.last_point, Some(point(0., 0., 0.)));
    enter(&mut app, "w2,0,0");
    assert_eq!(app.document.objects().len(), 1);
    assert_eq!(app.viewports[0].construction_plane(), frame);
}

#[test]
fn interactive_plane_origin_height_through_and_rotation_share_the_validated_edit_path() {
    let mut app = test_app();
    for input in [
        "CPlane",
        "w2,3,4",
        "CPlane Elevation",
        "5",
        "CPlane Through",
        "w12,13,7",
        "CPlane Rotate",
        "w0,0,0",
        "w0,0,1",
        "90",
    ] {
        enter(&mut app, input);
    }
    assert!(app.plane_prompt.is_none(), "{:?}", app.command_log);
    let frame = app.viewports[0].construction_plane();
    assert!(frame.origin().distance_to(point(-3., 2., 7.)).unwrap() < 1e-12);
    assert_eq!(app.document.objects().len(), 0);
    assert!(!app.document.can_undo());
    enter(&mut app, "Circle");
    enter(&mut app, "0");
    enter(&mut app, "2,0");
    let Geometry::Circle(circle) = app.document.objects().next().unwrap().geometry() else {
        panic!("circle")
    };
    assert!(circle.center().distance_to(frame.origin()).unwrap() < 1e-12);
    assert!((circle.radius() - 2.).abs() < 1e-12);
}

#[test]
fn complete_cplane_command_replaces_a_plane_prompt_without_cancelling_model_input() {
    let mut app = test_app();
    for input in ["Line", "w1,2,3", "CPlane 3Point", "0", "CPlane World Right"] {
        enter(&mut app, input);
    }
    assert!(app.plane_prompt.is_none());
    assert!(app.active_command.is_some());
    enter(&mut app, "2,3");
    let Geometry::Line(line) = app.document.objects().next().unwrap().geometry() else {
        panic!("line")
    };
    assert_eq!(line.start(), point(1., 2., 3.));
    assert_eq!(line.end(), point(0., 2., 3.));
}

#[test]
fn cplane_picked_points_edit_the_starting_viewport_when_reference_picks_use_other_views() {
    let mut app = test_app();
    enter(&mut app, "CPlane 3Point");
    for (index, p) in [
        (1, point(1., 2., 3.)),
        (2, point(2., 2., 3.)),
        (3, point(1., 2., 4.)),
    ] {
        app.active_viewport = index;
        assert!(app.handle_viewport_action(ViewportOutput {
            picked_point: Some(p),
            ..Default::default()
        }));
    }
    assert!(app.plane_prompt.is_none());
    assert_eq!(
        app.viewports[0].construction_plane().origin(),
        point(1., 2., 3.)
    );
    assert_eq!(
        app.viewports[0]
            .construction_plane()
            .z_axis()
            .as_vector()
            .to_array(),
        [0., -1., 0.]
    );
    assert_eq!(
        app.viewports[3].construction_plane().origin(),
        point(0., 0., 0.)
    );
    assert_eq!(app.document.objects().len(), 0);
}

#[test]
fn front_view_circle_uses_the_front_plane() {
    let mut app = test_app();
    app.active_viewport = 2;
    for input in ["Circle", "0", "0,5"] {
        enter(&mut app, input);
    }
    let Geometry::Circle(circle) = app
        .document
        .objects()
        .next()
        .expect("front circle")
        .geometry()
    else {
        panic!("circle");
    };
    assert_eq!(circle.radius(), 5.0);
    assert_eq!(
        circle.normal().unwrap().as_vector().to_array(),
        [0.0, -1.0, 0.0]
    );
}

#[test]
fn right_view_rectangle_uses_yz_width_and_height() {
    let mut app = test_app();
    app.active_viewport = 3;
    for input in ["Rectangle", "0", "4,3"] {
        enter(&mut app, input);
    }
    let Geometry::Polyline(rectangle) = app
        .document
        .objects()
        .next()
        .expect("right rectangle")
        .geometry()
    else {
        panic!("rectangle");
    };
    assert_eq!(
        rectangle.vertices(),
        &[
            point(0.0, 0.0, 0.0),
            point(0.0, 4.0, 0.0),
            point(0.0, 4.0, 3.0),
            point(0.0, 0.0, 3.0),
            point(0.0, 0.0, 0.0)
        ]
    );
}

#[test]
fn first_accepted_pick_latches_the_plane_until_completion() {
    let mut app = test_app();
    enter(&mut app, "Circle");
    app.active_viewport = 2;
    enter(&mut app, "0");
    app.active_viewport = 0;
    enter(&mut app, "w0,0,5");
    let Geometry::Circle(circle) = app.document.objects().next().unwrap().geometry() else {
        panic!("circle")
    };
    assert_eq!(
        circle.normal().unwrap().as_vector().to_array(),
        [0.0, -1.0, 0.0]
    );
    assert_eq!(circle.radius(), 5.0);
    assert!(app.drafting_plane.is_none());
    enter(&mut app, "Undo");
    assert_eq!(app.document.objects().len(), 0);
}

#[test]
fn cancellation_and_replacement_do_not_leak_a_previous_plane() {
    let mut app = test_app();
    app.active_viewport = 2;
    enter(&mut app, "Circle");
    enter(&mut app, "0");
    app.active_viewport = 0;
    enter(&mut app, "Circle 0,0,0 5");
    let Geometry::Circle(circle) = app.document.objects().next().unwrap().geometry() else {
        panic!("circle")
    };
    assert_eq!(
        circle.normal().unwrap().as_vector().to_array(),
        [0.0, 0.0, 1.0]
    );
    enter(&mut app, "Rectangle");
    enter(&mut app, "0");
    app.cancel_interactive_command(true);
    assert!(app.drafting_plane.is_none());
}

#[test]
fn zero_radius_and_zero_width_picks_remain_correctable() {
    let mut app = test_app();
    app.active_viewport = 2;
    for input in ["Circle", "0", "w0,0,0"] {
        enter(&mut app, input);
    }
    assert_eq!(app.last_point, Some(point(0.0, 0.0, 0.0)));
    assert_eq!(app.command_input, "w0,0,0");
    enter(&mut app, "w0,0,5");
    assert_eq!(app.document.objects().len(), 1);
    for input in ["Rectangle", "0", "0,5"] {
        enter(&mut app, input);
    }
    assert!(app.active_command.is_some());
    enter(&mut app, "4,5");
    assert_eq!(app.document.objects().len(), 2);
}

#[test]
fn boxes_use_plane_height_and_one_undo_transaction_in_every_view() {
    for viewport in 0..4 {
        for name in ["Box", "MeshBox XCount=2 YCount=3 ZCount=2"] {
            let mut app = test_app();
            app.active_viewport = viewport;
            for input in [name, "1,2,3", "5,7,3", "5,7,-3"] {
                enter(&mut app, input);
            }
            assert!(
                app.active_command.is_none(),
                "{name}: {:?}",
                app.command_log
            );
            let object = app.document.objects().next().unwrap();
            match object.geometry() {
                Geometry::Brep(brep) => {
                    assert!(brep.is_solid());
                    assert!(
                        (brep.signed_volume(app.document.tolerance()).unwrap() - 120.0).abs()
                            < 1e-9
                    );
                }
                Geometry::Mesh(mesh) => assert!(mesh.topology().is_solid()),
                _ => panic!("box"),
            }
            assert!(app.drafting_plane.is_none());
            enter(&mut app, "Undo");
            assert_eq!(app.document.objects().len(), 0);
            enter(&mut app, "Redo");
            assert_eq!(app.document.objects().len(), 1);
        }
    }
}

#[test]
fn front_view_transform_prompts_use_their_plane_and_undo_atomically() {
    for (inputs, expected) in [
        (vec!["Rotate", "0", "0,1", "1,0"], point(3.0, 2.0, -1.0)),
        (vec!["Mirror", "0", "0,1"], point(-1.0, 2.0, 3.0)),
        (vec!["Scale2D", "0", "0,1", "0,2"], point(2.0, 2.0, 6.0)),
        (vec!["Shear", "0", "0,1", "1,1"], point(4.0, 2.0, 3.0)),
        (
            vec!["ProjectToCPlane DeleteInput=Yes"],
            point(1.0, 0.0, 3.0),
        ),
        (
            vec!["SetPt 5,7,-2 XSet=No YSet=Yes ZSet=No Alignment=CPlane"],
            point(1.0, 2.0, -2.0),
        ),
    ] {
        let mut app = test_app();
        app.active_viewport = 2;
        enter(&mut app, "Point 1,2,3");
        enter(&mut app, "SelAll");
        let id = app.document.objects().next().unwrap().id();
        for input in inputs {
            enter(&mut app, input);
        }
        assert!(app.active_command.is_none(), "{:?}", app.command_log);
        let Geometry::Point(actual) = app.document.object(id).unwrap().geometry() else {
            panic!("point")
        };
        assert!(
            actual.distance_to(expected).unwrap() < 1e-12,
            "{:?}",
            app.command_log
        );
        enter(&mut app, "Undo");
        assert_eq!(
            app.document.object(id).unwrap().geometry(),
            &Geometry::Point(point(1.0, 2.0, 3.0))
        );
        enter(&mut app, "Redo");
        assert!(app.drafting_plane.is_none());
    }
}

#[test]
fn normal_only_angle_and_mirror_references_remain_correctable_in_front_view() {
    for name in ["Rotate", "Mirror", "Shear"] {
        let mut app = test_app();
        app.active_viewport = 2;
        for input in ["Point 1,2,3", "SelAll", name, "0", "w0,5,0"] {
            enter(&mut app, input);
        }
        assert!(app.active_command.is_some(), "{name}");
        assert_eq!(app.command_input, "w0,5,0");
        assert_eq!(app.last_point, Some(point(0.0, 0.0, 0.0)));
        enter(&mut app, "0,1");
        if name != "Mirror" {
            enter(&mut app, "1,1");
        }
        assert!(app.active_command.is_none(), "{:?}", app.command_log);
    }
}

#[test]
fn scale2d_uses_full_reference_distances_and_the_finishing_viewport() {
    let mut app = test_app();
    app.active_viewport = 2;
    for input in ["Point 1,2,3", "SelAll", "Scale2D", "0", "0,1"] {
        enter(&mut app, input);
    }
    app.active_viewport = 0;
    enter(&mut app, "w0,0,2");
    assert!(app.active_command.is_none(), "{:?}", app.command_log);
    assert_eq!(
        app.document.objects().next().unwrap().geometry(),
        &Geometry::Point(point(2.0, 4.0, 3.0))
    );
    assert!(app.drafting_plane.is_none());
}

#[test]
fn shear_accepts_a_normal_target_with_a_tilted_reference() {
    let mut app = test_app();
    for input in ["Point 3,4,7", "SelAll", "Shear", "0", "3,4,5", "0,0,5"] {
        enter(&mut app, input);
    }
    assert!(app.active_command.is_none(), "{:?}", app.command_log);
    let Geometry::Point(p) = app.document.objects().next().unwrap().geometry() else {
        panic!("point")
    };
    assert!(
        p.distance_to(point(
            3.0 - 4.0 * 2.0_f64.sqrt(),
            4.0 + 3.0 * 2.0_f64.sqrt(),
            7.0
        ))
        .unwrap()
            < 1e-12
    );
}
