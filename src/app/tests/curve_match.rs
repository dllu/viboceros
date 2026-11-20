use super::circle_preview::{screen_point, viewport_click, viewport_frame};
use super::curve_fillet::enter;
use super::*;
use viboceros_geometry::{CurveRef, NurbsCurve, ParameterSide};

fn cubic([a, b, c, d]: [[f64; 2]; 4]) -> NurbsCurve {
    NurbsCurve::try_new(
        3,
        [a, b, c, d].map(|[x, y]| point(x, y, 0.)).to_vec(),
        vec![0., 0., 0., 0., 1., 1., 1., 1.],
    )
    .unwrap()
}

fn setup(app: &mut VibocerosApp) -> (ObjectId, ObjectId, NurbsCurve) {
    let first = cubic([[-4., -1.], [-3., -0.3], [-2., 0.2], [-1., 0.]]);
    let second = cubic([[0.7, 1.], [1.5, 1.5], [3., 1.1], [4., 2.]]);
    let a = app
        .document
        .add_geometry(Geometry::NurbsCurve(first))
        .unwrap();
    let b = app
        .document
        .add_geometry(Geometry::NurbsCurve(second.clone()))
        .unwrap();
    app.document.clear_history().unwrap();
    app.viewports[0] = Viewport::new(ViewKind::Top);
    app.maximized_viewport = Some(0);
    (a, b, second)
}

fn curve(app: &VibocerosApp, id: ObjectId) -> CurveRef<'_> {
    app.document
        .object(id)
        .unwrap()
        .geometry()
        .curve_ref()
        .unwrap()
}

fn near(actual: Point3, expected: Point3, tolerance: f64) {
    assert!(
        actual.distance_to(expected).unwrap() < tolerance,
        "{actual:?} != {expected:?}"
    );
}

// The command panel can gain a row between picks. Project through the laid-out
// viewport after egui has settled its size and moved the pointer to the part.
pub(super) fn click_world(app: &mut VibocerosApp, context: &egui::Context, point: Point3) {
    for _ in 0..3 {
        viewport_frame(app, context, vec![]);
    }
    let pixel = screen_point(app, point);
    viewport_frame(app, context, vec![egui::Event::PointerMoved(pixel)]);
    let pixel = screen_point(app, point);
    viewport_click(app, context, pixel);
}

#[test]
fn ordered_viewport_match_previews_and_joins_without_changes_before_acceptance() {
    let mut app = test_app();
    let (first, second, target) = setup(&mut app);
    let source_before = app
        .document
        .object(first)
        .unwrap()
        .geometry_snapshot()
        .clone();
    let context = egui::Context::default();
    enter(&mut app, "SelAll");
    enter(&mut app, "Match");
    assert_eq!(app.document.selected_object_count(), 0);
    let source_point = curve(&app, first).evaluate(0.87).unwrap();
    click_world(&mut app, &context, source_point);
    assert_eq!(
        app.curve_match_first_id(),
        Some(first),
        "{:?}",
        app.command_log
    );
    let target_point = target.evaluate(0.10).unwrap();
    click_world(&mut app, &context, target_point);
    assert!(
        app.curve_match_session
            .as_ref()
            .and_then(|s| s.preview())
            .is_some(),
        "{:?}",
        app.command_log
    );
    enter(&mut app, "Mode");
    enter(&mut app, "_Position");
    enter(&mut app, "Join=Join");
    assert_eq!(
        app.curve_match_session.as_ref().unwrap().preview().unwrap()[0].domain(),
        0.0..=2.0
    );
    assert!(
        source_before.shares_storage_with(app.document.object(first).unwrap().geometry_snapshot())
    );
    assert!(!app.document.can_undo());
    assert_eq!(app.document.objects().len(), 2);
    enter(&mut app, "");
    assert!(app.command_line_idle(), "{:?}", app.command_log);
    let Geometry::PolyCurve(joined) = app.document.object(first).unwrap().geometry() else {
        panic!("expected a Match polycurve")
    };
    assert!(app.document.object(second).is_none());
    assert_eq!(joined.segments().len(), 2);
    near(joined.evaluate(1.).unwrap(), point(0.7, 1., 0.), 1e-9);
    assert_eq!(app.document.selected_object_count(), 0);
    enter(&mut app, "Undo");
    assert!(app.document.object(second).is_some());
    assert!(
        source_before.shares_storage_with(app.document.object(first).unwrap().geometry_snapshot())
    );
    assert!(!app.document.can_undo());
    assert!(app.document.can_redo());
}

#[test]
fn pick_point_matches_an_interior_target_and_leaves_its_curve_intact() {
    let mut app = test_app();
    let (first, second, target) = setup(&mut app);
    let input = app
        .document
        .object(second)
        .unwrap()
        .geometry_snapshot()
        .clone();
    let context = egui::Context::default();
    enter(&mut app, "Match");
    let source_point = curve(&app, first).evaluate(0.87).unwrap();
    click_world(&mut app, &context, source_point);
    enter(&mut app, "MatchTo=PickPoint");
    let target_point = target.evaluate(0.47).unwrap();
    click_world(&mut app, &context, target_point);
    enter(&mut app, "Mode=Tangency");
    let preview = &app
        .curve_match_session
        .as_ref()
        .unwrap()
        .preview()
        .expect("point preview")[0];
    near(preview.evaluate(1.).unwrap(), target_point, 2e-5);
    assert!(!app.document.can_undo());
    enter(&mut app, "Accept");
    near(curve(&app, first).end_point().unwrap(), target_point, 2e-5);
    assert!(input.shares_storage_with(app.document.object(second).unwrap().geometry_snapshot()));
}

#[test]
fn edge_contact_uses_a_typed_point_and_previews_perpendicular_tangency() {
    let mut app = test_app();
    let source = cubic([[-4., -1.], [-3., -0.3], [-2., 0.2], [-1., 0.]]);
    let first = app
        .document
        .add_geometry(Geometry::NurbsCurve(source))
        .unwrap();
    let surface = viboceros_geometry::NurbsSurface::try_bilinear([
        point(0., 0.7, 0.),
        point(4., 0.7, 0.),
        point(4., 2.7, 0.),
        point(0., 2.7, 0.),
    ])
    .unwrap();
    let surface =
        viboceros_geometry::Brep::try_surface_face(surface, app.document.tolerance()).unwrap();
    let location = point(0.6, 0.7, 0.);
    let edge = surface
        .edges()
        .iter()
        .position(|edge| {
            edge.curve()
                .closest_parameter(location, app.document.tolerance())
                .and_then(|t| edge.curve().evaluate(t))
                .and_then(|p| p.distance_to(location))
                .is_ok_and(|d| d < 1e-9)
        })
        .unwrap();
    let owner = app.document.add_geometry(Geometry::Brep(surface)).unwrap();
    let original = app
        .document
        .object(owner)
        .unwrap()
        .geometry_snapshot()
        .clone();
    app.document.clear_history().unwrap();
    app.viewports[0] = Viewport::new(ViewKind::Top);
    app.maximized_viewport = Some(0);
    let context = egui::Context::default();
    enter(&mut app, "_-Match");
    let source_point = curve(&app, first).evaluate(0.87).unwrap();
    click_world(&mut app, &context, source_point);
    enter(&mut app, "SurfaceEdge");
    click_world(&mut app, &context, point(0.35, 0.7, 0.));
    let guide = app.curve_match_guide().expect("picked surface edge");
    assert_eq!((guide.object_id(), guide.source().1), (owner, Some(edge)));
    enter(&mut app, "w.6,.7,0");
    enter(&mut app, "Mode=Tangency");
    let prior = app
        .curve_match_session
        .as_ref()
        .unwrap()
        .preview()
        .expect("edge preview")[0]
        .clone();
    near(
        prior.control_points()[2].point(),
        point(-0.4, 0.9, 0.),
        1e-9,
    );
    enter(&mut app, "PerpendicularToEdge=Yes");
    let preview = &app.curve_match_session.as_ref().unwrap().preview().unwrap()[0];
    let tangent = CurveRef::NurbsCurve(preview)
        .evaluate_with_tangent_on_side(1., ParameterSide::Left)
        .unwrap()
        .tangent();
    assert!(tangent.as_vector().x().abs() < 1e-9);
    assert_eq!(preview.degree(), 4);
    near(
        preview.control_points()[3].point(),
        point(0.6, -0.06485292703891776, 0.),
        1e-9,
    );
    assert!(!app.document.can_undo());
    enter(&mut app, "");
    near(curve(&app, first).end_point().unwrap(), location, 1e-9);
    assert!(original.shares_storage_with(app.document.object(owner).unwrap().geometry_snapshot()));
}

#[test]
fn curvature_merge_matches_the_native_span_and_removes_one_seam_knot() {
    let mut app = test_app();
    let (first, second, target) = setup(&mut app);
    let context = egui::Context::default();
    enter(&mut app, "Match");
    let source_point = curve(&app, first).evaluate(0.87).unwrap();
    click_world(&mut app, &context, source_point);
    let target_point = target.evaluate(0.10).unwrap();
    click_world(&mut app, &context, target_point);
    enter(&mut app, "Join=Merge");
    let preview = &app
        .curve_match_session
        .as_ref()
        .unwrap()
        .preview()
        .expect("native curvature merge preview")[0];
    assert_eq!(preview.control_points().len(), 6);
    assert!(
        (*preview.domain().end() - 1.9250779592927456).abs() < 1e-8,
        "{:?}",
        preview.domain()
    );
    enter(&mut app, "");
    assert!(matches!(
        app.document.object(first).unwrap().geometry(),
        Geometry::NurbsCurve(_)
    ));
    assert!(app.document.object(second).is_none());
}

#[test]
fn typed_start_picks_average_or_join_in_order_and_cancel_preserves_redo() {
    let mut app = test_app();
    let (first, second, _) = setup(&mut app);
    enter(&mut app, "Point 7,8,9");
    enter(&mut app, "Undo");
    enter(&mut app, "Match");
    enter(&mut app, "SelID");
    enter(&mut app, &first.to_string());
    enter(&mut app, &format!("SelID {second}"));
    enter(&mut app, "Mode=Position AverageCurves=Yes");
    let preview = app.curve_match_session.as_ref().unwrap().preview().unwrap();
    assert_eq!(preview.len(), 2);
    let midpoint = point(-1.65, 0., 0.);
    near(preview[0].evaluate(0.).unwrap(), midpoint, 1e-9);
    near(preview[1].evaluate(0.).unwrap(), midpoint, 1e-9);
    app.cancel_current_prompt_or_selection();
    assert!(app.document.can_redo());
    assert!(!app.document.can_undo());
    enter(&mut app, "Match");
    enter(&mut app, &format!("SelID {first}"));
    enter(&mut app, &format!("SelID {second}"));
    enter(&mut app, "AverageCurves=No Join=Join");
    enter(&mut app, "");
    let joined = curve(&app, first);
    assert_eq!(joined.domain(), -1.0..=1.0);
    near(joined.start_point().unwrap(), point(-1., 0., 0.), 1e-9);
    near(joined.end_point().unwrap(), point(4., 2., 0.), 1e-9);
}
