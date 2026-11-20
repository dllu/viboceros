use super::circle_preview::{screen_point, viewport_click, viewport_frame};
use super::curve_fillet::enter;
use super::curve_match::click_world;
use super::*;
use viboceros_geometry::{Brep, NurbsSurface};

fn surface(app: &VibocerosApp, id: ObjectId) -> &NurbsSurface {
    match app.document.object(id).unwrap().geometry() {
        Geometry::NurbsSurface(surface) => surface,
        Geometry::Brep(brep) if brep.faces().len() == 1 => brep.faces()[0].surface(),
        _ => panic!("expected a single surface"),
    }
}

fn torus(app: &mut VibocerosApp) -> ObjectId {
    app.execute_command("Torus 0,0,0 6 2");
    let id = app.document.objects().next().unwrap().id();
    app.document
        .select_object(id, SelectionMode::Replace)
        .unwrap();
    app.document.clear_history().unwrap();
    id
}

fn pick_of(surface: &NurbsSurface) -> (f64, f64, Point3) {
    let u = surface.parameter_at_u(0.19).unwrap();
    let (v, pick) = [0.26, 0.74]
        .into_iter()
        .map(|parameter| {
            let v = surface.parameter_at_v(parameter).unwrap();
            (v, surface.evaluate(u, v).unwrap())
        })
        .max_by(|a, b| a.1.z().total_cmp(&b.1.z()))
        .unwrap();
    (u, v, pick)
}

#[test]
fn both_surface_seams_follow_the_viewport_cursor_before_one_atomic_pick() {
    let mut app = test_app();
    let id = torus(&mut app);
    let before = app.document.object(id).unwrap().geometry_snapshot().clone();
    let source = surface(&app, id).clone();
    let (_, v, pick) = pick_of(&source);
    app.viewports[0] = Viewport::new(ViewKind::Top);
    app.maximized_viewport = Some(0);
    app.document.clear_selection();
    enter(&mut app, "SrfSeam");
    assert!(app.surface_seam_session.as_ref().unwrap().selecting());
    let context = egui::Context::default();
    click_world(
        &mut app,
        &context,
        source.evaluate(*source.domain_u().start(), v).unwrap(),
    );
    assert!(
        app.surface_seam_session
            .as_ref()
            .unwrap()
            .overlay()
            .is_some(),
        "{:?}",
        app.command_log,
    );
    for _ in 0..3 {
        viewport_frame(&mut app, &context, vec![]);
    }
    click_world(&mut app, &context, point(0., 0., 0.));
    assert!(
        app.surface_seam_session
            .as_ref()
            .unwrap()
            .overlay()
            .unwrap()
            .preview
            .is_none()
    );
    let pixel = screen_point(&app, pick);
    viewport_frame(&mut app, &context, vec![egui::Event::PointerMoved(pixel)]);
    let overlay = app
        .surface_seam_session
        .as_ref()
        .unwrap()
        .overlay()
        .unwrap();
    assert!(overlay.original.iter().all(Option::is_some));
    let preview = overlay.preview.expect("top-view surface cursor");
    assert!(preview.curves.iter().all(Option::is_some));
    assert!(preview.point.distance_to(pick).unwrap() < 1e-5);
    let expected = preview.point;
    assert!(before.shares_storage_with(app.document.object(id).unwrap().geometry_snapshot()));
    assert!(!app.document.can_undo());

    viewport_click(&mut app, &context, pixel);
    assert!(app.surface_seam_session.is_none(), "{:?}", app.command_log);
    let output = surface(&app, id);
    assert_eq!(output.domain_u(), source.domain_u());
    assert_eq!(output.domain_v(), source.domain_v());
    let corner = output
        .evaluate(*output.domain_u().start(), *output.domain_v().start())
        .unwrap();
    assert!(corner.distance_to(expected).unwrap() < 1e-8);
    assert!(!app.document.is_selected(id));
    assert_eq!(app.document.undo_label(), Some("SrfSeam"));
    app.document.undo().unwrap();
    assert!(before.shares_storage_with(app.document.object(id).unwrap().geometry_snapshot()));
}

#[test]
fn surface_seam_direction_names_the_curve_and_toggle_and_cancel_keep_preferences() {
    let mut app = test_app();
    let id = torus(&mut app);
    let source = surface(&app, id).clone();
    let (u, _, pick) = pick_of(&source);

    enter(&mut app, "SrfSeam");
    enter(&mut app, "Direction");
    assert!(
        app.surface_seam_session
            .as_ref()
            .unwrap()
            .hint()
            .contains("Direction <Both>")
    );
    enter(&mut app, "_U");
    let overlay = app
        .surface_seam_session
        .as_ref()
        .unwrap()
        .overlay()
        .unwrap();
    assert!(overlay.original[0].is_some() && overlay.original[1].is_none());
    enter(&mut app, "Toggle");
    let overlay = app
        .surface_seam_session
        .as_ref()
        .unwrap()
        .overlay()
        .unwrap();
    assert!(overlay.original[0].is_none() && overlay.original[1].is_some());
    enter(
        &mut app,
        &format!("w{},{},{}", pick.x(), pick.y(), pick.z()),
    );

    let output = surface(&app, id);
    assert_eq!(output.domain_u(), source.domain_u());
    assert_eq!(output.domain_v(), source.domain_v());
    let [u0, v0] = [*source.domain_u().start(), *source.domain_v().start()];
    let expected = source.evaluate(u, v0).unwrap();
    assert!(
        output
            .evaluate(u0, v0)
            .unwrap()
            .distance_to(expected)
            .unwrap()
            < 1e-8
    );

    enter(&mut app, "SrfSeam");
    assert!(
        app.surface_seam_session
            .as_ref()
            .unwrap()
            .hint()
            .contains("Direction=V")
    );
    enter(&mut app, "Direction=Both");
    enter(&mut app, "Cancel");
    assert_eq!(app.document.undo_label(), Some("SrfSeam"));
    enter(&mut app, "SrfSeam");
    assert!(
        app.surface_seam_session
            .as_ref()
            .unwrap()
            .hint()
            .contains("Direction=Both")
    );
}

#[test]
fn surface_seam_postselects_a_natural_brep_face_and_preserves_its_orientation() {
    let mut app = test_app();
    let source =
        NurbsSurface::try_cylinder(app.viewports[0].construction_plane(), 3., 0., 7.).unwrap();
    let tol = app.document.tolerance();
    let brep = Brep::try_rectangular_surface_face_with_orientation(
        source.clone(),
        source.domain_u(),
        source.domain_v(),
        true,
        tol,
    )
    .unwrap();
    let id = app.document.add_geometry(Geometry::Brep(brep)).unwrap();
    let Geometry::Brep(input) = app.document.object(id).unwrap().geometry() else {
        unreachable!()
    };
    assert!(input.faces()[0].is_reversed());
    app.document.clear_history().unwrap();

    enter(&mut app, "SrfSeam");
    assert!(app.surface_seam_session.as_ref().unwrap().selecting());
    enter(&mut app, &format!("SelID {id}"));
    let session = app.surface_seam_session.as_ref().unwrap();
    assert_eq!(session.overlay().unwrap().source.id(), id);
    assert_eq!(session.hint(), "Adjust surface seam");
    let u = source.parameter_at_u(0.28).unwrap();
    let v = source.parameter_at_v(0.43).unwrap();
    enter(&mut app, &format!("Parameter={u}"));
    let Geometry::Brep(output) = app.document.object(id).unwrap().geometry() else {
        panic!("SrfSeam must retain the single-face B-rep");
    };
    let face = &output.faces()[0];
    assert!(face.is_reversed());
    assert!(face.is_untrimmed(tol).unwrap());
    let result = face.surface();
    assert_eq!(result.domain_u(), source.domain_u());
    assert_eq!(result.domain_v(), source.domain_v());
    assert!(
        result
            .evaluate(*result.domain_u().start(), v)
            .unwrap()
            .distance_to(source.evaluate(u, v).unwrap())
            .unwrap()
            < 1e-8
    );
    assert!(!app.document.is_selected(id));
    assert_eq!(app.document.undo_label(), Some("SrfSeam"));
    assert!(!app.try_start_interactive_command("SrfSeam Parameter=1 Direction=U"));
    assert!(!app.try_start_interactive_command("SrfSeam Direction=Sideways"));
}
