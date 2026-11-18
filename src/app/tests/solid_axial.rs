use super::{
    circle_preview::{screen_point, viewport_click, viewport_frame},
    curve_fillet::enter,
    *,
};
use viboceros_command::solid_axial::{Direction, Kind};
use viboceros_geometry::{Brep, NurbsSurface};

fn brep(app: &VibocerosApp) -> &Brep {
    let Geometry::Brep(brep) = app.document.objects().last().unwrap().geometry() else {
        panic!("expected a boundary representation");
    };
    brep
}

fn cylinder_end(wall: &NurbsSurface, row: usize) -> Point3 {
    let a = wall.control_point(0, row).unwrap().point();
    let b = wall.control_point(4, row).unwrap().point();
    a.midpoint(b).unwrap()
}

#[test]
fn solid_axial_viewport_height_picks_preview_the_wall_and_commit_one_edit() {
    for (name, kind) in [("Cone", Kind::Cone), ("Cyl", Kind::Cylinder)] {
        let mut app = test_app();
        app.viewports[0] = Viewport::new(ViewKind::Top);
        app.maximized_viewport = Some(0);
        let ctx = egui::Context::default();
        enter(&mut app, name);
        enter(&mut app, "DirectionConstraint");
        enter(&mut app, "Vertical");
        enter(&mut app, "w0,0,0");
        viewport_frame(&mut app, &ctx, vec![]);
        let base_point = screen_point(&app, point(3., 0., 0.));
        let wires = viewport_frame(&mut app, &ctx, vec![egui::Event::PointerMoved(base_point)]);
        assert!(!wires.is_empty());
        viewport_click(&mut app, &ctx, base_point);
        let input = app.solid_axial_preview_input().unwrap();
        assert_eq!(input.kind, kind);
        assert!(input.height_constraint().is_some());

        app.viewports[0] = Viewport::new(ViewKind::Front);
        viewport_frame(&mut app, &ctx, vec![]);
        let end = point(0., 0., -5.);
        let pointer = screen_point(&app, end);
        let wires = viewport_frame(&mut app, &ctx, vec![egui::Event::PointerMoved(pointer)]);
        assert!(wires.len() > 64);
        assert!(app.document.undo_label().is_none());
        viewport_click(&mut app, &ctx, pointer);
        let saved = brep(&app).clone();
        let wall = saved.faces()[0].surface();
        let actual = if kind == Kind::Cylinder {
            cylinder_end(wall, 1)
        } else {
            wall.control_point(0, 0).unwrap().point()
        };
        assert!(actual.distance_to(end).unwrap() < 1e-5);
        assert!(saved.is_solid());
        assert_eq!(app.document.undo_label(), Some(kind.name()));
        enter(&mut app, "Undo");
        assert!(app.document.objects().next().is_none());
        enter(&mut app, "Redo");
        assert_eq!(brep(&app), &saved);
    }
}

#[test]
fn solid_axial_free_axis_uses_the_cursor_side_for_signed_typed_heights() {
    for name in ["Cone", "Cylinder"] {
        let mut app = test_app();
        for step in [name, "w1,2,3", "3", "DirectionConstraint"] {
            enter(&mut app, step);
        }
        assert_eq!(
            app.solid_axial_preview_input()
                .unwrap()
                .preferences
                .direction,
            Direction::None
        );
        app.observe_solid_axial_cursor(point(4., 6., 15.));
        enter(&mut app, "-13");
        let wall = brep(&app).faces()[0].surface();
        let actual = if name == "Cylinder" {
            cylinder_end(wall, 1)
        } else {
            wall.control_point(0, 0).unwrap().point()
        };
        assert!(actual.distance_to(point(-2., -2., -9.)).unwrap() < 1e-9);
    }
}

#[test]
fn solid_axial_cylinder_range_caps_and_representation_follow_the_prompts() {
    let mut app = test_app();
    for step in [
        "UseExtrusions Polysurface",
        "Cylinder Solid=No",
        "0,0",
        "Diameter=6",
        "BothSides=Yes",
        "-5",
    ] {
        enter(&mut app, step);
    }
    let output = brep(&app);
    assert!(!output.is_extrusion());
    assert!(!output.is_solid());
    let wall = output.faces()[0].surface();
    for (row, height) in [5., -5.].into_iter().enumerate() {
        assert!(
            cylinder_end(wall, row)
                .distance_to(point(0., 0., height))
                .unwrap()
                < 1e-12
        );
    }
    enter(&mut app, "Undo");
    for step in ["Cylinder Solid=Yes", "0,0", "Enter", "Enter"] {
        enter(&mut app, step);
    }
    assert!(brep(&app).is_solid());
    assert_eq!(brep(&app).faces().len(), 3);
}

#[test]
fn solid_axial_guide_and_fit_handoffs_keep_sources_and_grouped_history() {
    let mut app = test_app();
    enter(&mut app, "Line 0,0,0 0,10,10");
    let guide = app.document.objects().last().unwrap().id();
    let source = app.document.object(guide).unwrap().geometry().clone();
    for step in [
        "Cone DirectionConstraint=AroundCurve",
        &format!("Curve={guide}"),
        "Solid=No",
        "w0,3,3",
        "w6,3,8",
        "5",
    ] {
        enter(&mut app, step);
    }
    let apex = brep(&app).faces()[0]
        .surface()
        .control_point(0, 0)
        .unwrap()
        .point();
    assert!(
        apex.distance_to(point(0., 6.535533905932738, 6.535533905932738))
            .unwrap()
            < 1e-9
    );
    assert!(!brep(&app).is_solid());
    assert_eq!(app.document.object(guide).unwrap().geometry(), &source);
    assert!(!app.document.is_selected(guide));

    for step in [
        "Cylinder DirectionConstraint=AroundCurve",
        &format!("Curve={guide}"),
        "2Point",
        "w0,3,3",
        "w4,3,3",
        "5",
    ] {
        enter(&mut app, step);
    }
    assert!(
        cylinder_end(brep(&app).faces()[0].surface(), 1)
            .distance_to(point(2., 3., 8.))
            .unwrap()
            < 1e-9
    );
    assert!(!app.document.is_selected(guide));
    for step in [
        "Cylinder Tangent",
        "Point",
        "4,0",
        "Point",
        "0,4",
        "Point",
        "-4,0",
        "5",
    ] {
        enter(&mut app, step);
    }
    assert!(brep(&app).is_extrusion());
    for p in [point(4., 0., 0.), point(0., 4., 0.), point(-4., 0., 0.)] {
        app.document.add_geometry(Geometry::Point(p)).unwrap();
    }
    enter(&mut app, "Cylinder FitPoints");
    enter(&mut app, "SelAll");
    assert!(app.solid_axial_fit_preview().is_some());
    enter(&mut app, "");
    assert!(app.solid_axial_preview_input().unwrap().circle.is_some());
    enter(&mut app, "w0,0,-5");
    assert!(
        cylinder_end(brep(&app).faces()[0].surface(), 1)
            .distance_to(point(0., 0., -5.))
            .unwrap()
            < 1e-9
    );
    assert_eq!(app.document.selected_object_count(), 0);
    assert_eq!(app.document.undo_label(), Some("Cylinder"));
    enter(&mut app, "Undo");
    assert_eq!(app.document.selected_object_count(), 0);
}
