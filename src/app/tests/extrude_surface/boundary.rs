use super::super::circle_preview::{screen_point, viewport_click, viewport_frame};
use super::*;

fn cutter(app: &mut VibocerosApp) -> ObjectId {
    add_patch(
        app,
        [
            point(-2., -2., 6.),
            point(6., -2., 6.),
            point(6., 5., 6.),
            point(-2., 5., 6.),
        ],
    )
}

fn add_patch(app: &mut VibocerosApp, corners: [Point3; 4]) -> ObjectId {
    let surface = viboceros_geometry::NurbsSurface::try_bilinear(corners).unwrap();
    app.document
        .add_geometry(Geometry::NurbsSurface(surface))
        .unwrap()
}

fn result<'a>(app: &'a VibocerosApp, targets: &[ObjectId]) -> &'a viboceros_geometry::Brep {
    let object = app
        .document
        .objects()
        .find(|object| !targets.contains(&object.id()))
        .unwrap();
    let Geometry::Brep(result) = object.geometry() else {
        panic!("expected an extrusion B-rep")
    };
    result
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
        assert_eq!(
            app.document.selected_object_ids().collect::<Vec<_>>(),
            vec![boundary]
        );
        let result = result(&app, &[boundary]);
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
        assert!(!app.document.is_selected(boundary));
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
    assert!(app.document.is_selected(boundary));
    let result = result(&app, &[source, boundary]);
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
        let result = result(&app, &[source, boundary]);
        assert!(result.is_solid());
        assert_eq!((result.faces().len(), result.edges().len()), (faces, edges));
        let volume = 4.5 * (8. + std::f64::consts::PI);
        assert!((result.signed_volume(tolerance).unwrap().abs() - volume).abs() < 1e-5);
    }
}

#[cfg(feature = "native-smlib")]
#[test]
fn surface_extrusion_rejected_boundary_commits_retained_solid_and_finishes_getter() {
    let mut app = test_app();
    let source = surface_at(&mut app, 2., 0.);
    let boundary = add_patch(
        &mut app,
        [
            point(-0.2, -0.2, 6.),
            point(1., -0.2, 6.),
            point(1., 2., 6.),
            point(-0.2, 2., 6.),
        ],
    );
    app.document
        .select_objects_direct([source], SelectionMode::Replace)
        .unwrap();
    app.document.clear_history().unwrap();
    let settings = app.commands.surface_extrusion_settings();
    enter(
        &mut app,
        "ExtrudeSrf ToBoundary Solid=No DeleteInput=Yes BothSides=Yes",
    );
    enter(&mut app, &format!("_SelID {boundary}"));
    assert!(app.active_command.is_none(), "{:?}", app.command_log);
    assert!(
        app.command_log
            .back()
            .unwrap()
            .contains("doesn't form a boundary")
    );
    assert_eq!(app.commands.surface_extrusion_settings(), settings);
    assert_eq!(app.document.undo_label(), Some("ExtrudeSrf"));
    assert_eq!(app.document.selected_object_count(), 2);
    assert!(app.document.is_selected(source) && app.document.is_selected(boundary));
    let solid = result(&app, &[source, boundary]);
    assert!(solid.is_solid());
    assert!((solid.bounds().max().z() - 7.574943945906542).abs() < 1e-6);
    enter(&mut app, "Undo");
    assert_eq!(app.document.objects().count(), 2);
    assert_eq!(
        app.document.selected_object_ids().collect::<Vec<_>>(),
        vec![source]
    );
    enter(&mut app, "Redo");
    assert_eq!(app.document.objects().count(), 3);
    assert_eq!(
        app.document.selected_object_ids().collect::<Vec<_>>(),
        vec![source]
    );
}

#[cfg(feature = "native-smlib")]
#[test]
fn surface_extrusion_boundary_miss_preserves_existing_redo() {
    let mut app = test_app();
    let source = surface_at(&mut app, 2., 0.);
    let boundary = add_patch(
        &mut app,
        [
            point(4., 4., 6.),
            point(5., 4., 6.),
            point(5., 5., 6.),
            point(4., 5., 6.),
        ],
    );
    app.document.clear_history().unwrap();
    enter(&mut app, "Point 99,99,99");
    enter(&mut app, "Undo");
    assert_eq!(app.document.redo_label(), Some("Point"));
    app.document
        .select_objects_direct([source], SelectionMode::Replace)
        .unwrap();
    enter(
        &mut app,
        &format!("ExtrudeSrf DeleteInput=Yes ToBoundary={boundary}"),
    );
    assert!(app.active_command.is_none(), "{:?}", app.command_log);
    assert!(
        app.command_log
            .back()
            .unwrap()
            .contains("doesn't form a boundary")
    );
    assert_eq!(app.document.objects().count(), 2);
    assert_eq!(app.document.selected_object_count(), 2);
    assert!(app.document.undo_label().is_none());
    assert_eq!(app.document.redo_label(), Some("Point"));
}

#[cfg(feature = "native-smlib")]
#[test]
fn surface_extrusion_rejected_capsule_uses_padded_trim_bounds_and_unsplit_walls() {
    let mut app = test_app();
    let tolerance = app.document.tolerance();
    let source = app
        .document
        .add_geometry(super::tangents::profile("capsule", tolerance))
        .unwrap();
    let boundary = add_patch(
        &mut app,
        [
            point(-2., -2., 3.),
            point(4., -2., 6.),
            point(4., 5., 6.),
            point(-2., 5., 3.),
        ],
    );
    app.document
        .select_objects_direct([source], SelectionMode::Replace)
        .unwrap();
    enter(
        &mut app,
        &format!("ExtrudeSrf SplitAtTangents=Yes ToBoundary={boundary}"),
    );
    assert!(app.active_command.is_none(), "{:?}", app.command_log);
    assert!(
        app.command_log
            .back()
            .unwrap()
            .contains("doesn't form a boundary")
    );
    let solid = result(&app, &[source, boundary]);
    assert!(solid.is_solid());
    assert_eq!((solid.faces().len(), solid.edges().len()), (3, 3));
    assert!((solid.bounds().max().z() - 11.948221624995078).abs() < 1e-6);
}

#[cfg(feature = "native-smlib")]
#[test]
fn surface_extrusion_mixed_boundary_results_commit_together() {
    let mut app = test_app();
    let source = surface_at(&mut app, 2., 0.);
    let extra = add_patch(
        &mut app,
        [
            point(6., 0., 2.),
            point(8., 0., 2.),
            point(8., 3., 2.),
            point(6., 3., 2.),
        ],
    );
    let boundary = add_patch(
        &mut app,
        [
            point(-2., -2., 6.),
            point(7., -2., 6.),
            point(7., 5., 6.),
            point(-2., 5., 6.),
        ],
    );
    app.document
        .select_objects_direct([source, extra], SelectionMode::Replace)
        .unwrap();
    app.document.clear_history().unwrap();
    enter(
        &mut app,
        &format!("ExtrudeSrf DeleteInput=Yes ToBoundary={boundary}"),
    );
    assert!(app.active_command.is_none(), "{:?}", app.command_log);
    assert_eq!(
        app.command_log.back().unwrap(),
        "Extruded 1 surface face(s)"
    );
    assert!(app.document.object(source).is_none());
    assert!(app.document.is_selected(extra) && app.document.is_selected(boundary));
    assert_eq!(app.document.selected_object_count(), 2);
    let solids = app
        .document
        .objects()
        .filter(|object| ![extra, boundary].contains(&object.id()))
        .map(|object| {
            let Geometry::Brep(result) = object.geometry() else {
                panic!("expected both sweep products")
            };
            result
        })
        .collect::<Vec<_>>();
    assert_eq!(solids.len(), 2);
    assert!((solids[0].bounds().min().x() - 6.).abs() < 1e-6);
    assert!((solids[0].bounds().max().z() - 14.84523257866513).abs() < 1e-6);
    assert!(
        (solids[1]
            .signed_volume(app.document.tolerance())
            .unwrap()
            .abs()
            - 24.)
            .abs()
            < 1e-5
    );
    enter(&mut app, "Undo");
    assert_eq!(app.document.objects().count(), 3);
    assert_eq!(
        app.document.selected_object_ids().collect::<Vec<_>>(),
        vec![source]
    );
    enter(&mut app, "Redo");
    assert_eq!(app.document.objects().count(), 4);
    assert_eq!(app.document.selected_object_count(), 0);
}
