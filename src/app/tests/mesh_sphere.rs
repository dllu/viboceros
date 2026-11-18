use super::{
    circle_preview::{screen_point, viewport_frame},
    curve_fillet::enter,
    *,
};

fn last_mesh(app: &VibocerosApp) -> &TriangleMesh {
    let Geometry::Mesh(mesh) = app.document.objects().last().unwrap().geometry() else {
        panic!("expected a polygon mesh");
    };
    mesh
}

#[test]
fn mesh_sphere_style_getters_preview_and_keep_only_accepted_settings() {
    let mut app = test_app();
    app.viewports[0] = Viewport::new(ViewKind::Top);
    app.maximized_viewport = Some(0);
    enter(&mut app, "MeshSphere Style");
    assert!(
        app.command_log
            .back()
            .unwrap()
            .contains("Style <UV> ( UV Triangle Quad )")
    );
    assert!(!app.accept_drafting_point(point(0., 0., 0.)));
    for step in ["Quad", "Subdivisions", "1", "0,0"] {
        enter(&mut app, step);
    }
    let ctx = egui::Context::default();
    viewport_frame(&mut app, &ctx, vec![]);
    let pointer = screen_point(&app, point(3., 2., 0.));
    let wires = viewport_frame(&mut app, &ctx, vec![egui::Event::PointerMoved(pointer)]);
    assert_eq!(wires.len(), 48, "{:?}", app.command_log);
    assert!(app.document.objects().next().is_none());
    enter(&mut app, "7");
    assert_eq!(last_mesh(&app).face_count(), 24);
    let accepted = app.commands.mesh_sphere_preferences();
    enter(&mut app, "Undo");
    for step in [
        "MeshSphere",
        "Style=Triangle",
        "Subdivisions=2",
        "Style",
        "Cancel",
    ] {
        enter(&mut app, step);
    }
    assert!(app.active_command.is_none());
    assert_eq!(app.commands.mesh_sphere_preferences(), accepted);
    assert_eq!(app.document.redo_label(), Some("MeshSphere"));
    for step in ["MeshSphere", "0,0", ""] {
        enter(&mut app, step);
    }
    assert_eq!(last_mesh(&app).face_count(), 24);
    for &vertex in last_mesh(&app).vertices() {
        assert!((vertex.distance_to(point(0., 0., 0.)).unwrap() - 1.).abs() < 1e-10);
    }
}

#[test]
fn mesh_sphere_fit_previews_selected_points_with_the_staged_topology() {
    let mut app = test_app();
    let points = [
        point(3., 0., 0.),
        point(0., 3., 0.),
        point(-3., 0., 0.),
        point(0., 0., 3.),
    ];
    for p in points {
        app.document.add_geometry(Geometry::Point(p)).unwrap();
    }
    for step in ["MeshSphere Style=Triangle", "Subdivisions=1", "FitPoints"] {
        enter(&mut app, step);
    }
    assert_eq!(
        app.object_prompt.as_ref().unwrap().description.command,
        "MeshSphere FitPoints"
    );
    assert!(app.mesh_sphere_fit_preview().is_none());
    enter(&mut app, "SelAll");
    let (topology, definition) = app.mesh_sphere_fit_preview().unwrap();
    let preview = topology.mesh(definition, app.document.tolerance()).unwrap();
    assert_eq!(preview.face_count(), 80);
    assert_eq!(app.document.objects().len(), points.len());
    enter(&mut app, "");
    assert!(app.object_prompt.is_none());
    assert_eq!(last_mesh(&app), &preview);
    assert_eq!(app.document.selected_object_count(), 0);
    assert_eq!(app.document.undo_label(), Some("MeshSphere"));
    enter(&mut app, "Undo");
    assert_eq!(app.document.objects().len(), points.len());
    assert_eq!(app.document.selected_object_count(), 0);
}

#[test]
fn mesh_sphere_reference_prompts_keep_the_mesh_style_through_preview_and_commit() {
    let mut app = test_app();
    app.viewports[0] = Viewport::new(ViewKind::Top);
    app.maximized_viewport = Some(0);
    let ctx = egui::Context::default();
    viewport_frame(&mut app, &ctx, vec![]);
    for step in [
        "MeshSphere Style=Triangle Subdivisions=1 Tangent",
        "Point",
        "3,0",
        "Point",
        "0,3",
        "Point",
    ] {
        enter(&mut app, step);
    }
    let pointer = screen_point(&app, point(-3., 0., 0.));
    let wires = viewport_frame(&mut app, &ctx, vec![egui::Event::PointerMoved(pointer)]);
    assert!(wires.len() > 80);
    assert!(app.document.objects().next().is_none());
    enter(&mut app, "-3,0");
    assert_eq!(last_mesh(&app).face_count(), 80);
    enter(&mut app, "Line 0,0,0 0,10,10");
    let guide = app.document.objects().last().unwrap().id();
    let source = app.document.object(guide).unwrap().geometry().clone();
    for step in [
        "MeshSphere Style=Quad Subdivisions=1 AroundCurve",
        &format!("Curve={guide}"),
        "w0,3,3",
        "w6,3,8",
    ] {
        enter(&mut app, step);
    }
    assert_eq!(last_mesh(&app).face_count(), 24);
    assert_eq!(app.document.object(guide).unwrap().geometry(), &source);
    assert!(!app.document.is_selected(guide));
    assert_eq!(app.document.undo_label(), Some("MeshSphere"));
}
