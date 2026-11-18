use super::{
    circle_preview::{screen_point, viewport_click, viewport_frame},
    curve_fillet::enter,
    *,
};

#[test]
fn pipe_previews_radii_and_station_picks_before_one_edit() {
    let mut app = test_app();
    app.viewports[0] = Viewport::new(ViewKind::Top);
    app.maximized_viewport = Some(0);
    app.grid_snap = false;
    app.osnap = false;
    enter(&mut app, "Line w0,4,0 w0,16,12");
    let rail = app.document.objects().next().unwrap().id();
    enter(&mut app, &format!("Pipe Curve={rail} Diameter"));
    let ctx = egui::Context::default();
    viewport_frame(&mut app, &ctx, vec![]);
    viewport_frame(&mut app, &ctx, vec![]);
    let a = screen_point(&app, point(1., 4., 0.));
    let b = screen_point(&app, point(2., 4., 0.));
    let small = viewport_frame(&mut app, &ctx, vec![egui::Event::PointerMoved(a)]);
    let large = viewport_frame(&mut app, &ctx, vec![egui::Event::PointerMoved(b)]);
    assert!(!small.is_empty());
    assert_ne!(small, large);
    viewport_click(&mut app, &ctx, b);
    enter(&mut app, "8");
    assert_eq!(app.document.objects().len(), 1);

    app.viewports[0] = Viewport::new(ViewKind::Front);
    viewport_frame(&mut app, &ctx, vec![]);
    viewport_frame(&mut app, &ctx, vec![]);
    let station = screen_point(&app, point(0., 10., 6.));
    viewport_frame(&mut app, &ctx, vec![egui::Event::PointerMoved(station)]);
    viewport_click(&mut app, &ctx, station);
    let plane = app.pipe_session.as_ref().unwrap().radius_plane().unwrap();
    assert!(plane.origin().distance_to(point(0., 10., 6.)).unwrap() < 1e-5);
    let before = screen_point(&app, point(2.5, 10., 6.));
    let after = screen_point(&app, point(3., 10., 6.));
    let narrow = viewport_frame(&mut app, &ctx, vec![egui::Event::PointerMoved(before)]);
    let wide = viewport_frame(&mut app, &ctx, vec![egui::Event::PointerMoved(after)]);
    assert_ne!(narrow, wide);
    enter(&mut app, "5");
    assert_eq!(app.document.undo_label(), Some("Line"));
    assert_eq!(app.document.objects().len(), 1);
    enter(&mut app, "");
    let Geometry::Brep(pipe) = app.document.objects().last().unwrap().geometry() else {
        panic!("expected a capped pipe");
    };
    assert!(pipe.is_solid());
    assert_eq!(pipe.faces().len(), 3);
    assert_eq!(app.document.undo().unwrap().as_deref(), Some("Pipe"));
    assert_eq!(app.document.objects().next().unwrap().id(), rail);
    assert!(!app.document.is_selected(rail));
    assert_eq!(app.document.redo().unwrap().as_deref(), Some("Pipe"));
    assert_eq!(app.document.objects().len(), 2);
}

#[test]
fn pipe_multiple_keeps_picks_together_and_cancel_releases_highlights() {
    let mut app = test_app();
    enter(&mut app, "Line w0,0,0 w0,0,5");
    enter(&mut app, "Line w5,0,0 w5,0,5");
    let source_ids = app.document.objects().map(|o| o.id()).collect::<Vec<_>>();
    for token in ["Pipe Multiple", "SelCrv", ""] {
        enter(&mut app, token);
    }
    assert_eq!(app.pipe_session.as_ref().unwrap().rails.len(), 2);
    assert_eq!(app.document.selected_object_count(), 2);
    assert_eq!(app.document.objects().len(), 2);
    app.cancel_current_prompt_or_selection();
    assert!(app.pipe_session.is_none());
    assert_eq!(app.document.selected_object_count(), 0);
    for token in [
        "Pipe Multiple",
        "SelCrv",
        "",
        "Thick=Yes",
        "1",
        "WallThickness",
        "0.5",
    ] {
        enter(&mut app, token);
    }
    assert!(app.pipe_session.is_none());
    assert_eq!(app.document.objects().len(), 4);
    for object in app.document.objects().skip(2) {
        assert!(matches!(object.geometry(), Geometry::Brep(b) if b.is_extrusion() && b.is_solid()));
    }
    assert_eq!(app.document.undo().unwrap().as_deref(), Some("Pipe"));
    assert_eq!(
        app.document.objects().map(|o| o.id()).collect::<Vec<_>>(),
        source_ids
    );
    assert_eq!(app.document.selected_object_count(), 0);
}

#[test]
fn pipe_chains_surface_edges_without_temporary_objects() {
    let mut app = test_app();
    enter(&mut app, "Box 0,0,0 8,8,0 7");
    let parent = app.document.objects().next().unwrap();
    let id = parent.id();
    let source = parent.geometry_snapshot().clone();
    let Geometry::Brep(brep) = parent.geometry() else {
        panic!("expected a box");
    };
    let first = brep.edges()[0].vertices();
    let edge = brep
        .edges()
        .iter()
        .enumerate()
        .skip(1)
        .find(|(_, edge)| edge.vertices().iter().any(|v| first.contains(v)))
        .unwrap()
        .0;
    enter(&mut app, "Pipe ChainEdges");
    app.pick_pipe_rail(id, Some(0));
    app.pick_pipe_rail(id, Some(edge));
    assert_eq!(app.pipe_edge_highlights().count(), 2);
    enter(&mut app, "");
    assert_eq!(app.pipe_session.as_ref().unwrap().rails.len(), 1);
    for token in ["0.5", "", ""] {
        enter(&mut app, token);
    }
    assert!(app.pipe_session.is_none(), "{:?}", app.command_log);
    assert!(source.shares_storage_with(app.document.object(id).unwrap().geometry_snapshot()));
    assert!(
        matches!(app.document.objects().last().unwrap().geometry(), Geometry::Brep(b) if b.is_solid())
    );
    assert_eq!(app.document.undo().unwrap().as_deref(), Some("Pipe"));
    assert_eq!(app.document.objects().len(), 1);
}

#[test]
fn pipe_subd_previews_the_control_cage_and_commits_one_edit() {
    let mut app = test_app();
    app.viewports[0] = Viewport::new(ViewKind::Top);
    app.maximized_viewport = Some(0);
    app.grid_snap = false;
    app.osnap = false;
    enter(&mut app, "Line w0,0,0 w0,0,12");
    let rail = app.document.objects().next().unwrap().id();
    enter(&mut app, &format!("Pipe Curve={rail}"));
    let ctx = egui::Context::default();
    viewport_frame(&mut app, &ctx, vec![]);
    viewport_frame(&mut app, &ctx, vec![]);
    let a = screen_point(&app, point(1., 0., 0.));
    let b = screen_point(&app, point(2., 0., 0.));
    let surface = viewport_frame(&mut app, &ctx, vec![egui::Event::PointerMoved(a)]);
    enter(&mut app, "Output");
    enter(&mut app, "SubD");
    let small = viewport_frame(&mut app, &ctx, vec![egui::Event::PointerMoved(a)]);
    let large = viewport_frame(&mut app, &ctx, vec![egui::Event::PointerMoved(b)]);
    assert_ne!(surface, small);
    assert!(!large.is_empty());
    assert_ne!(small, large);
    viewport_click(&mut app, &ctx, b);
    enter(&mut app, "3");
    enter(&mut app, "w0,0,6");
    enter(&mut app, "2.5");
    assert_eq!(app.document.undo_label(), Some("Line"));
    assert_eq!(app.document.objects().len(), 1);
    enter(&mut app, "");
    let Geometry::SubD(subd) = app.document.objects().last().unwrap().geometry() else {
        panic!("expected SubD: {:?}", app.command_log);
    };
    assert_eq!(subd.faces().len(), 64);
    assert!(subd.edge_face_counts().iter().all(|&count| count == 2));
    assert_eq!(app.document.undo().unwrap().as_deref(), Some("Pipe"));
    assert!(!app.document.is_selected(rail));
    assert_eq!(app.document.objects().len(), 1);
    assert_eq!(app.document.redo().unwrap().as_deref(), Some("Pipe"));
    assert_eq!(app.document.objects().len(), 2);
}

#[test]
fn pipe_subd_closed_rail_updates_its_preview_and_needs_one_radius() {
    let mut app = test_app();
    app.viewports[0] = Viewport::new(ViewKind::Top);
    app.active_viewport = 0;
    app.maximized_viewport = Some(0);
    app.grid_snap = false;
    app.osnap = false;
    enter(&mut app, "Circle 0,0,0 8");
    app.viewports[0] = Viewport::new(ViewKind::Front);
    let rail = app.document.objects().next().unwrap().id();
    enter(
        &mut app,
        &format!("Pipe Curve={rail} Thick=Yes Output=SubD"),
    );
    let ctx = egui::Context::default();
    viewport_frame(&mut app, &ctx, vec![]);
    viewport_frame(&mut app, &ctx, vec![]);
    let a = screen_point(&app, point(8., 0., 1.));
    let b = screen_point(&app, point(8., 0., 2.));
    let small = viewport_frame(&mut app, &ctx, vec![egui::Event::PointerMoved(a)]);
    let large = viewport_frame(&mut app, &ctx, vec![egui::Event::PointerMoved(b)]);
    assert!(!small.is_empty());
    assert!(small != large, "preview should follow both radius points");
    viewport_click(&mut app, &ctx, b);
    assert!(app.pipe_session.as_ref().unwrap().radius_plane().is_none());
    assert_eq!(app.document.objects().len(), 1);
    enter(&mut app, "");
    let Geometry::SubD(pipe) = app.document.objects().last().unwrap().geometry() else {
        panic!("expected a closed SubD pipe: {:?}", app.command_log);
    };
    assert_eq!(pipe.faces().len(), 4112);
    assert!(pipe.edge_face_counts().iter().all(|&n| n == 2));
    assert_eq!(app.document.undo().unwrap().as_deref(), Some("Pipe"));
    assert_eq!(app.document.objects().len(), 1);
}
