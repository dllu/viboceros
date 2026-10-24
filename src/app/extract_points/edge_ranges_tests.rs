//! Arc parameters, preview, numeric input and stale edge sessions.
use super::*;
use crate::app::tests::{point, test_app};
fn enter(app: &mut VibocerosApp, input: &str) {
    app.command_input = input.into();
    app.run_command();
}
fn source() -> (Document, Vec<ObjectId>) {
    let row = native::range_rows()
        .into_iter()
        .find(|r| r["id"] == "range_edge_brep_cylinder_0_forward_points")
        .unwrap();
    native::setup(&row)
}

#[test]
fn edge_range_preview_and_locked_numeric_length_keep_analytic_arc_parameters() {
    let mut app = test_app();
    let (doc, ids) = source();
    app.document = doc;
    app.document.clear_selection();
    let storage = app
        .document
        .object(ids[0])
        .unwrap()
        .geometry_snapshot()
        .clone();
    let resolved =
        viboceros_command::curve_reference::resolve(&storage, Some(0), app.document.tolerance())
            .unwrap();
    let curve = resolved.curve();
    let domain = curve.domain();
    let t = |f: f64| *domain.start() + f * (*domain.end() - *domain.start());
    let start = curve.evaluate(t(0.2)).unwrap();
    let end = curve.evaluate(t(0.7)).unwrap();
    enter(&mut app, "ExtractPt SubCrv");
    enter(&mut app, &format!("Edge={},0", ids[0]));
    assert_eq!(app.accept_uv_subcurve_point(start), Some(true));
    app.update_subcurve_hover(end);
    let preview = app.subcurve_draft_preview().unwrap();
    assert_eq!(preview.geometry.curve.control_points().len(), 5);
    assert!(preview.geometry.endpoints[0].distance_to(start).unwrap() < 1e-8);
    assert!(preview.geometry.endpoints[1].distance_to(end).unwrap() < 1e-8);
    enter(&mut app, "Direction=Locked");
    let length = curve.length(app.document.tolerance()).unwrap() / 4.;
    enter(&mut app, &format!("{length}"));
    assert!(
        app.intersection_prompt
            .as_ref()
            .unwrap()
            .uv_subcurves
            .pending
            .is_none(),
        "{:?}",
        app.command_log
    );
    enter(&mut app, "");
    assert!(app.command_line_idle(), "{:?}", app.command_log);
    assert_eq!(app.document.objects().len(), ids.len() + 3);
    assert!(storage.shares_storage_with(app.document.object(ids[0]).unwrap().geometry_snapshot()));
}

#[test]
fn edge_ranges_reject_changed_tolerance_before_and_after_the_second_pick() {
    for completed in [false, true] {
        let mut app = test_app();
        let (doc, ids) = source();
        app.document = doc;
        app.document.clear_selection();
        enter(&mut app, "ExtractPt SubCrv");
        enter(&mut app, &format!("Edge={},0", ids[0]));
        assert_eq!(app.accept_uv_subcurve_point(point(2., 0., 0.)), Some(true));
        if completed {
            assert_eq!(app.accept_uv_subcurve_point(point(0., 2., 0.)), Some(true));
        }
        app.document
            .set_tolerance(viboceros_geometry::Tolerance::try_new(1e-5, 1e-10, 1e-6).unwrap());
        // Changing settings itself clears Redo; seed a fresh branch afterwards.
        app.document
            .add_geometry(Geometry::Point(point(9., 9., 9.)))
            .unwrap();
        app.document.undo().unwrap();
        let redo = app.document.redo_label().map(str::to_owned);
        let before = format!("{:?}", app.document);
        if completed {
            enter(&mut app, "");
        } else {
            assert_eq!(app.accept_uv_subcurve_point(point(0., 2., 0.)), Some(false));
        }
        assert_eq!(format!("{:?}", app.document), before);
        assert_eq!(app.document.redo_label(), redo.as_deref());
        assert!(app.command_log.iter().any(|s| s.contains("input changed")));
        app.cancel_current_prompt_or_selection();
        assert!(app.command_line_idle());
        assert_eq!(app.document.redo_label(), redo.as_deref());
    }
}
