use super::*;

#[test]
fn surface_apex_boundary_getter_picks_a_face_and_previews_only_to_the_boundary() {
    use super::super::circle_preview::{screen_point, viewport_click, viewport_frame};

    let mut app = test_app();
    app.maximized_viewport = Some(0);
    app.osnap = false;
    app.grid_snap = false;
    let source = planar_apex_source(&mut app, 0.);
    let boundary = boundary_surface(&mut app, 3.);
    app.document.clear_history().unwrap();
    let before = format!("{:?}", app.document);
    enter(&mut app, "ExtrudeSrfToPoint ToBoundary");
    assert!(app.picking_surface_apex_boundary());
    let context = egui::Context::default();
    viewport_frame(&mut app, &context, Vec::new());
    viewport_frame(&mut app, &context, Vec::new());
    let location = screen_point(&app, point(5., 2., 3.));
    viewport_click(&mut app, &context, location);
    assert!(
        !app.picking_surface_apex_boundary(),
        "{:?}",
        app.command_log
    );
    assert!(app.command_hint().contains("beyond the boundary"));
    assert_eq!(format!("{:?}", app.document), before);

    app.viewports[0] = Viewport::new(ViewKind::Front);
    let plane = app.viewports[0]
        .construction_plane()
        .with_origin(point(0., 1.5, 0.));
    app.viewports[0].set_construction_plane(plane);
    viewport_frame(&mut app, &context, Vec::new());
    viewport_frame(&mut app, &context, Vec::new());
    let apex = screen_point(&app, point(2., 1.5, 6.));
    let rim = screen_point(&app, point(2., 1.5, 3.));
    let preview = viewport_frame(&mut app, &context, vec![egui::Event::PointerMoved(apex)]);
    assert!(preview.iter().any(|p| (p[1].y - rim.y).abs() < 1e-3));
    assert!(preview.iter().flatten().all(|p| p.y >= rim.y - 1e-3));
    assert_eq!(format!("{:?}", app.document), before);
    viewport_click(&mut app, &context, apex);
    #[cfg(feature = "native-smlib")]
    {
        assert!(app.active_command.is_none(), "{:?}", app.command_log);
        let Geometry::Brep(result) = app.document.selected_objects().next().unwrap().geometry()
        else {
            panic!("expected truncated extrusion");
        };
        assert!(result.is_solid());
        assert_eq!(result.faces().len(), 6);
        assert!((result.signed_volume(app.document.tolerance()).unwrap() - 21.).abs() < 1e-6);
        let bounds = result.tight_bounds(app.document.tolerance()).unwrap();
        assert!((bounds.max().z() - 3.).abs() < 1e-6);
        assert_eq!(app.document.undo_label(), Some("ExtrudeSrfToPoint"));
        enter(&mut app, "Undo");
    }
    #[cfg(not(feature = "native-smlib"))]
    assert!(app.active_command.is_some());
    assert_eq!(app.document.objects().len(), 2);
    assert!(app.document.object(source).is_some());
    assert!(app.document.object(boundary).is_some());
}

#[test]
fn surface_apex_boundary_previews_and_accepts_a_coplanar_mouse_target() {
    use super::super::circle_preview::{screen_point, viewport_click, viewport_frame};
    use viboceros_geometry::{Frame3, NurbsSurface, UnitVector3};

    let mut app = test_app();
    app.viewports[0] = Viewport::new(ViewKind::Top);
    app.maximized_viewport = Some(0);
    app.osnap = false;
    app.grid_snap = false;
    let source = planar_apex_source(&mut app, 0.);
    let tolerance = app.document.tolerance();
    let plane = Frame3::try_from_unit_normal(
        point(5., 1.5, 0.),
        UnitVector3::try_new(1., -0.25, 0.4, tolerance).unwrap(),
    )
    .unwrap();
    let corners = [[-6., -6.], [6., -6.], [6., 6.], [-6., 6.]]
        .map(|[x, y]| plane.point_at([x, y, 0.]).unwrap());
    let boundary = app
        .document
        .add_geometry(Geometry::NurbsSurface(
            NurbsSurface::try_bilinear(corners).unwrap(),
        ))
        .unwrap();
    app.document.clear_history().unwrap();
    let before = format!("{:?}", app.document);
    enter(
        &mut app,
        &format!("ExtrudeSrfToPoint ToBoundary={boundary} DeleteInput=Yes"),
    );
    let context = egui::Context::default();
    viewport_frame(&mut app, &context, Vec::new());
    viewport_frame(&mut app, &context, Vec::new());
    app.viewports[0]
        .zoom_extents(&app.document, Default::default())
        .unwrap();
    viewport_frame(&mut app, &context, Vec::new());
    let expected = point(296. / 61., 111. / 122., 0.);
    let apex = screen_point(&app, point(8., 1.5, 0.));
    let start = screen_point(&app, point(0., 0., 0.));
    let cut = screen_point(&app, expected);
    let preview = viewport_frame(&mut app, &context, vec![egui::Event::PointerMoved(apex)]);
    assert!(
        preview
            .iter()
            .any(|[a, b]| a.distance(start) < 1e-3 && b.distance(cut) < 1e-3)
    );
    assert_eq!(format!("{:?}", app.document), before);
    viewport_click(&mut app, &context, apex);
    if cfg!(feature = "native-smlib") {
        assert!(app.active_command.is_none(), "{:?}", app.command_log);
        assert!(app.document.object(source).is_none());
        let Geometry::Brep(brep) = app.document.selected_objects().next().unwrap().geometry()
        else {
            panic!("expected truncated extrusion");
        };
        assert!(
            brep.vertices()
                .iter()
                .any(|v| v.point().distance_to(expected).unwrap() < 1e-5)
        );
        assert_eq!(app.document.undo_label(), Some("ExtrudeSrfToPoint"));
        enter(&mut app, "Undo");
    } else {
        assert!(app.active_command.is_some());
    }
    assert!(app.document.object(source).is_some());
    assert!(app.document.object(boundary).is_some());
}

#[test]
fn surface_apex_warped_boundary_previews_a_flat_sweep_and_accepts_the_mouse() {
    use super::super::circle_preview::{screen_point, viewport_click, viewport_frame};
    use viboceros_geometry::NurbsSurface;

    let mut app = test_app();
    app.viewports[0] = Viewport::new(ViewKind::Top);
    app.maximized_viewport = Some(0);
    app.osnap = false;
    app.grid_snap = false;
    let source = app
        .document
        .add_geometry(Geometry::NurbsSurface(
            NurbsSurface::try_bilinear([
                point(0., 0., 0.),
                point(2., 0., 0.),
                point(2., 3., 0.),
                point(0., 3., 0.),
            ])
            .unwrap(),
        ))
        .unwrap();
    let warped = NurbsSurface::try_bilinear([
        point(2.8, -4., -4.),
        point(4.8, 7., -3.),
        point(3.3, 7., 5.),
        point(4.5, -4., 3.),
    ])
    .unwrap();
    let picked = warped.evaluate(0.5, 0.5).unwrap();
    let boundary = app
        .document
        .add_geometry(Geometry::NurbsSurface(warped))
        .unwrap();
    app.document
        .select_objects_direct([source], SelectionMode::Replace)
        .unwrap();
    app.document.clear_history().unwrap();
    let before = format!("{:?}", app.document);
    enter(&mut app, "ExtrudeSrfToPoint ToBoundary DeleteInput=Yes");
    let context = egui::Context::default();
    viewport_frame(&mut app, &context, Vec::new());
    viewport_frame(&mut app, &context, Vec::new());
    app.viewports[0]
        .zoom_extents(&app.document, Default::default())
        .unwrap();
    viewport_frame(&mut app, &context, Vec::new());
    let pick = screen_point(&app, picked);
    viewport_click(&mut app, &context, pick);
    assert!(
        !app.picking_surface_apex_boundary(),
        "{:?}",
        app.command_log
    );
    viewport_frame(&mut app, &context, Vec::new());
    viewport_frame(&mut app, &context, Vec::new());
    let target = point(6., 1.5, 0.);
    let (apex, preview) = boundary_hover(&mut app, &context, target);
    let expected = point(3.82412509014911, 0.9560312725372776, 0.);
    let start = screen_point(&app, point(0., 0., 0.));
    let cut = screen_point(&app, expected);
    assert!(
        preview
            .iter()
            .any(|[a, b]| a.distance(start) < 0.001 && b.distance(cut) < 1.),
        "{} preview lines, closest start/end errors: {:?}, apex {apex:?}, log {:?}",
        preview.len(),
        preview
            .iter()
            .min_by(|a, b| a[0].distance(start).total_cmp(&b[0].distance(start)))
            .map(|[a, b]| (a.distance(start), b.distance(cut))),
        app.command_log
    );
    assert_eq!(format!("{:?}", app.document), before);
    viewport_click(&mut app, &context, apex);
    if cfg!(feature = "native-smlib") {
        assert!(app.active_command.is_none(), "{:?}", app.command_log);
        assert!(app.document.object(source).is_none());
        let Geometry::Brep(brep) = app.document.selected_objects().next().unwrap().geometry()
        else {
            panic!("expected the truncated sweep");
        };
        assert_eq!(brep.faces().len(), 6);
        assert!(
            brep.vertices()
                .iter()
                .any(|v| v.point().distance_to(expected).unwrap() < 1e-5)
        );
        assert_eq!(app.document.undo_label(), Some("ExtrudeSrfToPoint"));
        enter(&mut app, "Undo");
    } else {
        assert!(app.active_command.is_some());
    }
    assert!(app.document.object(source).is_some());
    assert!(app.document.object(boundary).is_some());
}

#[test]
fn surface_apex_previews_the_nearest_raised_cut_after_an_ambiguous_flat_pick() {
    use super::super::circle_preview::{screen_point, viewport_click, viewport_frame};
    use viboceros_geometry::{BoundingBox3, NurbsSurface};

    let mut app = test_app();
    app.document = Document::new(Tolerance::try_new(1e-6, 1e-12, 1e-8).unwrap());
    app.viewports[0] = Viewport::new(ViewKind::Top);
    app.maximized_viewport = Some(0);
    app.osnap = false;
    app.grid_snap = false;
    let source = app
        .document
        .add_geometry(Geometry::NurbsSurface(
            NurbsSurface::try_bilinear([
                point(0., 0., 0.),
                point(2., 0., 0.),
                point(2., 3., 0.),
                point(0., 3., 0.),
            ])
            .unwrap(),
        ))
        .unwrap();
    let cutter = NurbsSurface::try_new(
        2,
        1,
        3,
        2,
        vec![
            point(2.4, -1., 0.63),
            point(3.6, -1., -2.28),
            point(4.8, -1., 0.57),
            point(2.4, 4., 0.97),
            point(3.6, 4., -1.88),
            point(4.8, 4., 1.03),
        ],
        vec![0., 0., 0., 1., 1., 1.],
        vec![0., 0., 1., 1.],
    )
    .unwrap();
    let boundary = app
        .document
        .add_geometry(Geometry::NurbsSurface(cutter))
        .unwrap();
    app.document
        .select_objects_direct([source], SelectionMode::Replace)
        .unwrap();
    app.document.clear_history().unwrap();
    let before = format!("{:?}", app.document);
    enter(
        &mut app,
        &format!("ExtrudeSrfToPoint ToBoundary={boundary} DeleteInput=Yes"),
    );
    let context = egui::Context::default();
    viewport_frame(&mut app, &context, Vec::new());
    viewport_frame(&mut app, &context, Vec::new());
    let bounds = BoundingBox3::from_points([point(-0.5, -1.5, -2.), point(6.5, 4.5, 2.)]).unwrap();
    app.viewports[0].zoom_bounding_box(bounds).unwrap();
    viewport_frame(&mut app, &context, Vec::new());

    let (apex, preview) = boundary_hover(&mut app, &context, point(6., 1.5, 0.));
    assert!(preview.is_empty(), "two cuts cross the flat sweep");
    viewport_click(&mut app, &context, apex);
    assert!(app.active_command.is_some());
    assert!(
        app.command_log
            .iter()
            .any(|line| line.starts_with("Error:"))
    );
    assert_eq!(format!("{:?}", app.document), before);

    let plane = app.viewports[0]
        .construction_plane()
        .with_origin(point(0., 0., 0.25));
    app.viewports[0].set_construction_plane(plane);
    viewport_frame(&mut app, &context, Vec::new());
    let (apex, preview) = boundary_hover(&mut app, &context, point(6., 1.5, 0.25));
    let start = screen_point(&app, point(0., 0., 0.));
    let expected = point(2.6994134512323598, 0.6748533628080899, 0.11247556046801499);
    let cut = screen_point(&app, expected);
    let hover = app
        .surface_extrusion_sources
        .as_ref()
        .and_then(|sources| sources.apex_draft(&app.document))
        .and_then(|draft| draft.hover);
    assert!(
        preview
            .iter()
            .any(|[a, b]| a.distance(start) < 0.001 && b.distance(cut) < 3.),
        "{} preview lines, closest start/end errors {:?}, hover {hover:?}, log {:?}",
        preview.len(),
        preview
            .iter()
            .min_by(|a, b| a[0].distance(start).total_cmp(&b[0].distance(start)))
            .map(|[a, b]| (a.distance(start), b.distance(cut))),
        app.command_log
    );
    assert_eq!(format!("{:?}", app.document), before);
    viewport_click(&mut app, &context, apex);
    if cfg!(feature = "native-smlib") {
        assert!(app.active_command.is_none(), "{:?}", app.command_log);
        let Geometry::Brep(sweep) = app.document.selected_objects().next().unwrap().geometry()
        else {
            panic!("expected a truncated sweep");
        };
        assert!(sweep.is_solid());
        assert!(
            sweep
                .vertices()
                .iter()
                .any(|v| v.point().distance_to(expected).unwrap() < 1e-5)
        );
        assert!(app.document.object(source).is_none());
        enter(&mut app, "Undo");
    } else {
        assert!(app.active_command.is_some());
    }
    assert!(app.document.object(source).is_some());
}

fn boundary_hover(
    app: &mut VibocerosApp,
    context: &egui::Context,
    target: Point3,
) -> (egui::Pos2, Vec<[egui::Pos2; 2]>) {
    use super::super::circle_preview::{screen_point, viewport_frame};

    // A command log update can resize the viewport across several frames.
    for _ in 0..8 {
        let apex = screen_point(app, target);
        assert!(app.viewports[0].screen_rect().unwrap().contains(apex));
        let preview = viewport_frame(app, context, vec![egui::Event::PointerMoved(apex)]);
        if screen_point(app, target) == apex {
            return (apex, preview);
        }
    }
    panic!("viewport did not settle while hovering at {target:?}");
}

#[test]
fn surface_apex_boundary_selection_and_failed_apex_keep_model_selection_private() {
    let mut app = test_app();
    let source = planar_apex_source(&mut app, 0.);
    let boundary = boundary_surface(&mut app, 3.);
    app.document.clear_history().unwrap();
    let before = format!("{:?}", app.document);
    enter(&mut app, "ExtrudeSrfToPoint");
    enter(&mut app, "ToBoundary");
    assert!(app.picking_surface_apex_boundary());
    enter(&mut app, &format!("_SelID {boundary}"));
    assert!(app.command_hint().contains("beyond the boundary"));
    enter(&mut app, "w2,1.5,1");
    assert!(app.active_command.is_some());
    assert!(app.command_hint().contains("beyond the boundary"));
    assert_eq!(format!("{:?}", app.document), before);
    assert!(app.document.is_selected(source));
    assert!(!app.document.can_undo());
    app.cancel_current_prompt_or_selection();
    assert!(app.active_command.is_none());
}

#[test]
fn surface_apex_boundary_menu_enter_cancels_without_a_surface_pick() {
    let mut app = test_app();
    planar_apex_source(&mut app, 0.);
    let before = format!("{:?}", app.document);
    enter(&mut app, "ExtrudeSrfToPoint ToBoundary");
    assert!(app.picking_surface_apex_boundary());
    enter(&mut app, "");
    assert!(app.active_command.is_none());
    assert_eq!(format!("{:?}", app.document), before);
}
