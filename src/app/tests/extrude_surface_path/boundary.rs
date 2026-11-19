use super::point_picks::{quadratic, scene};
use super::*;
use viboceros_geometry::NurbsSurface;

fn cutter(app: &mut VibocerosApp, height: f64) -> ObjectId {
    let surface = NurbsSurface::try_bilinear([
        point(-5., -12., height),
        point(14., -12., height),
        point(14., 10., height),
        point(-5., 10., height),
    ])
    .unwrap();
    app.document
        .add_geometry(Geometry::NurbsSurface(surface))
        .unwrap()
}

#[test]
fn surface_path_boundary_preserves_the_deferred_path_during_source_selection() {
    let (mut app, source, rail) = scene(quadratic());
    let target = cutter(&mut app, 4.);
    app.document.clear_selection();
    app.document.clear_history().unwrap();
    enter(
        &mut app,
        &format!("ExtrudeSrfAlongCrv Path={rail} ToBoundary SubCurve=Yes"),
    );
    assert!(app.object_prompt.is_some());
    app.apply_selection_click(SelectionClick {
        object_id: Some(source),
        mode: SelectionMode::Replace,
    });
    enter(&mut app, "");
    assert!(app.picking_surface_path_boundary());
    enter(&mut app, &format!("_SelID {target}"));
    let session = app.surface_path_prompt.as_ref().unwrap();
    assert!(session.hint().starts_with("Starting"));
    assert_eq!(session.selected_rail(&app.document), Some(rail));
    assert_eq!(app.document.objects().count(), 3);
    assert!(!app.document.can_undo());
}

#[test]
fn surface_path_boundary_picker_releases_changed_face_and_cancels_cleanly() {
    let (mut app, source, rail) = scene(quadratic());
    let target = cutter(&mut app, 4.);
    let wrong = app
        .document
        .add_geometry(Geometry::Point(point(-3., 2., 2.)))
        .unwrap();
    app.document.clear_history().unwrap();
    enter(&mut app, "ExtrudeSrfAlongCrv _ToBoundary");
    assert_eq!(
        app.surface_path_prompt.as_ref().unwrap().hint(),
        "Select a boundary surface"
    );
    enter(&mut app, &format!("_SelID {wrong}"));
    assert!(app.picking_surface_path_boundary());
    enter(&mut app, &format!("_SelID {target}"));
    let hint = app.surface_path_prompt.as_ref().unwrap().hint();
    assert!(hint.starts_with("Select path"));
    assert!(!hint.contains("Solid"));
    enter(&mut app, &format!("SubCurve=Yes Path={rail}"));
    enter(&mut app, "w8.112,-2.8,0.734");
    app.document
        .replace_object_geometries([(target, Geometry::Point(point(-3., 0., 4.)))])
        .unwrap();
    let before = format!("{:?}", app.document);
    enter(&mut app, "w11.312,-1.2,4.734");
    assert!(app.picking_surface_path_boundary());
    assert!(
        app.command_log
            .back()
            .unwrap()
            .contains("Boundary surface changed")
    );
    enter(&mut app, "_Cancel");
    assert!(app.surface_path_prompt.is_none());
    assert_eq!(format!("{:?}", app.document), before);
    assert!(app.document.is_selected(source));
}

#[test]
fn surface_path_boundary_viewport_picks_show_the_clipped_subcurve_preview() {
    use super::super::circle_preview::{screen_point, viewport_click, viewport_frame};
    use super::point_picks::{frame_rails, hover};
    use std::time::{Duration, Instant};

    let curve = quadratic();
    let (mut app, source, rail) = scene(curve.clone());
    let target = cutter(&mut app, 4.);
    app.document.clear_history().unwrap();
    let ctx = egui::Context::default();
    app.viewports[0] = Viewport::new(ViewKind::Top);
    enter(&mut app, "ExtrudeSrfAlongCrv _ToBoundary SubCurve=Yes");
    frame_rails(&mut app, &ctx);
    let (pointer, _) = hover(&mut app, &ctx, point(-1., 1., 4.));
    viewport_click(&mut app, &ctx, pointer);
    assert_eq!(
        app.surface_path_prompt
            .as_ref()
            .unwrap()
            .picked_boundary(&app.document)
            .unwrap()
            .object,
        target
    );

    app.viewports[0] = Viewport::new(ViewKind::Front);
    frame_rails(&mut app, &ctx);
    let (pointer, _) = hover(&mut app, &ctx, curve.evaluate(0.1).unwrap());
    viewport_click(&mut app, &ctx, pointer);
    assert!(
        app.surface_path_prompt
            .as_ref()
            .unwrap()
            .hint()
            .starts_with("Starting")
    );
    enter(&mut app, "w8.112,-2.8,0.734");
    let end = curve.evaluate(0.95).unwrap();
    hover(&mut app, &ctx, end);
    let deadline = Instant::now() + Duration::from_secs(5);
    let preview = loop {
        let lines = viewport_frame(&mut app, &ctx, vec![]);
        if !lines.is_empty() {
            break lines;
        }
        assert!(Instant::now() < deadline, "boundary preview timed out");
        std::thread::sleep(Duration::from_millis(10));
    };
    let cap_y = screen_point(&app, point(3., 0., 4.)).y;
    assert!(preview.iter().flatten().all(|p| p.y > cap_y - 0.05));
    assert!(
        preview
            .iter()
            .any(|[a, b]| (a.y - cap_y).abs() < 0.05 && (b.y - cap_y).abs() < 0.05)
    );
    assert_eq!(app.document.objects().count(), 3);
    assert!(!app.document.can_undo());

    let (_, before_cutter) = hover(&mut app, &ctx, curve.evaluate(0.25).unwrap());
    assert!(before_cutter.is_empty());
    let (pointer, _) = hover(&mut app, &ctx, end);
    viewport_click(&mut app, &ctx, pointer);
    assert!(app.surface_path_prompt.is_none(), "{:?}", app.command_log);
    let Geometry::Brep(result) = app
        .document
        .objects()
        .find(|o| ![source, rail, target].contains(&o.id()))
        .unwrap()
        .geometry()
    else {
        panic!("expected clipped result");
    };
    assert!(result.is_solid());
    let top = result
        .vertices()
        .iter()
        .map(|vertex| vertex.point().z())
        .reduce(f64::max)
        .unwrap();
    assert!((top - 4.).abs() < 1e-6, "{top}");
    enter(&mut app, "Undo");
    assert_eq!(app.document.objects().count(), 3);
    assert_eq!(app.document.selected_object_count(), 0);
    enter(&mut app, "Redo");
    assert_eq!(app.document.objects().count(), 4);
}
