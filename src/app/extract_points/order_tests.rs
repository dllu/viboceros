//! Selection edits preserve the chronological position of temporary portions.
use super::*;
use crate::app::tests::{point, test_app};
fn enter(app: &mut VibocerosApp, input: &str) {
    app.command_input = input.into();
    app.run_command();
}
fn sources(app: &mut VibocerosApp) -> Vec<ObjectId> {
    enter(app, "Line 0,0,0 10,0,0");
    enter(app, "Line 0,10,0 10,10,0");
    app.document.clear_selection();
    app.document.objects().map(|o| o.id()).collect()
}
fn portion(app: &mut VibocerosApp, id: ObjectId, start: f64, end: f64) {
    app.select_intersection_prompt_objects([id], SelectionMode::Add);
    assert_eq!(
        app.accept_uv_subcurve_point(point(start, 0., 0.)),
        Some(true)
    );
    assert_eq!(app.accept_uv_subcurve_point(point(end, 0., 0.)), Some(true));
}
fn output(app: &VibocerosApp) -> Vec<Point3> {
    app.document
        .objects()
        .filter_map(|o| match o.geometry() {
            Geometry::Point(p) => Some(*p),
            _ => None,
        })
        .collect()
}

#[test]
fn removing_and_reselecting_a_whole_input_moves_it_after_the_portion() {
    let mut app = test_app();
    let ids = sources(&mut app);
    app.document
        .add_geometry(Geometry::Point(point(9., 9., 9.)))
        .unwrap();
    app.document.undo().unwrap();
    let redo = app.document.redo_label().map(str::to_owned);
    enter(&mut app, "ExtractPt SubCrv");
    app.document
        .select_objects_direct([ids[1]], SelectionMode::Add)
        .unwrap();
    portion(&mut app, ids[0], 2., 8.);
    app.select_intersection_prompt_objects([ids[1]], SelectionMode::Remove);
    app.select_intersection_prompt_objects([ids[1]], SelectionMode::Add);
    assert_eq!(app.document.redo_label(), redo.as_deref());
    enter(&mut app, "");
    let points = vec![
        point(2., 0., 0.),
        point(8., 0., 0.),
        point(0., 10., 0.),
        point(10., 10., 0.),
    ];
    assert_eq!(output(&app), points);
    enter(&mut app, "Undo");
    assert!(output(&app).is_empty());
    enter(&mut app, "Redo");
    assert_eq!(output(&app), points);
}

#[test]
fn clearing_the_getter_removes_completed_portions_and_their_order() {
    let mut app = test_app();
    let ids = sources(&mut app);
    enter(&mut app, "ExtractPt SubCrv");
    portion(&mut app, ids[0], 2., 8.);
    app.select_intersection_prompt_objects([ids[1]], SelectionMode::Add);
    enter(&mut app, "SelNone");
    app.select_intersection_prompt_objects([ids[1]], SelectionMode::Add);
    enter(&mut app, "SubCrv");
    portion(&mut app, ids[0], 1., 3.);
    enter(&mut app, "");
    assert_eq!(
        output(&app),
        [
            point(0., 10., 0.),
            point(10., 10., 0.),
            point(1., 0., 0.),
            point(3., 0., 0.)
        ]
    );
}
