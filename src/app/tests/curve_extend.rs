use super::circle_preview::{screen_point, viewport_frame};
use super::curve_fillet::enter;
use super::curve_match::click_world;
use super::*;
use std::time::{Duration, Instant};
use viboceros_geometry::{CurveContinuityLevel, CurveRef, curve_end_continuity};

fn line(app: &mut VibocerosApp) -> ObjectId {
    enter(app, "Line -4,0,0 -1,0,0");
    app.document.objects().last().unwrap().id()
}

fn end(app: &VibocerosApp, id: ObjectId) -> Point3 {
    app.document
        .object(id)
        .unwrap()
        .geometry()
        .curve_ref()
        .unwrap()
        .end_point()
        .unwrap()
}

fn near(actual: Point3, reference: Point3, tolerance: f64) {
    assert!(
        actual.distance_to(reference).unwrap() <= tolerance,
        "{actual:?} != {reference:?}"
    );
}

fn top(app: &mut VibocerosApp) -> egui::Context {
    app.viewports[0] = Viewport::new(ViewKind::Top);
    app.maximized_viewport = Some(0);
    egui::Context::default()
}

#[test]
fn curve_extend_boundary_repeats_forward_with_local_and_grouped_undo() {
    let mut app = test_app();
    let source = line(&mut app);
    enter(&mut app, "Line 2,-4,0 2,4,0");
    enter(&mut app, "Line 4,-4,0 4,4,0");
    let boundaries = app.document.objects().skip(1).cloned().collect::<Vec<_>>();
    app.document.clear_history().unwrap();
    let ctx = top(&mut app);
    enter(&mut app, "Extend Type=Line");
    click_world(&mut app, &ctx, point(2., 2., 0.));
    enter(&mut app, &format!("SelID {}", boundaries[1].id()));
    assert_eq!(app.document.selected_object_count(), 2);
    enter(&mut app, "");
    click_world(&mut app, &ctx, point(-1.25, 0., 0.));
    near(end(&app, source), point(2., 0., 0.), 1e-8);
    click_world(&mut app, &ctx, point(1.75, 0., 0.));
    near(end(&app, source), point(4., 0., 0.), 1e-8);
    enter(&mut app, "Undo");
    near(end(&app, source), point(2., 0., 0.), 1e-8);
    click_world(&mut app, &ctx, point(1.75, 0., 0.));
    enter(&mut app, "");
    assert!(app.command_line_idle(), "{:?}", app.command_log);
    assert_eq!(app.document.undo_label(), Some("Extend"));
    for boundary in boundaries {
        assert_eq!(app.document.object(boundary.id()), Some(&boundary));
    }
    assert_eq!(app.document.selected_object_count(), 0);
    enter(&mut app, "Undo");
    near(end(&app, source), point(-1., 0., 0.), 1e-8);
    assert!(!app.document.can_undo());
    enter(&mut app, "Redo");
    near(end(&app, source), point(4., 0., 0.), 1e-8);
}

#[test]
fn curve_extend_dynamic_projects_and_previews_the_cursor_as_a_line_extension() {
    let mut app = test_app();
    let source = line(&mut app);
    app.document.clear_history().unwrap();
    let ctx = top(&mut app);
    enter(&mut app, "ExtendDynamic Type=Line");
    click_world(&mut app, &ctx, point(-1.25, 0., 0.));
    assert!(app.curve_extend_session.as_ref().unwrap().wants_point());
    let destination = point(2., 1., 0.);
    let deadline = Instant::now() + Duration::from_secs(5);
    loop {
        let pixel = screen_point(&app, destination);
        viewport_frame(&mut app, &ctx, vec![egui::Event::PointerMoved(pixel)]);
        if let Some(curves) = app.curve_extend_session.as_ref().and_then(|s| s.preview()) {
            let curve = &curves[0];
            if curve
                .evaluate(*curve.domain().end())
                .unwrap()
                .distance_to(point(2., 0., 0.))
                .unwrap()
                < 1e-5
            {
                break;
            }
        }
        assert!(
            Instant::now() < deadline,
            "the projected Extend draft did not arrive: {:?}",
            app.command_log
        );
        std::thread::sleep(Duration::from_millis(5));
    }
    assert!(!app.document.can_undo());
    near(end(&app, source), point(-1., 0., 0.), 1e-9);
    click_world(&mut app, &ctx, destination);
    near(end(&app, source), point(2., 0., 0.), 1e-5);
    enter(&mut app, "");
    assert_eq!(app.document.undo_label(), Some("ExtendDynamic"));
}

#[test]
fn curve_extend_fixed_length_and_shortening_accept_typed_ends_and_preserve_segment_boundaries() {
    let mut app = test_app();
    let source = line(&mut app);
    app.document.clear_history().unwrap();
    enter(&mut app, "Extend");
    enter(&mut app, "Type");
    enter(&mut app, "_Line");
    enter(&mut app, "Join=Yes");
    enter(&mut app, "2");
    enter(&mut app, &format!("SelID {source}"));
    assert!(!app.document.can_undo());
    enter(&mut app, "End");
    let Geometry::Polyline(curve) = app.document.object(source).unwrap().geometry() else {
        panic!("Rhino retains a polyline segment boundary")
    };
    assert_eq!(curve.parameters(), &[0., 1., 2.]);
    assert_eq!(
        curve.vertices(),
        &[point(-4., 0., 0.), point(-1., 0., 0.), point(1., 0., 0.)]
    );
    enter(&mut app, "ExtensionLength=-1");
    enter(&mut app, &format!("SelID {source}"));
    enter(&mut app, "End");
    near(end(&app, source), point(0., 0., 0.), 1e-8);
    app.cancel_current_prompt_or_selection();
    assert!(app.command_line_idle());
    enter(&mut app, "Undo");
    near(end(&app, source), point(-1., 0., 0.), 1e-9);
    assert!(app.document.can_redo());
}

#[test]
fn curve_extend_arc_to_point_and_center_use_the_picked_end_tangent() {
    for instructions in [
        vec!["ToPoint", "w1,2,0"],
        vec!["Center", "w-1,2,0", "w1,2,0"],
    ] {
        let mut app = test_app();
        let source = line(&mut app);
        let original = app.document.object(source).unwrap().clone();
        app.document.clear_history().unwrap();
        enter(&mut app, "ExtendDynamic Type=Arc Join=No");
        enter(&mut app, &format!("SelID {source}"));
        enter(&mut app, "End");
        for token in instructions {
            enter(&mut app, token);
        }
        enter(&mut app, "");
        assert_eq!(app.document.object(source), Some(&original));
        let result = app
            .document
            .objects()
            .find(|object| object.id() != source)
            .unwrap_or_else(|| panic!("{:?}", app.command_log));
        let Geometry::PolyCurve(curve) = result.geometry() else {
            panic!("expected a separate arc extension")
        };
        assert_eq!(curve.segments().len(), 1);
        near(end(&app, result.id()), point(1., 2., 0.), 1e-8);
        assert!((*curve.domain().end() - (3. + std::f64::consts::PI)).abs() < 1e-9);
    }
}

#[test]
fn curve_extend_smooth_to_point_preserves_endpoint_curvature() {
    let mut app = test_app();
    let source = viboceros_geometry::NurbsCurve::try_new(
        3,
        vec![
            point(-4., -1.5, 0.),
            point(-3., -1., 0.),
            point(-2., 0.1, 0.),
            point(-1., 0., 0.),
        ],
        vec![0., 0., 0., 0., 1., 1., 1., 1.],
    )
    .unwrap();
    let id = app
        .document
        .add_geometry(Geometry::NurbsCurve(source.clone()))
        .unwrap();
    app.document.clear_history().unwrap();
    for token in [
        "ExtendDynamic Type=Smooth Join=No",
        &format!("SelID {id}"),
        "End",
        "ToPoint",
        "w3,2,0",
        "",
    ] {
        enter(&mut app, token);
    }
    let piece = app
        .document
        .objects()
        .find(|object| object.id() != id)
        .unwrap_or_else(|| panic!("{:?}", app.command_log));
    near(end(&app, piece.id()), point(3., 2., 0.), 1e-9);
    let Geometry::NurbsCurve(piece_curve) = piece.geometry() else {
        panic!("expected cubic extension")
    };
    let native_end = 2.644777111701541;
    assert!((*piece_curve.domain().end() - native_end).abs() < 1e-10);
    near(
        piece_curve.control_points()[2].point(),
        point(2.2895542234030817, -3.5753055189530256, 0.),
        1e-9,
    );
    assert_eq!(
        curve_end_continuity(
            CurveRef::NurbsCurve(&source),
            true,
            piece.geometry().curve_ref().unwrap(),
            false,
            app.document.tolerance()
        )
        .unwrap()
        .level,
        CurveContinuityLevel::CurvatureOrHigher
    );
    enter(&mut app, "Undo");
    for (point, native_end) in [
        ("w3,-.4,0", 2.333333333333334),
        ("w3,0,0", 2.3478181826566917),
        ("w3,-2,0", 2.5956887664426596),
        ("w1,1,0", 1.8223885558507704),
        ("w-1,2,0", 1.6739090913283459),
    ] {
        for token in [
            "ExtendDynamic Type=Smooth Join=No",
            &format!("SelID {id}"),
            "End",
            "ToPoint",
            point,
            "",
        ] {
            enter(&mut app, token);
        }
        let piece = app
            .document
            .objects()
            .find(|object| object.id() != id)
            .unwrap();
        let domain = piece.geometry().curve_ref().unwrap().domain();
        assert!(
            (*domain.end() - native_end).abs() < 1e-10,
            "{point}: {domain:?}"
        );
        enter(&mut app, "Undo");
    }
    for token in [
        "ExtendDynamic Type=Smooth Join=Merge",
        &format!("SelID {id}"),
        "End",
        "ToPoint",
        "w3,2,0",
        "",
    ] {
        enter(&mut app, token);
    }
    let Geometry::NurbsCurve(merged) = app.document.object(id).unwrap().geometry() else {
        panic!("expected one merged NURBS curve")
    };
    assert_eq!(merged.control_points().len(), 5);
    assert_eq!(merged.knots().iter().filter(|&&k| k == 1.).count(), 1);
    assert!((*merged.domain().end() - native_end).abs() < 1e-10);
    near(
        merged.control_points()[2].point(),
        point(-0.35522288829845894, 1.9092548228716955, 0.),
        1e-8,
    );
}

#[test]
fn curve_extend_surface_edge_creates_a_curve_and_keeps_its_parent() {
    let mut app = test_app();
    let surface = viboceros_geometry::NurbsSurface::try_bilinear([
        point(-4., 0., 0.),
        point(-1., 0., 0.),
        point(-1., 2., 0.),
        point(-4., 2., 0.),
    ])
    .unwrap();
    let brep =
        viboceros_geometry::Brep::try_surface_face(surface, app.document.tolerance()).unwrap();
    let owner = app.document.add_geometry(Geometry::Brep(brep)).unwrap();
    let original = app.document.object(owner).unwrap().clone();
    app.document.clear_history().unwrap();
    let ctx = top(&mut app);
    enter(&mut app, "ExtendDynamic Type=Line");
    click_world(&mut app, &ctx, point(-1.4, 0., 0.));
    assert!(app.curve_extend_session.as_ref().unwrap().wants_point());
    enter(&mut app, "2");
    enter(&mut app, "");
    assert_eq!(app.document.object(owner), Some(&original));
    let curve = app
        .document
        .objects()
        .find(|o| o.id() != owner)
        .unwrap_or_else(|| panic!("{:?}", app.command_log))
        .geometry()
        .curve_ref()
        .unwrap();
    let points = [curve.start_point().unwrap(), curve.end_point().unwrap()];
    assert!(
        points
            .iter()
            .any(|p| p.distance_to(point(1., 0., 0.)).unwrap() < 1e-8),
        "{points:?}"
    );
}
