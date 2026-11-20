use super::circle_preview::{screen_point, viewport_click, viewport_frame};
use super::curve_fillet::enter;
use super::*;
use std::time::{Duration, Instant};

fn crossing(app: &mut VibocerosApp) -> [ObjectId; 2] {
    enter(app, "Line -3,0,0 3,0,0");
    enter(app, "Line 0,-4,0 0,4,0");
    let ids: Vec<_> = app.document.objects().map(|o| o.id()).collect();
    app.document.clear_history().unwrap();
    ids.try_into().unwrap()
}

fn assert_ends(curve: viboceros_geometry::CurveRef<'_>, expected: [Point3; 2]) {
    for (actual, expected) in [curve.start_point().unwrap(), curve.end_point().unwrap()]
        .into_iter()
        .zip(expected)
    {
        assert!(
            actual.distance_to(expected).unwrap() < 1e-9,
            "{actual:?} != {expected:?}"
        );
    }
}

#[test]
fn viewport_picks_keep_the_clicked_ends_and_preview_join_option_changes() {
    let mut app = test_app();
    let ctx = egui::Context::default();
    let [first, second] = crossing(&mut app);
    app.viewports[0] = Viewport::new(ViewKind::Top);
    app.maximized_viewport = Some(0);
    enter(&mut app, "SelAll");
    enter(&mut app, "Connect");
    assert!(app.curve_connect_session.is_some());
    assert!(!app.component_preselection_available());
    assert_eq!(app.document.selected_object_count(), 0);
    viewport_frame(&mut app, &ctx, vec![]);
    let cursor = screen_point(&app, point(2., 0., 0.));
    viewport_click(&mut app, &ctx, cursor);
    assert_eq!(app.connect_first_id(), Some(first), "{:?}", app.command_log);
    let hover = |app: &mut VibocerosApp, target, count| {
        let deadline = Instant::now() + Duration::from_secs(5);
        loop {
            let cursor = screen_point(app, target);
            viewport_frame(app, &ctx, vec![egui::Event::PointerMoved(cursor)]);
            if let Some(curves) = app
                .curve_connect_session
                .as_ref()
                .and_then(|s| s.preview_curves(&app.document))
                .filter(|curves| curves.len() == count)
            {
                return (cursor, curves.to_vec());
            }
            assert!(Instant::now() < deadline, "Connect preview did not arrive");
            std::thread::sleep(Duration::from_millis(5));
        }
    };
    let bottom = point(0., -3., 0.);
    let (_, separate) = hover(&mut app, bottom, 2);
    assert_ends(
        viboceros_geometry::CurveRef::NurbsCurve(&separate[0]),
        [point(0., 0., 0.), point(3., 0., 0.)],
    );
    assert_ends(
        viboceros_geometry::CurveRef::NurbsCurve(&separate[1]),
        [point(0., -4., 0.), point(0., 0., 0.)],
    );
    enter(&mut app, "Join");
    assert!(app.command_input.is_empty());
    enter(&mut app, "ExtendArcsBy");
    enter(&mut app, "_Line");
    enter(&mut app, "ExtendOtherCurvesBy Smooth");
    let (cursor, joined) = hover(&mut app, bottom, 1);
    let ends = [point(3., 0., 0.), point(0., -4., 0.)];
    assert_ends(viboceros_geometry::CurveRef::NurbsCurve(&joined[0]), ends);
    assert_eq!(app.document.objects().len(), 2);
    assert!(!app.document.can_undo());
    viewport_click(&mut app, &ctx, cursor);
    assert!(app.command_line_idle(), "{:?}", app.command_log);
    assert_eq!(app.document.selected_object_count(), 0);
    assert!(app.document.object(second).is_none());
    let joined = app
        .document
        .object(first)
        .unwrap()
        .geometry()
        .curve_ref()
        .unwrap();
    assert_ends(joined, ends);
    assert_eq!(joined.domain(), -6.0..=1.0);
    enter(&mut app, "Undo");
    assert_eq!(app.document.objects().len(), 2);
    assert_eq!(app.document.selected_object_count(), 0);
    assert!(!app.document.can_undo());
    enter(&mut app, "Connect");
    assert!(app.commands.connect_options().join);
    app.cancel_current_prompt_or_selection();
    assert!(app.document.can_redo());
}

#[test]
fn temporary_second_curve_keeps_its_parent_and_uses_the_last_point() {
    let mut app = test_app();
    let [first, second] = crossing(&mut app);
    let original = app
        .document
        .object(second)
        .unwrap()
        .geometry_snapshot()
        .clone();
    enter(&mut app, "Connect Join=No");
    enter(&mut app, &format!("SelID {first}"));
    assert_eq!(app.connect_first_id(), Some(first), "{:?}", app.command_log);
    enter(&mut app, "SubCrv");
    enter(&mut app, &format!("SelID {second}"));
    assert!(app.command_input.is_empty());
    enter(&mut app, "w0,-2,0");
    enter(&mut app, "w0,2,0");
    assert!(app.command_line_idle(), "{:?}", app.command_log);
    assert_eq!(app.document.objects().len(), 3, "{:?}", app.command_log);
    assert_eq!(app.document.selected_object_count(), 0);
    assert!(original.shares_storage_with(app.document.object(second).unwrap().geometry_snapshot()));
    let first_curve = app
        .document
        .object(first)
        .unwrap()
        .geometry()
        .curve_ref()
        .unwrap();
    assert_ends(first_curve, [point(-3., 0., 0.), point(0., 0., 0.)]);
    let portion = app
        .document
        .objects()
        .find(|o| o.id() != first && o.id() != second)
        .unwrap();
    assert_ends(
        portion.geometry().curve_ref().unwrap(),
        [point(0., 0., 0.), point(0., 2., 0.)],
    );
    enter(&mut app, "Undo");
    assert_eq!(app.document.objects().len(), 2);
    assert!(!app.document.can_undo());
}

#[test]
fn unconnectable_picks_leave_geometry_and_redo_intact() {
    let mut app = test_app();
    enter(&mut app, "Line -3,0,0 3,0,0");
    enter(&mut app, "Line -3,2,0 3,2,0");
    let sources: Vec<_> = app.document.objects().cloned().collect();
    app.document.clear_history().unwrap();
    enter(&mut app, "Point 9,9,9");
    enter(&mut app, "Undo");
    enter(&mut app, "Connect");
    for source in &sources {
        enter(&mut app, &format!("SelID {}", source.id()));
    }
    assert!(app.command_line_idle());
    assert!(
        app.command_log
            .iter()
            .any(|line| line.contains("Unable to connect")),
        "{:?}",
        app.command_log
    );
    assert_eq!(app.document.objects().cloned().collect::<Vec<_>>(), sources);
    assert_eq!(app.document.selected_object_count(), 0);
    assert!(!app.document.can_undo());
    assert!(app.document.can_redo());
}
