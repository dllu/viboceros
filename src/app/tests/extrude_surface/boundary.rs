use super::super::circle_preview::{screen_point, viewport_click, viewport_frame};
use super::*;

fn cutter(app: &mut VibocerosApp) -> ObjectId {
    let surface = viboceros_geometry::NurbsSurface::try_bilinear([
        point(-2., -2., 6.),
        point(6., -2., 6.),
        point(6., 5., 6.),
        point(-2., 5., 6.),
    ])
    .unwrap();
    app.document
        .add_geometry(Geometry::NurbsSurface(surface))
        .unwrap()
}

#[test]
fn surface_extrusion_boundary_picker_accepts_a_viewport_face_after_direction() {
    let mut app = test_app();
    app.viewports[0] = Viewport::new(ViewKind::Top);
    app.active_viewport = 0;
    app.maximized_viewport = Some(0);
    let source = surface_at(&mut app, 2., 0.);
    let boundary = cutter(&mut app);
    app.document
        .select_objects_direct([source], SelectionMode::Replace)
        .unwrap();
    app.document.clear_history().unwrap();
    let before = format!("{:?}", app.document);
    enter(
        &mut app,
        "ExtrudeSrf Solid=No BothSides=Yes DeleteInput=Yes",
    );
    enter(&mut app, "ToBoundary");
    assert!(app.picking_surface_boundary());
    assert!(!app.viewport_drafting_input(false, false).active);
    enter(&mut app, "Direction");
    assert!(!app.picking_surface_boundary());
    enter(&mut app, "w0");
    enter(&mut app, "w1,0,2");
    assert!(app.picking_surface_boundary());
    assert_eq!(format!("{:?}", app.document), before);

    let context = egui::Context::default();
    viewport_frame(&mut app, &context, Vec::new());
    viewport_frame(&mut app, &context, Vec::new());
    let pointer = screen_point(&app, point(3.5, 4., 6.));
    assert!(app.viewports[0].screen_rect().unwrap().contains(pointer));
    viewport_click(&mut app, &context, pointer);
    if cfg!(feature = "native-smlib") {
        assert!(app.active_command.is_none(), "{:?}", app.command_log);
        assert!(app.document.object(source).is_none());
        assert!(app.document.object(boundary).is_some());
        let Geometry::Brep(result) = app.document.selected_objects().next().unwrap().geometry()
        else {
            panic!("expected extrusion to the picked boundary");
        };
        assert!(result.is_solid());
        assert_eq!(result.faces().len(), 6);
        assert!(
            (result
                .signed_volume(app.document.tolerance())
                .unwrap()
                .abs()
                - 24.)
                .abs()
                < 1e-5
        );
        assert!(result.vertices().iter().any(|v| {
            v.point()
                .is_near(point(4., 3., 6.), app.document.tolerance())
        }));
        assert!(!app.commands.surface_extrusion_settings().options.both_sides);
        enter(&mut app, "Undo");
        assert!(app.document.object(source).is_some());
        enter(&mut app, "Redo");
        assert!(app.document.object(source).is_none());
    } else {
        assert!(app.picking_surface_boundary());
        assert_eq!(format!("{:?}", app.document), before);
    }
}

#[test]
fn surface_extrusion_boundary_picker_cancels_and_rejects_invalid_references() {
    let mut app = test_app();
    let source = surface_at(&mut app, 2., 0.);
    let point = app
        .document
        .add_geometry(Geometry::Point(point(2., 3., 6.)))
        .unwrap();
    app.document
        .select_objects_direct([source], SelectionMode::Replace)
        .unwrap();
    app.document.clear_history().unwrap();
    let before = format!("{:?}", app.document);
    enter(&mut app, "ExtrudeSrf _ToBoundary");
    assert!(app.picking_surface_boundary());
    enter(&mut app, &format!("_SelID {point}"));
    assert!(app.picking_surface_boundary());
    assert_eq!(format!("{:?}", app.document), before);
    enter(&mut app, "Cancel");
    assert!(app.active_command.is_none());
    assert_eq!(format!("{:?}", app.document), before);
}

#[cfg(feature = "native-smlib")]
#[test]
fn surface_extrusion_inline_boundary_survives_source_selection() {
    let mut app = test_app();
    let source = surface_at(&mut app, 2., 0.);
    let boundary = cutter(&mut app);
    enter(&mut app, &format!("ExtrudeSrf ToBoundary={boundary}:0"));
    assert!(app.object_prompt.is_some());
    app.apply_selection_click(SelectionClick {
        object_id: Some(source),
        mode: SelectionMode::Add,
    });
    enter(&mut app, "");
    assert!(app.active_command.is_none(), "{:?}", app.command_log);
    assert_eq!(app.document.objects().count(), 3);
    assert!(app.document.object(source).is_some());
    let Geometry::Brep(result) = app.document.selected_objects().next().unwrap().geometry() else {
        panic!("expected extrusion to the specified boundary");
    };
    assert!(result.is_solid());
    assert!(
        (result
            .signed_volume(app.document.tolerance())
            .unwrap()
            .abs()
            - 24.)
            .abs()
            < 1e-5
    );
}

#[cfg(feature = "native-smlib")]
#[test]
fn surface_extrusion_to_boundary_keeps_tangent_split_topology() {
    for (split, faces, edges) in [("No", 3, 3), ("Yes", 6, 12)] {
        let mut app = test_app();
        let tolerance = app.document.tolerance();
        let source = app
            .document
            .add_geometry(super::tangents::profile("capsule", tolerance))
            .unwrap();
        let cutter = viboceros_geometry::NurbsSurface::try_bilinear([
            point(-5., -2., 3.),
            point(5., -2., 6.),
            point(5., 5., 6.),
            point(-5., 5., 3.),
        ])
        .unwrap();
        let boundary = app
            .document
            .add_geometry(Geometry::NurbsSurface(cutter))
            .unwrap();
        app.document
            .select_objects_direct([source], SelectionMode::Replace)
            .unwrap();
        enter(
            &mut app,
            &format!("ExtrudeSrf SplitAtTangents={split} ToBoundary={boundary}"),
        );
        assert!(app.active_command.is_none(), "{:?}", app.command_log);
        let Geometry::Brep(result) = app.document.selected_objects().next().unwrap().geometry()
        else {
            panic!("expected a trimmed capsule extrusion");
        };
        assert!(result.is_solid());
        assert_eq!((result.faces().len(), result.edges().len()), (faces, edges));
        let volume = 4.5 * (8. + std::f64::consts::PI);
        assert!((result.signed_volume(tolerance).unwrap().abs() - volume).abs() < 1e-5);
    }
}
