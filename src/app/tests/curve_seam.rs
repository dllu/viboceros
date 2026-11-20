use super::circle_preview::{screen_point, viewport_frame};
use super::curve_fillet::enter;
use super::curve_match::click_world;
use super::*;
use viboceros_geometry::{Circle3, CurveRef, NurbsCurve, UnitVector3};

fn circle(app: &mut VibocerosApp, x: f64, radius: f64) -> ObjectId {
    app.execute_command(&format!("Circle {x},0 {radius}"));
    app.document.objects().last().unwrap().id()
}

fn curve(app: &VibocerosApp, id: ObjectId) -> CurveRef<'_> {
    app.document
        .object(id)
        .unwrap()
        .geometry()
        .curve_ref()
        .unwrap()
}

fn seam(app: &VibocerosApp, id: ObjectId) -> Point3 {
    let curve = curve(app, id);
    curve.evaluate(*curve.domain().start()).unwrap()
}

fn marker(app: &VibocerosApp, index: usize) -> Point3 {
    app.curve_seam_session.as_ref().unwrap().editor.markers()[index].point
}

fn pointer(pos: egui::Pos2, pressed: bool) -> egui::Event {
    egui::Event::PointerButton {
        pos,
        button: egui::PointerButton::Primary,
        pressed,
        modifiers: egui::Modifiers::NONE,
    }
}

#[test]
fn drags_a_seam_on_its_curve_with_live_preview_then_flips_the_direction() {
    let mut app = test_app();
    let id = circle(&mut app, 0., 5.);
    let original = app.document.object(id).unwrap().geometry_snapshot().clone();
    app.document.clear_history().unwrap();
    app.document
        .select_object(id, SelectionMode::Replace)
        .unwrap();
    app.viewports[0] = Viewport::new(ViewKind::Top);
    app.maximized_viewport = Some(0);
    enter(&mut app, "CrvSeam");
    let context = egui::Context::default();
    for _ in 0..3 {
        viewport_frame(&mut app, &context, vec![]);
    }
    let start = screen_point(&app, point(5., 0., 0.));
    let off_curve = screen_point(&app, point(0., 7., 0.));
    viewport_frame(
        &mut app,
        &context,
        vec![egui::Event::PointerMoved(start), pointer(start, true)],
    );
    viewport_frame(
        &mut app,
        &context,
        vec![egui::Event::PointerMoved(off_curve)],
    );
    let target = point(0., 5., 0.);
    assert!(marker(&app, 0).distance_to(target).unwrap() < 1e-5);
    assert!(original.shares_storage_with(app.document.object(id).unwrap().geometry_snapshot()));
    assert!(!app.document.can_undo());

    viewport_frame(&mut app, &context, vec![pointer(off_curve, false)]);
    enter(&mut app, "Flip");
    click_world(&mut app, &context, target);
    assert!(
        app.curve_seam_session
            .as_ref()
            .unwrap()
            .hint()
            .contains("Drag seam")
    );
    enter(&mut app, "");

    assert!(matches!(curve(&app, id), CurveRef::Circle(_)));
    let result = curve(&app, id);
    let sample = result
        .evaluate_with_tangent(*result.domain().start())
        .unwrap();
    assert!(sample.point().distance_to(target).unwrap() < 1e-5);
    assert!(sample.tangent().as_vector().x() > 0.999999);
    assert!(app.document.is_selected(id));
    assert_eq!(app.document.undo_label(), Some("CrvSeam"));
    app.document.undo().unwrap();
    assert!(original.shares_storage_with(app.document.object(id).unwrap().geometry_snapshot()));
    app.document.redo().unwrap();
    assert!(seam(&app, id).distance_to(target).unwrap() < 1e-5);

    let endpoint = *curve(&app, id).domain().end();
    enter(&mut app, "CrvSeam");
    enter(&mut app, &format!("Parameter={endpoint}"));
    click_world(&mut app, &context, target);
    enter(&mut app, "");
    assert_eq!(*curve(&app, id).domain().start(), endpoint);
    assert!(seam(&app, id).distance_to(target).unwrap() < 1e-5);
}

#[test]
fn postselects_only_closed_curves_and_restores_natural_or_automatic_seams() {
    let mut app = test_app();
    let first = circle(&mut app, 0., 2.);
    let second = circle(&mut app, 6., 2.);
    app.execute_command("Line -2,-5 2,-5");
    app.document.clear_history().unwrap();
    app.viewports[0] = Viewport::new(ViewKind::Top);
    app.maximized_viewport = Some(0);
    let context = egui::Context::default();

    enter(&mut app, "CrvSeam");
    assert!(app.selecting_curve_seams());
    click_world(&mut app, &context, point(0., -5., 0.));
    assert_eq!(app.document.selected_object_count(), 0);
    click_world(&mut app, &context, point(2., 0., 0.));
    enter(&mut app, &format!("SelID {second}"));
    assert_eq!(
        app.document.selected_object_ids().collect::<Vec<_>>(),
        [first, second]
    );
    enter(&mut app, "");
    let aligned = point(4., 0., 0.);
    assert!(marker(&app, 1).distance_to(aligned).unwrap() < 1e-12);
    assert_eq!(seam(&app, second), point(8., 0., 0.));

    enter(&mut app, "Natural");
    assert_eq!(marker(&app, 1), seam(&app, second));
    enter(&mut app, "Automatic");
    assert!(marker(&app, 1).distance_to(aligned).unwrap() < 1e-12);
    enter(&mut app, &format!("SelID {first}"));
    enter(&mut app, "w0,2,0");
    assert!(marker(&app, 0).distance_to(point(0., 2., 0.)).unwrap() < 1e-12);
    enter(&mut app, "");
    assert_eq!(app.document.selected_object_count(), 0);
    assert_eq!(app.document.undo_label(), Some("CrvSeam"));
    assert!(seam(&app, second).distance_to(aligned).unwrap() < 1e-12);
    assert!(seam(&app, first).distance_to(point(0., 2., 0.)).unwrap() < 1e-12);

    app.document.undo().unwrap();
    assert_eq!(seam(&app, first), point(2., 0., 0.));
    assert_eq!(seam(&app, second), point(8., 0., 0.));
    assert!(!app.document.can_undo());
}

#[test]
fn snap_to_knots_changes_the_drag_and_is_remembered_after_cancel() {
    let mut app = test_app();
    let knots = vec![0., 0., 1., 2., 3., 4., 4.];
    let points = [(0., 0.), (4., 0.), (4., 4.), (0., 4.), (0., 0.)].map(|(x, y)| point(x, y, 0.));
    let id = app
        .document
        .add_geometry(Geometry::NurbsCurve(
            NurbsCurve::try_new(1, points.to_vec(), knots).unwrap(),
        ))
        .unwrap();
    app.document
        .select_object(id, SelectionMode::Replace)
        .unwrap();
    app.viewports[0] = Viewport::new(ViewKind::Top);
    app.maximized_viewport = Some(0);
    app.document.clear_history().unwrap();
    let context = egui::Context::default();
    let start = point(0., 0., 0.);
    let destination = point(3., 0., 0.);

    enter(&mut app, "CrvSeam");
    let drag = |app: &mut VibocerosApp| {
        for _ in 0..3 {
            viewport_frame(app, &context, vec![]);
        }
        let from = screen_point(app, start);
        let to = screen_point(app, destination);
        viewport_frame(
            app,
            &context,
            vec![egui::Event::PointerMoved(from), pointer(from, true)],
        );
        viewport_frame(app, &context, vec![egui::Event::PointerMoved(to)]);
        viewport_frame(app, &context, vec![pointer(to, false)]);
    };
    drag(&mut app);
    assert_eq!(marker(&app, 0), point(4., 0., 0.));
    enter(&mut app, "Natural");
    enter(&mut app, "SnapToKnots=No");
    drag(&mut app);
    assert!(marker(&app, 0).distance_to(destination).unwrap() < 1e-5);
    enter(&mut app, "Cancel");
    assert_eq!(seam(&app, id), start);
    assert!(!app.document.can_undo());
    assert!(app.document.is_selected(id));

    enter(&mut app, "CrvSeam");
    assert!(
        app.curve_seam_session
            .as_ref()
            .unwrap()
            .hint()
            .contains("SnapToKnots=No")
    );
    enter(&mut app, "");
    assert!(!app.document.can_undo());
    assert!(!app.try_start_interactive_command("CrvSeam Parameter=0.5"));
}

#[test]
fn automatic_uses_the_current_traversal_and_the_first_seam_as_anchor() {
    let mut app = test_app();
    let first = circle(&mut app, 0., 2.);
    let tolerance = app.document.tolerance();
    let mut add = |center, x_axis: UnitVector3, normal: UnitVector3| {
        app.document
            .add_geometry(Geometry::Circle(
                Circle3::try_from_frame(center, 2., x_axis, normal, tolerance).unwrap(),
            ))
            .unwrap()
    };
    let axis = |x, y, z| UnitVector3::try_new(x, y, z, tolerance).unwrap();
    let second = add(point(6., 0., 0.), axis(0., 1., 0.), axis(0., 0., -1.));
    let third = add(point(12., 1., 0.), axis(1., 0., 0.), axis(0., 0., 1.));
    app.restore_curve_selection(vec![first, second, third]);
    app.document.clear_history().unwrap();

    enter(&mut app, "CrvSeam");
    assert!(marker(&app, 2).y() < 1.);
    enter(&mut app, "Automatic");
    let y = 1. + 2. / 65_f64.sqrt();
    assert!((marker(&app, 2).y() - y).abs() < 1e-12);
    enter(&mut app, "");
    assert!((seam(&app, third).y() - y).abs() < 1e-12);
    let end = 2. * (std::f64::consts::PI + 0.125_f64.atan());
    assert!((*curve(&app, third).domain().end() - end).abs() < 1e-12);

    app.document.undo().unwrap();
    enter(&mut app, "CrvSeam");
    enter(&mut app, "Flip");
    enter(&mut app, &format!("SelID {first}"));
    enter(&mut app, "Automatic");
    enter(&mut app, "");
    for id in [first, second] {
        let curve = curve(&app, id);
        let tangent = curve
            .evaluate_with_tangent(*curve.domain().start())
            .unwrap();
        assert!(tangent.tangent().as_vector().y() < -0.999999);
    }
}

#[test]
fn automatic_alignment_respects_nurbs_knot_snapping() {
    let mut app = test_app();
    let first = circle(&mut app, 0., 2.);
    let points =
        [(3., -1.), (5., -1.), (5., 2.), (3., 2.), (3., -1.)].map(|(x, y)| point(x, y, 0.));
    let knots = vec![0., 0., 1., 2., 3., 4., 4.];
    let second = app
        .document
        .add_geometry(Geometry::NurbsCurve(
            NurbsCurve::try_new(1, points.to_vec(), knots).unwrap(),
        ))
        .unwrap();
    app.restore_curve_selection(vec![first, second]);

    enter(&mut app, "CrvSeam");
    assert_eq!(marker(&app, 1), point(3., -1., 0.));
    enter(&mut app, "SnapToKnots=No");
    enter(&mut app, "Natural");
    enter(&mut app, "Automatic");
    assert!(marker(&app, 1).distance_to(point(3., 0., 0.)).unwrap() < 1e-12);
}
