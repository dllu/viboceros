use super::*;
use super::{
    circle_preview::{screen_point, viewport_click, viewport_frame},
    curve_fillet::enter,
};
use viboceros_command::torus_input::Kind;

fn mesh(app: &VibocerosApp) -> &viboceros_geometry::TriangleMesh {
    let Geometry::Mesh(mesh) = app.document.objects().last().unwrap().geometry() else {
        panic!("expected a torus mesh");
    };
    mesh
}

#[test]
fn mesh_torus_face_prompts_keep_values_and_preview_the_pending_mesh() {
    let mut app = test_app();
    app.viewports[0] = Viewport::new(ViewKind::Top);
    app.maximized_viewport = Some(0);
    for step in [
        "MeshTorus VerticalFaces",
        "4",
        "AroundFaces",
        "6",
        "0,0",
        "4",
    ] {
        enter(&mut app, step);
    }
    let session = app.torus_session.as_ref().unwrap();
    assert_eq!(session.input.kind, Kind::Mesh);
    assert!(session.input.preferences.tube_diameter);
    let cursor = point(5., 0., 0.);
    app.update_torus_hover(cursor);
    let (definition, topology) = app.torus_session.as_ref().unwrap().mesh_preview().unwrap();
    let preview = definition.mesh(topology, app.document.tolerance()).unwrap();
    let context = egui::Context::default();
    let wires = viewport_frame(&mut app, &context, vec![]);
    assert_eq!(wires.len(), 48);
    assert!(app.document.undo_label().is_none());
    assert!(app.accept_drafting_point(cursor));
    assert_eq!(mesh(&app), &preview);
    assert_eq!(app.document.undo_label(), Some("MeshTorus"));
    let accepted = app.commands.mesh_torus_preferences();
    enter(&mut app, "Undo");
    for step in ["MeshTorus", "VerticalFaces=7", "AroundFaces=9", "Cancel"] {
        enter(&mut app, step);
    }
    assert_eq!(app.commands.mesh_torus_preferences(), accepted);
    assert_eq!(app.document.redo_label(), Some("MeshTorus"));
    for step in ["MeshTorus", "0,0", "Enter", "Enter"] {
        enter(&mut app, step);
    }
    assert_eq!(mesh(&app), &preview);
}

#[test]
fn mesh_torus_projects_endpoint_snaps_while_sizing_the_base() {
    let mut app = test_app();
    app.viewports[0] = Viewport::new(ViewKind::Top);
    app.maximized_viewport = Some(0);
    app.osnap = false;
    enter(&mut app, "Line 6,0,8 9,0,8");
    let context = egui::Context::default();
    for (setting, radius) in [("Yes", 6.), ("No", 61_f64.sqrt())] {
        for step in [
            "MeshTorus VerticalFaces=4 AroundFaces=6",
            "w0,0,3",
            &format!("ProjectOsnap={setting}"),
            "_End",
        ] {
            enter(&mut app, step);
        }
        viewport_frame(&mut app, &context, vec![]);
        let pointer = screen_point(&app, point(6., 0., 8.));
        viewport_click(&mut app, &context, pointer);
        let circle = app.torus_session.as_ref().unwrap().input.circle.unwrap();
        assert!((circle.radius - radius).abs() < 1e-10);
        enter(&mut app, "1");
        assert_eq!(mesh(&app).face_count(), 24);
        enter(&mut app, "Undo");
    }
}

#[test]
fn mesh_torus_tangent_around_and_fit_resume_the_tube_prompt() {
    let mut app = test_app();
    app.viewports[0] = Viewport::new(ViewKind::Top);
    for step in [
        "MeshTorus VerticalFaces=4 AroundFaces=6 Tangent",
        "Point",
        "4,0",
        "Point",
        "0,4",
        "Point",
        "-4,0",
    ] {
        enter(&mut app, step);
    }
    let session = app.torus_session.as_ref().unwrap();
    assert_eq!(session.input.circle.unwrap().radius, 4.);
    assert_eq!(session.mesh_preview().unwrap().1.around_faces, 6);
    enter(&mut app, "2");
    assert_eq!(mesh(&app).face_count(), 24);
    enter(&mut app, "Line 0,0,0 0,10,10");
    let guide = app.document.objects().last().unwrap().id();
    for step in [
        "MeshTorus AroundCurve",
        &format!("Curve={guide}"),
        "w0,3,3",
        "w6,3,8",
        "2",
    ] {
        enter(&mut app, step);
    }
    assert_eq!(mesh(&app).face_count(), 24);
    assert!(!app.document.is_selected(guide));
    for point in [point(3., 0., 2.), point(0., 3., 2.), point(-3., 0., 2.)] {
        app.document.add_geometry(Geometry::Point(point)).unwrap();
    }
    let base_radius = app.commands.mesh_torus_preferences().radius;
    enter(&mut app, "MeshTorus FitPoints");
    assert_eq!(
        app.object_prompt.as_ref().unwrap().description.command,
        "MeshTorus FitPoints"
    );
    enter(&mut app, "SelAll");
    assert!(app.mesh_torus_fit_preview().is_some());
    enter(&mut app, "");
    assert!(app.object_prompt.is_none());
    assert!(app.torus_session.as_ref().unwrap().mesh_preview().is_some());
    enter(&mut app, "2");
    assert_eq!(mesh(&app).face_count(), 24);
    assert_eq!(app.document.selected_object_count(), 0);
    assert_eq!(app.commands.mesh_torus_preferences().radius, base_radius);
}

#[test]
fn mesh_torus_fixed_inner_preview_keeps_the_first_radius_as_the_inner_rim() {
    for (mode, radius, outer) in [("Diameter", 2., 8.), ("Radius", 4., 12.)] {
        let mut app = test_app();
        app.viewports[0] = Viewport::new(ViewKind::Top);
        for step in [
            "MeshTorus VerticalFaces=4 AroundFaces=6",
            "0,0,0",
            "4",
            mode,
            "FixInnerDimension=Yes",
        ] {
            enter(&mut app, step);
        }
        let picked = point(8., 0., 9.);
        app.update_torus_hover(picked);
        let (definition, preferences) = app.torus_session.as_ref().unwrap().mesh_preview().unwrap();
        assert_eq!(definition.major_radius(), 4. + radius);
        assert_eq!(definition.tube_radius, radius);
        let preview = definition
            .mesh(preferences, app.document.tolerance())
            .unwrap();
        assert_eq!(preview.vertices()[0], point(outer, 0., 0.));
        assert!(
            preview.vertices()[12]
                .distance_to(point(4., 0., 0.))
                .unwrap()
                < 1e-12
        );
        assert!(app.accept_drafting_point(picked));
        assert_eq!(mesh(&app), &preview);
    }
}
