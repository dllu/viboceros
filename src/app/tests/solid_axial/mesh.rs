use super::*;
use viboceros_geometry::{MeshFace, TriangleMesh};

fn mesh(app: &VibocerosApp) -> &TriangleMesh {
    let Geometry::Mesh(mesh) = app.document.objects().last().unwrap().geometry() else {
        panic!("expected a mesh");
    };
    mesh
}

#[test]
fn solid_axial_mesh_previews_polygon_edges_before_a_viewport_height_pick() {
    let mut app = test_app();
    app.viewports[0] = Viewport::new(ViewKind::Top);
    app.maximized_viewport = Some(0);
    let ctx = egui::Context::default();
    for action in [
        "MeshCylinder VerticalFaces=2 AroundFaces=6 Solid=No",
        "w0,0,0",
        "3",
    ] {
        enter(&mut app, action);
    }
    app.viewports[0] = Viewport::new(ViewKind::Front);
    viewport_frame(&mut app, &ctx, vec![]);
    viewport_frame(&mut app, &ctx, vec![]);
    let end = point(0., 0., -5.);
    let pointer = screen_point(&app, end);
    viewport_frame(&mut app, &ctx, vec![egui::Event::PointerMoved(pointer)]);
    let pointer = screen_point(&app, end);
    let wires = viewport_frame(&mut app, &ctx, vec![egui::Event::PointerMoved(pointer)]);
    assert_eq!(wires.len(), 30);
    assert!(app.document.undo_label().is_none());
    viewport_click(&mut app, &ctx, pointer);

    let shape = mesh(&app).clone();
    assert_eq!(shape.face_count(), 12);
    assert_eq!(shape.vertices()[0], point(3., 0., 0.));
    assert!((shape.bounds().min().z() + 5.).abs() < 1e-4);
    assert_eq!(app.document.undo_label(), Some("MeshCylinder"));
    enter(&mut app, "Undo");
    assert!(app.document.objects().next().is_none());
    enter(&mut app, "Redo");
    assert_eq!(mesh(&app), &shape);
}

#[test]
fn solid_axial_mesh_cap_preferences_survive_odd_face_counts() {
    let mut app = test_app();
    enter(&mut app, "MeshCone");
    enter(&mut app, "VerticalFaces");
    assert!(
        app.command_log
            .back()
            .unwrap()
            .contains("VerticalFaces <10>")
    );
    for action in ["2", "AroundFaces", "6", "CapFaceStyle", "AroundFaces=7"] {
        enter(&mut app, action);
    }
    assert!(!app.command_log.back().unwrap().contains("CapFaceStyle"));
    for action in ["w0,0,0", "3", "4"] {
        enter(&mut app, action);
    }
    assert_eq!(mesh(&app).face_count(), 21);
    assert!(
        mesh(&app).faces()[14..]
            .iter()
            .all(|face| face.is_triangle())
    );
    enter(&mut app, "Undo");
    enter(&mut app, "MeshCone AroundFaces=6");
    assert!(
        app.command_log
            .back()
            .unwrap()
            .contains("CapFaceStyle=Quad")
    );
    for action in ["w0,0,0", "", ""] {
        enter(&mut app, action);
    }
    assert_eq!(mesh(&app).face_count(), 15);
    assert!(mesh(&app).faces()[12..].iter().all(|face| face.is_quad()));
}

#[test]
fn solid_axial_mesh_cylinder_remembers_the_picked_span_and_resets_both_sides() {
    let mut app = test_app();
    for action in ["MeshCylinder", "w0,0,0", "3", "BothSides", "w4,7,-5"] {
        enter(&mut app, action);
    }
    assert_eq!(mesh(&app).vertices()[0], point(3., 0., 5.));
    assert_eq!(mesh(&app).bounds().min().z(), -5.);
    enter(&mut app, "Undo");
    for action in ["MeshCylinder", "w0,0,0", ""] {
        enter(&mut app, action);
    }
    let input = app.solid_axial_preview_input().unwrap();
    assert!(!input.preferences.both_sides);
    assert_eq!(input.preferences.height, 10.);
    enter(&mut app, "");
    assert_eq!(mesh(&app).bounds().min().z(), 0.);
    assert_eq!(mesh(&app).bounds().max().z(), 10.);
}

#[test]
fn solid_axial_mesh_reference_modes_preserve_guide_geometry_and_fit_sources() {
    let mut app = test_app();
    enter(&mut app, "Line 0,0,0 0,10,10");
    let guide = app.document.objects().last().unwrap().id();
    let source = app.document.object(guide).unwrap().geometry().clone();
    for action in [
        "MeshTruncatedCone DirectionConstraint=AroundCurve",
        &format!("Curve={guide}"),
        "VerticalFaces",
        "3",
        "AroundFaces=8",
        "CapFaceStyle=Quad",
        "w0,3,3",
        "3",
        "5",
        "Radius=0",
    ] {
        enter(&mut app, action);
    }
    let collapsed = mesh(&app);
    assert_eq!(collapsed.vertices().len(), 41);
    assert_eq!(collapsed.faces()[16], MeshFace::Quad([16, 17, 25, 24]));
    assert_eq!(collapsed.vertices()[24], collapsed.vertices()[31]);
    assert_eq!(app.document.object(guide).unwrap().geometry(), &source);
    assert!(!app.document.is_selected(guide));
    enter(&mut app, "Undo");
    assert!(!app.document.is_selected(guide));

    for action in [
        "MeshCone Tangent",
        "Point",
        "4,0",
        "Point",
        "0,4",
        "Point",
        "-4,0",
        "5",
    ] {
        enter(&mut app, action);
    }
    assert_eq!(mesh(&app).vertices()[0], point(0., 0., 5.));
    for p in [point(4., 0., 0.), point(0., 4., 0.), point(-4., 0., 0.)] {
        app.document.add_geometry(Geometry::Point(p)).unwrap();
    }
    enter(&mut app, "MeshCylinder FitPoints");
    enter(&mut app, "SelAll");
    assert!(app.solid_axial_fit_preview().is_some());
    enter(&mut app, "");
    enter(&mut app, "w0,0,5");
    assert_eq!(mesh(&app).bounds().max().z(), 5.);
    assert_eq!(app.document.selected_object_count(), 0);
    assert_eq!(app.document.undo_label(), Some("MeshCylinder"));
    enter(&mut app, "Undo");
    assert_eq!(app.document.selected_object_count(), 0);
}
