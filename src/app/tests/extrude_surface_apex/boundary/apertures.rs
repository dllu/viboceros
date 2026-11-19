use super::super::super::circle_preview::{screen_point, viewport_click, viewport_frame};
use super::*;
use viboceros_geometry::{BoundingBox3, Brep, Circle3, NurbsSurface, Polyline3, UnitVector3};

#[test]
fn surface_apex_previews_shallow_cutter_holes_and_retries_a_crossing() {
    let tolerance = Tolerance::try_new(1e-6, 1e-12, 1e-8).unwrap();
    let mut app = test_app();
    app.document = Document::new(tolerance);
    app.viewports[0] = Viewport::new(ViewKind::Right);
    app.maximized_viewport = Some(0);
    app.osnap = false;
    app.grid_snap = false;
    let height = 3e-5;
    let plane = app.viewports[0]
        .construction_plane()
        .with_origin(point(6., 0., 0.));
    app.viewports[0].set_construction_plane(plane);
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
    let cut_z = height * 7. / 12.;
    let outer = Polyline3::try_new(
        vec![
            point(4., -4., -4.),
            point(4., 4., -4.),
            point(4., 4., 4.),
            point(4., -4., 4.),
            point(4., -4., -4.),
        ],
        tolerance,
    )
    .unwrap()
    .to_nurbs()
    .unwrap();
    let center = point(4., 1.5, cut_z);
    let radius = 1.5e-6;
    let hole = Circle3::try_new(
        center,
        radius,
        UnitVector3::try_new(1., 0., 0., tolerance).unwrap(),
        tolerance,
    )
    .unwrap()
    .to_nurbs()
    .unwrap();
    let cutter = Brep::try_planar_face_with_holes(&outer, &[hole], tolerance).unwrap();
    let boundary = app.document.add_geometry(Geometry::Brep(cutter)).unwrap();
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
    app.viewports[0]
        .zoom_bounding_box(
            BoundingBox3::from_points([point(0., 1.5 - 5e-5, -3e-5), point(6.5, 1.5 + 5e-5, 8e-5)])
                .unwrap(),
        )
        .unwrap();
    viewport_frame(&mut app, &context, Vec::new());

    let shows_hole = |app: &VibocerosApp, strokes: &[[egui::Pos2; 2]]| {
        let c = screen_point(app, center);
        let r = c.distance(screen_point(app, point(4., 1.5 + radius, cut_z)));
        strokes
            .iter()
            .filter(|line| line.iter().all(|p| (p.distance(c) - r).abs() < 0.1))
            .count()
            >= 8
    };
    let (_, outside) = boundary_hover(&mut app, &context, point(6., 1.5, 4.5e-5));
    let hover = app
        .surface_extrusion_sources
        .as_ref()
        .and_then(|sources| sources.apex_draft(&app.document))
        .and_then(|draft| draft.hover);
    assert!(
        !outside.is_empty(),
        "hover {hover:?}, log {:?}",
        app.command_log
    );
    assert!(!shows_hole(&app, &outside));

    let (crossing, preview) = boundary_hover(&mut app, &context, point(6., 1.5, 2. * cut_z));
    assert!(preview.is_empty());
    viewport_click(&mut app, &context, crossing);
    assert!(app.active_command.is_some());
    assert!(app.command_log.back().unwrap().starts_with("Error:"));
    assert_eq!(format!("{:?}", app.document), before);

    // Let the longer command log settle before picking at a tight zoom.
    viewport_frame(&mut app, &context, Vec::new());
    viewport_frame(&mut app, &context, Vec::new());
    let (inside, preview) = boundary_hover(&mut app, &context, point(6., 1.5, height));
    assert!(
        shows_hole(&app, &preview),
        "aperture missing from {} strokes",
        preview.len()
    );
    viewport_click(&mut app, &context, inside);
    {
        assert!(app.active_command.is_none(), "{:?}", app.command_log);
        let Geometry::Brep(cut) = app.document.selected_objects().next().unwrap().geometry() else {
            panic!("expected a truncated sweep");
        };
        assert_eq!([cut.faces().len(), cut.edges().len()], [6, 13]);
        assert!(cut.faces().iter().any(|face| face.loops().len() == 2));
        assert!(app.document.object(source).is_none());
        enter(&mut app, "Undo");
    }
    assert!(app.document.object(source).is_some());
    assert!(app.document.object(boundary).is_some());
}
