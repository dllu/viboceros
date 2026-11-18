use super::*;

fn ring_radius(wall: &NurbsSurface, row: usize) -> f64 {
    let seam = wall.control_point(0, row).unwrap().point();
    seam.distance_to(cylinder_end(wall, row)).unwrap()
}

#[test]
fn solid_axial_truncated_cone_sizes_the_top_live_on_the_accepted_axis_plane() {
    let mut app = test_app();
    app.viewports[0] = Viewport::new(ViewKind::Perspective);
    app.maximized_viewport = Some(0);
    let ctx = egui::Context::default();
    let original_plane = app.viewports[0].construction_plane();
    for step in [
        "TruncatedCone DirectionConstraint=None",
        "w1,2,3",
        "3",
        "w4,6,15",
        "Diameter",
    ] {
        enter(&mut app, step);
    }
    let input = app.solid_axial_preview_input().unwrap();
    let top_plane = input.end_plane().unwrap();
    let center = point(4., 6., 15.);
    assert!(top_plane.origin().distance_to(center).unwrap() < 1e-9);
    assert!(input.height_constraint().is_none());
    assert!(input.preferences.diameter);
    assert!(app.document.undo_label().is_none());
    let cursor = top_plane.point_at([4., 0., 0.]).unwrap();
    // Settle the panel layout before projecting the pointer into the viewport.
    viewport_frame(&mut app, &ctx, vec![]);
    viewport_frame(&mut app, &ctx, vec![]);
    let pointer = screen_point(&app, cursor);
    viewport_frame(&mut app, &ctx, vec![egui::Event::PointerMoved(pointer)]);
    let pointer = screen_point(&app, cursor);
    let wires = viewport_frame(&mut app, &ctx, vec![egui::Event::PointerMoved(pointer)]);
    assert!(wires.len() > 64);
    viewport_click(&mut app, &ctx, pointer);

    let shape = brep(&app).clone();
    assert!(shape.is_solid());
    let wall = shape.faces()[0].surface();
    let radius = ring_radius(wall, 1);
    assert!(
        (radius - 2.).abs() < 1e-4,
        "radius {radius}, picked {:?}, target {cursor:?}",
        app.last_point,
    );
    assert!(cylinder_end(wall, 1).distance_to(center).unwrap() < 1e-9);
    assert_eq!(app.viewports[0].construction_plane(), original_plane);
    assert_eq!(app.document.undo_label(), Some("TruncatedCone"));
    enter(&mut app, "Undo");
    assert!(app.document.objects().next().is_none());
    enter(&mut app, "Redo");
    assert_eq!(brep(&app), &shape);
}

#[test]
fn solid_axial_truncated_cone_remembers_top_size_and_cancels_without_consuming_redo() {
    let mut app = test_app();
    for step in ["TruncatedCone Solid=No", "w0,0,0", "", ""] {
        enter(&mut app, step);
    }
    let input = app.solid_axial_preview_input().unwrap();
    assert!(input.circle.is_some());
    assert!(input.end_plane().is_none());
    assert_eq!(input.preferences.height, 0.);
    assert!(app.document.undo_label().is_none());
    enter(&mut app, "5");
    enter(&mut app, "Diameter=4");
    let wall = brep(&app).faces()[0].surface();
    assert_eq!(ring_radius(wall, 0), 1.);
    assert_eq!(ring_radius(wall, 1), 2.);
    enter(&mut app, "Undo");
    for step in ["TruncatedCone", "w0,0,0", "", "", "Radius", ""] {
        enter(&mut app, step);
    }
    let saved = brep(&app).clone();
    assert_eq!(ring_radius(saved.faces()[0].surface(), 1), 4.);
    assert!(!saved.is_solid());
    enter(&mut app, "Undo");
    for step in ["TruncatedCone", "w0,0,0", "", "", "Cancel"] {
        enter(&mut app, step);
    }
    assert!(app.solid_axial_preview_input().is_none());
    assert!(app.document.objects().next().is_none());
    enter(&mut app, "Redo");
    assert_eq!(brep(&app), &saved);
}

#[test]
fn solid_axial_truncated_cone_reference_getters_continue_to_the_top_size() {
    let mut app = test_app();
    enter(&mut app, "Line 0,0,0 0,10,10");
    let guide = app.document.objects().last().unwrap().id();
    let reference = app.document.object(guide).unwrap().geometry().clone();
    for step in [
        "TruncatedCone DirectionConstraint=AroundCurve",
        &format!("Curve={guide}"),
        "w0,3,3",
        "3",
        "5",
        "Radius=0",
    ] {
        enter(&mut app, step);
    }
    let apex = cylinder_end(brep(&app).faces()[0].surface(), 1);
    assert!(
        apex.distance_to(point(0., 6.535533905932738, 6.535533905932738))
            .unwrap()
            < 1e-9
    );
    assert_eq!(brep(&app).faces().len(), 2);
    assert!(!app.document.is_selected(guide));
    assert_eq!(app.document.object(guide).unwrap().geometry(), &reference);
    enter(&mut app, "Undo");

    for step in [
        "TruncatedCone DirectionConstraint=Vertical Tangent",
        "Point",
        "4,0",
        "Point",
        "0,4",
        "Point",
        "-4,0",
        "5",
        "1",
    ] {
        enter(&mut app, step);
    }
    assert_eq!(ring_radius(brep(&app).faces()[0].surface(), 0), 4.);
    for p in [point(4., 0., 0.), point(0., 4., 0.), point(-4., 0., 0.)] {
        app.document.add_geometry(Geometry::Point(p)).unwrap();
    }
    for step in ["TruncatedCone FitPoints", "SelAll"] {
        enter(&mut app, step);
    }
    assert!(app.solid_axial_fit_preview().is_some());
    for step in ["", "w0,0,-5", "w2,0,-9"] {
        enter(&mut app, step);
    }
    let wall = brep(&app).faces()[0].surface();
    assert!(
        cylinder_end(wall, 1)
            .distance_to(point(0., 0., -5.))
            .unwrap()
            < 1e-9
    );
    assert!((ring_radius(wall, 1) - 2.).abs() < 1e-9);
    assert_eq!(app.document.selected_object_count(), 0);
    enter(&mut app, "Undo");
    assert_eq!(app.document.selected_object_count(), 0);
}
