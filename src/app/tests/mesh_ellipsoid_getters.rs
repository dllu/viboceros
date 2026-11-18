use super::{
    circle_preview::{screen_point, viewport_click, viewport_frame},
    curve_fillet::enter,
    *,
};
use viboceros_geometry::{MeshEllipsoidOptions, MeshFace, TriangleMesh};

fn last_mesh(app: &VibocerosApp) -> &TriangleMesh {
    app.document
        .objects()
        .filter_map(|object| match object.geometry() {
            Geometry::Mesh(mesh) => Some(mesh),
            _ => None,
        })
        .last()
        .expect("expected a mesh ellipsoid")
}

#[test]
fn mesh_ellipsoid_settings_prompt_matches_rhino_memory_and_odd_caps() {
    let mut app = test_app();
    let defaults = app.commands.mesh_ellipsoid_preferences();
    enter(&mut app, "MeshEllipsoid");
    enter(&mut app, "VerticalFaces");
    assert!(app.ellipse_session.as_ref().unwrap().waiting_for_topology());
    assert!(!app.viewport_drafting_input(false, false).active);
    assert!(!app.accept_drafting_point(point(3., 0., 0.)));
    enter(&mut app, "1");
    assert!(app.ellipse_session.as_ref().unwrap().waiting_for_topology());
    enter(&mut app, "4");
    enter(&mut app, "AroundFaces 6");
    enter(&mut app, "CapFaceStyle");
    app.cancel_current_prompt_or_selection();
    assert_eq!(app.commands.mesh_ellipsoid_preferences(), defaults);
    assert!(app.document.objects().next().is_none());

    for (steps, count, caps) in [
        (
            &[
                "MeshEllipsoid",
                "VerticalFaces",
                "5",
                "AroundFaces",
                "8",
                "CapFaceStyle",
            ][..],
            32,
            true,
        ),
        (&["MeshEllipsoid", "AroundFaces=5"][..], 25, false),
        (&["MeshEllipsoid", "AroundFaces=6"][..], 24, true),
    ] {
        for step in steps {
            enter(&mut app, step);
        }
        assert_eq!(
            app.command_log.back().unwrap().contains("CapFaceStyle"),
            caps
        );
        for step in ["0", "w5,0,0", "w0,3,0", "2"] {
            enter(&mut app, step);
        }
        assert_eq!(app.active_command, None);
        let mesh = last_mesh(&app);
        assert_eq!(mesh.face_count(), count);
        assert_eq!(matches!(mesh.faces()[0], MeshFace::Quad(_)), caps);
    }
}

#[test]
fn mesh_ellipsoid_third_axis_draws_the_committed_wire_topology() {
    let mut app = test_app();
    app.viewports[0] = Viewport::new(ViewKind::Front);
    app.maximized_viewport = Some(0);
    app.osnap = false;
    for step in [
        "MeshEllipsoid VerticalFaces=4 AroundFaces=6 CapFaceStyle=Quad Diameter",
        "w-5,0,0",
        "w5,0,0",
        "w0,3,0",
    ] {
        enter(&mut app, step);
    }
    let context = egui::Context::default();
    viewport_frame(&mut app, &context, vec![]);
    let first = screen_point(&app, point(3., 0., 2.));
    let initial = viewport_frame(&mut app, &context, vec![egui::Event::PointerMoved(first)]);
    let third = screen_point(&app, point(8., 0., -4.));
    let preview = viewport_frame(&mut app, &context, vec![egui::Event::PointerMoved(third)]);
    assert_ne!(preview, initial);
    assert!(!app.document.can_undo());
    viewport_click(&mut app, &context, third);
    let mesh = last_mesh(&app);
    let mut edges = std::collections::BTreeSet::new();
    for face in mesh.faces() {
        let indices = face.indices();
        for (&a, &b) in indices.iter().zip(indices.iter().cycle().skip(1)) {
            edges.insert((a.min(b), a.max(b)));
        }
    }
    assert_eq!(preview.len(), edges.len());
    for (a, b) in edges {
        let pa = screen_point(&app, mesh.vertices()[a as usize]);
        let pb = screen_point(&app, mesh.vertices()[b as usize]);
        assert!(preview.iter().any(|[x, y]| {
            (x.distance(pa) < 1e-3 && y.distance(pb) < 1e-3)
                || (y.distance(pa) < 1e-3 && x.distance(pb) < 1e-3)
        }));
    }
    let expected = TriangleMesh::try_ellipsoid_grid(
        viboceros_command::construction_plane::WorldPlane::Top.frame(),
        [5., 3., 4.],
        MeshEllipsoidOptions {
            vertical_count: 4,
            around_count: 6,
            cap_style: MeshCapFaceStyle::Quadrilaterals,
        },
        app.document.tolerance(),
    )
    .unwrap();
    for (&actual, &vertex) in mesh.vertices().iter().zip(expected.vertices()) {
        assert!(actual.distance_to(vertex).unwrap() < 1e-5);
    }
    assert_eq!(app.document.undo_label(), Some("MeshEllipsoid"));
}

#[test]
fn mesh_ellipsoid_focal_markers_and_curve_guide_use_one_edit_each() {
    let mut app = test_app();
    for step in [
        "MeshEllipsoid FromFoci",
        "w-4,0,0",
        "w4,0,0",
        "MarkFoci=Yes",
        "w0,3,0",
    ] {
        enter(&mut app, step);
    }
    assert!(app.commands.mesh_ellipsoid_preferences().mark_foci);
    assert!(!app.commands.ellipsoid_mark_foci());
    assert_eq!(app.document.objects().count(), 3);
    enter(&mut app, "Undo");
    assert_eq!(app.document.objects().count(), 0);
    enter(&mut app, "Line 0,0,0 0,10,10");
    let guide = app.document.objects().next().unwrap().clone();
    enter(&mut app, "MeshEllipsoid AroundCurve");
    enter(&mut app, &format!("Curve={}", guide.id()));
    for step in [
        "w0,3,3",
        "w5,3,3",
        "w0,5.121320343559643,0.8786796564403576",
        "2",
    ] {
        enter(&mut app, step);
    }
    assert_eq!(app.document.objects().count(), 2);
    assert!(
        last_mesh(&app).vertices()[0]
            .distance_to(point(-5., 3., 3.))
            .unwrap()
            < 1e-10
    );
    assert_eq!(app.document.object(guide.id()), Some(&guide));
    assert!(!app.document.is_selected(guide.id()));
    enter(&mut app, "Undo");
    assert_eq!(app.document.objects().count(), 1);
    assert!(!app.document.is_selected(guide.id()));
    enter(&mut app, "Redo");
    assert_eq!(app.document.objects().count(), 2);
    assert!(!app.document.is_selected(guide.id()));
}

#[test]
fn mesh_ellipsoid_dense_preview_keeps_full_resolution_at_commit() {
    let mut app = test_app();
    app.viewports[0] = Viewport::new(ViewKind::Front);
    app.maximized_viewport = Some(0);
    for step in [
        "MeshEllipsoid VerticalFaces=80 AroundFaces=64",
        "0",
        "w5,0,0",
        "w0,3,0",
    ] {
        enter(&mut app, step);
    }
    let context = egui::Context::default();
    viewport_frame(&mut app, &context, vec![]);
    let cursor = screen_point(&app, point(0., 0., 4.));
    let preview = viewport_frame(&mut app, &context, vec![egui::Event::PointerMoved(cursor)]);
    assert_eq!(preview.len(), 192);
    assert!(!app.document.can_undo());
    viewport_click(&mut app, &context, cursor);
    assert_eq!(last_mesh(&app).face_count(), 5120);
}
