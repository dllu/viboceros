use super::*;
use crate::app::extrude_tapered::Stage;
use viboceros_geometry::NurbsSurface;

fn rectangle(app: &mut VibocerosApp, [x, y, xx, yy]: [f64; 4], z: f64) -> ObjectId {
    app.document
        .add_geometry(Geometry::NurbsSurface(
            NurbsSurface::try_bilinear([
                point(x, y, z),
                point(xx, y, z),
                point(xx, yy, z),
                point(x, yy, z),
            ])
            .unwrap(),
        ))
        .unwrap()
}

fn scene() -> (VibocerosApp, ObjectId, ObjectId) {
    let mut app = test_app();
    let source = rectangle(&mut app, [0., 0., 6., 4.], 2.);
    let cutter = rectangle(&mut app, [-8., -8., 13., 11.], 5.);
    app.document.clear_history().unwrap();
    (app, source, cutter)
}

#[test]
fn surface_taper_boundary_direction_returns_to_frozen_postselection_and_cancels() {
    let (mut app, source, _) = scene();
    enter(&mut app, "ExtrudeSrfTapered _ToBoundary _DraftAngle=-10");
    assert!(app.object_prompt.is_some());
    app.apply_selection_click(SelectionClick {
        object_id: Some(source),
        mode: SelectionMode::Replace,
    });
    enter(&mut app, "");
    assert!(app.picking_surface_boundary());
    assert!(
        app.command_log
            .back()
            .unwrap()
            .contains("Select a boundary surface (Direction)")
    );
    enter(&mut app, "_Direction");
    assert!(!app.picking_surface_boundary());
    enter(&mut app, "w0");
    enter(&mut app, "w0,0,2");
    assert!(app.picking_tapered_boundary());
    let Some(InteractiveCommand::ExtrudeTapered {
        stage: Stage::Boundary,
        options,
        ..
    }) = app.active_command
    else {
        panic!()
    };
    assert_eq!(options.draft_angle, -10.);
    assert_eq!(options.direction.unwrap().to_array(), [0., 0., 1.]);
    enter(&mut app, "_SelID bad-id");
    assert!(app.picking_tapered_boundary());
    enter(&mut app, "Cancel");
    assert!(app.active_command.is_none());
    assert_eq!(app.document.objects().count(), 2);
    assert!(!app.document.can_undo());
}

#[test]
fn surface_taper_boundary_viewport_face_pick_builds_a_solid_and_replays_deletion() {
    let (mut app, source, cutter) = scene();
    check_viewport_cut(&mut app, source, cutter, "ExtrudeSrfTapered");
}

#[test]
fn surface_taper_oblique_boundary_pick_replays_rhinos_open_fallback() {
    let mut app = test_app();
    let source = rectangle(&mut app, [0., 0., 6., 4.], 2.);
    let cutter = rectangle(&mut app, [-9., -9., 15., 13.], 5.);
    app.document
        .select_objects_direct([source], SelectionMode::Replace)
        .unwrap();
    app.document.clear_history().unwrap();
    enter(
        &mut app,
        "ExtrudeSrfTapered ToBoundary DraftAngle=-10 DeleteInput=Yes Solid=No",
    );
    enter(&mut app, "Direction");
    assert!(app.accept_drafting_point(point(0., 0., 0.)));
    assert!(app.accept_drafting_point(point(1., 0., 6.)));
    assert!(app.picking_tapered_boundary());
    enter(&mut app, &cutter.to_string());
    assert!(app.active_command.is_none(), "{:?}", app.command_log);
    assert!(app.document.object(source).is_some());
    assert!(app.document.is_selected(source));
    assert!(app.document.is_selected(cutter));
    let Geometry::Brep(result) = app
        .document
        .objects()
        .find(|o| o.id() != source && o.id() != cutter)
        .unwrap()
        .geometry()
    else {
        panic!("expected the fallback sweep");
    };
    assert!(!result.is_solid());
    assert_eq!((result.faces().len(), result.edges().len()), (5, 16));
    let bounds = result.bounds();
    assert!((bounds.max().z() - 35.19848539598654).abs() < 1e-6);
    assert!((bounds.min().y() + 5.765110338097707).abs() < 1e-6);
    enter(&mut app, "Undo");
    assert_eq!(app.document.objects().len(), 2);
    assert_eq!(
        app.document.selected_object_ids().collect::<Vec<_>>(),
        [source]
    );
    enter(&mut app, "Redo");
    assert_eq!(app.document.objects().len(), 3);
    assert_eq!(
        app.document.selected_object_ids().collect::<Vec<_>>(),
        [source]
    );
}

#[test]
fn curve_taper_boundary_viewport_face_pick_builds_a_solid_and_replays_deletion() {
    let mut app = test_app();
    let cutter = rectangle(&mut app, [-8., -8., 13., 11.], 5.);
    let source = app
        .document
        .add_geometry(Geometry::Polyline(
            viboceros_geometry::Polyline3::try_new(
                vec![
                    point(0., 0., 2.),
                    point(6., 0., 2.),
                    point(6., 4., 2.),
                    point(0., 4., 2.),
                    point(0., 0., 2.),
                ],
                app.document.tolerance(),
            )
            .unwrap(),
        ))
        .unwrap();
    app.document.clear_history().unwrap();
    check_viewport_cut(&mut app, source, cutter, "ExtrudeCrvTapered");
}

#[test]
fn surface_taper_oblique_boundary_direction_and_viewport_click_close_the_disk() {
    use viboceros_geometry::{BoundingBox3, Brep, Circle3, UnitVector3};

    for corners in [
        [
            point(-4., -5., 5.),
            point(6., -5., 5.),
            point(6., 5., 5.),
            point(-4., 5., 5.),
        ],
        [
            point(-9., -9., 4.3),
            point(15., -9., 5.5),
            point(15., 13., 6.8),
            point(-9., 13., 5.2),
        ],
    ] {
        let mut app = test_app();
        let tolerance = app.document.tolerance();
        let circle = Circle3::try_new(
            point(0., 0., 0.),
            3.,
            UnitVector3::try_new(0., 0., 1., tolerance).unwrap(),
            tolerance,
        )
        .unwrap()
        .to_nurbs()
        .unwrap();
        let source = app
            .document
            .add_geometry(Geometry::Brep(
                Brep::try_planar_face(&circle, tolerance).unwrap(),
            ))
            .unwrap();
        let chart = NurbsSurface::try_bilinear(corners).unwrap();
        let cutter = app
            .document
            .add_geometry(Geometry::NurbsSurface(chart.clone()))
            .unwrap();
        app.document
            .select_objects_direct([source], SelectionMode::Replace)
            .unwrap();
        app.document.clear_history().unwrap();
        enter(
            &mut app,
            "ExtrudeSrfTapered ToBoundary DraftAngle=-10 Solid=No DeleteInput=Yes",
        );
        enter(&mut app, "Direction");
        enter(&mut app, "w0,0,0");
        enter(&mut app, "w1,0,6");
        assert!(app.picking_tapered_boundary());
        let bounds =
            BoundingBox3::from_points(corners.into_iter().chain([point(0., 0., 0.)])).unwrap();
        pick_boundary_in_top_view(
            &mut app,
            [bounds.min(), bounds.max()],
            chart.evaluate(0.08, 0.16).unwrap(),
        );
        assert!(app.active_command.is_none(), "{:?}", app.command_log);
        assert!(app.document.object(source).is_none());
        let Geometry::Brep(result) = app
            .document
            .objects()
            .find(|o| o.id() != cutter)
            .unwrap()
            .geometry()
        else {
            panic!("expected a capped oblique extrusion");
        };
        assert!(result.is_solid());
        assert_eq!((result.faces().len(), result.edges().len()), (6, 12));
        assert!(result.faces().iter().any(|face| face.surface() == &chart));
        assert_eq!(
            app.document.selected_object_ids().collect::<Vec<_>>(),
            [cutter]
        );
        enter(&mut app, "Undo");
        assert!(app.document.object(source).is_some());
        enter(&mut app, "Redo");
        assert!(app.document.object(source).is_none());
    }
}

#[test]
fn surface_taper_oblique_boundary_viewport_retries_after_an_enclosed_cutter_hole() {
    use viboceros_geometry::{Brep, Circle3, Polyline3, UnitVector3};

    let mut app = test_app();
    let tolerance = app.document.tolerance();
    let circle = Circle3::try_new(
        point(0., 0., 0.),
        3.,
        UnitVector3::try_new(0., 0., 1., tolerance).unwrap(),
        tolerance,
    )
    .unwrap()
    .to_nurbs()
    .unwrap();
    let source = app
        .document
        .add_geometry(Geometry::Brep(
            Brep::try_planar_face(&circle, tolerance).unwrap(),
        ))
        .unwrap();
    let outline = |[x, y, xx, yy]: [f64; 4], z| {
        Polyline3::try_new(
            vec![
                point(x, y, z),
                point(xx, y, z),
                point(xx, yy, z),
                point(x, yy, z),
                point(x, y, z),
            ],
            tolerance,
        )
        .unwrap()
        .to_nurbs()
        .unwrap()
    };
    let mut cutters = Vec::new();
    for (outer, hole, height) in [
        ([-9., -9., 15., 13.], [0.8, -0.25, 1.3, 0.25], 5.),
        ([-7., -8., 16., 12.], [7., -0.25, 8., 0.25], 4.),
    ] {
        let face = Brep::try_planar_face_with_holes(
            &outline(outer, height),
            &[outline(hole, height)],
            tolerance,
        )
        .unwrap()
        .try_split_kinky_edges(&[0, 1], tolerance.angular(), tolerance)
        .unwrap()
        .unwrap();
        let chart = face.faces()[0].surface().clone();
        cutters.push((
            app.document.add_geometry(Geometry::Brep(face)).unwrap(),
            chart,
        ));
    }
    app.document
        .select_objects_direct([source], SelectionMode::Replace)
        .unwrap();
    app.document.clear_history().unwrap();
    enter(
        &mut app,
        "ExtrudeSrfTapered ToBoundary DraftAngle=-10 DeleteInput=Yes",
    );
    enter(&mut app, "Direction");
    enter(&mut app, "w0,0,0");
    enter(&mut app, "w1,0,6");
    assert!(app.picking_tapered_boundary(), "{:?}", app.command_log);
    let scene_bounds = [point(-10., -10., 0.), point(17., 14., 8.)];
    pick_boundary_in_top_view(&mut app, scene_bounds, point(-8., -2., 5.));
    assert!(app.picking_tapered_boundary(), "{:?}", app.command_log);
    assert_eq!(app.document.objects().len(), 3);
    assert!(!app.document.can_undo());
    assert!(app.document.is_selected(source));
    assert!(app.document.is_selected(cutters[0].0));

    pick_boundary_in_top_view(&mut app, scene_bounds, point(15.5, -2., 4.));
    assert!(app.active_command.is_none(), "{:?}", app.command_log);
    assert!(app.document.object(source).is_none());
    let Geometry::Brep(result) = app
        .document
        .objects()
        .find(|object| cutters.iter().all(|(id, _)| *id != object.id()))
        .unwrap()
        .geometry()
    else {
        panic!("expected a trimmed extrusion");
    };
    assert!(result.is_solid());
    assert!(
        result
            .faces()
            .iter()
            .any(|face| face.surface() == &cutters[1].1)
    );
    enter(&mut app, "Undo");
    assert!(app.document.object(source).is_some());
    enter(&mut app, "Redo");
    assert!(app.document.object(source).is_none());
}

fn pick_boundary_in_top_view(app: &mut VibocerosApp, bounds: [Point3; 2], aim: Point3) {
    use super::super::circle_preview::{screen_point, viewport_click, viewport_frame};

    app.viewports[0] = Viewport::new(ViewKind::Top);
    app.active_viewport = 0;
    app.maximized_viewport = Some(0);
    app.osnap = false;
    let ctx = egui::Context::default();
    viewport_frame(app, &ctx, vec![]);
    viewport_frame(app, &ctx, vec![]);
    let bounds = viboceros_geometry::BoundingBox3::from_points(bounds).unwrap();
    app.viewports[0].zoom_bounding_box(bounds).unwrap();
    let pointer = (0..8)
        .find_map(|_| {
            let pointer = screen_point(app, aim);
            viewport_frame(app, &ctx, vec![egui::Event::PointerMoved(pointer)]);
            (screen_point(app, aim) == pointer).then_some(pointer)
        })
        .expect("boundary viewport layout");
    viewport_click(app, &ctx, pointer);
}

fn check_viewport_cut(app: &mut VibocerosApp, source: ObjectId, cutter: ObjectId, command: &str) {
    let original = app.document.object(source).unwrap().clone();
    app.document
        .select_objects_direct([source], SelectionMode::Replace)
        .unwrap();
    enter(
        app,
        &format!("{command} DraftAngle=-10 DeleteInput=Yes Solid=Yes"),
    );
    enter(app, "ToBoundary");
    pick_boundary_in_top_view(
        app,
        [point(-8., -8., 0.), point(13., 11., 8.)],
        point(-3., -2., 5.),
    );
    assert!(app.active_command.is_none(), "{:?}", app.command_log);
    assert!(app.document.object(source).is_none());
    assert_eq!(
        app.document.selected_object_ids().collect::<Vec<_>>(),
        [cutter]
    );
    let Geometry::Brep(result) = app
        .document
        .objects()
        .find(|o| o.id() != cutter)
        .unwrap()
        .geometry()
    else {
        panic!("expected tapered boundary solid");
    };
    assert!(result.is_solid());
    assert!(
        (result
            .signed_volume(app.document.tolerance())
            .unwrap()
            .abs()
            - 88.98871161228938)
            .abs()
            < 1e-5
    );
    enter(app, "Undo");
    assert_eq!(app.document.object(source), Some(&original));
    assert_eq!(
        app.document.selected_object_ids().collect::<Vec<_>>(),
        [source]
    );
    enter(app, "Redo");
    assert!(app.document.object(source).is_none());
    assert_eq!(app.document.selected_object_count(), 0);
}
