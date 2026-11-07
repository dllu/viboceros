use super::super::arc_blend::{frame, scene};
use super::*;
use viboceros_command::blend_curve::{Handle, Input};
fn lengths(input: &Input) -> [f64; 2] {
    let p = input.preview().unwrap().control_points();
    [
        p[0].point().distance_to(p[1].point()).unwrap(),
        p[p.len() - 1]
            .point()
            .distance_to(p[p.len() - 2].point())
            .unwrap(),
    ]
}
fn pointer(pos: egui::Pos2, pressed: bool, shift: bool) -> egui::Event {
    egui::Event::PointerButton {
        pos,
        button: egui::PointerButton::Primary,
        pressed,
        modifiers: egui::Modifiers {
            shift,
            ..Default::default()
        },
    }
}
fn setup(order: u8) -> (VibocerosApp, egui::Context, egui::Rect) {
    let (mut app, ids) = scene();
    let context = egui::Context::default();
    frame(&mut app, &context, vec![]);
    app.viewports[0].zoom_out(0.5).unwrap();
    let (_, rect) = frame(&mut app, &context, vec![]);
    enter(&mut app, "BlendCrv");
    enter(
        &mut app,
        &format!("Continuity_1=G{order} Continuity_2=G{order}"),
    );
    app.pick_blend_curve_at(ids[0], None, Point3::try_new(0., 0., 0.).unwrap());
    app.pick_blend_curve_at(ids[1], None, Point3::try_new(5., 4., 0.).unwrap());
    (app, context, rect)
}
#[test]
fn shift_press_survives_key_release_and_paired_candidates_cancel_without_history() {
    for order in 1..=4 {
        let (mut app, context, rect) = setup(order);
        let input = &app.blend_curve_session.as_ref().unwrap().input;
        let before = input.preview().cloned().unwrap();
        let shape = input
            .control_points()
            .into_iter()
            .find(|(h, _)| *h == Handle::StartShape)
            .unwrap()
            .1;
        let screen = app.viewports[0].project(shape, rect).unwrap();
        frame(&mut app, &context, vec![egui::Event::PointerMoved(screen)]);
        frame(&mut app, &context, vec![pointer(screen, true, true)]);
        let (out, _) = frame(
            &mut app,
            &context,
            vec![
                egui::Event::ModifiersChanged(Default::default()),
                pointer(screen, false, false),
            ],
        );
        assert_eq!(out.blend_curve_handle, Some(Handle::StartShape));
        assert!(out.blend_curve_symmetric);
        app.handle_viewport_action(out);
        assert_eq!(
            app.blend_curve_session
                .as_ref()
                .unwrap()
                .input
                .paired_handle(),
            Some(Handle::EndShape)
        );
        let target = app.viewports[0]
            .project(Point3::try_new(2.5, 0., 0.).unwrap(), rect)
            .unwrap();
        let (out, _) = frame(&mut app, &context, vec![egui::Event::PointerMoved(target)]);
        app.handle_viewport_action(out);
        let s = app.blend_curve_session.as_ref().unwrap();
        assert_eq!(s.input.preview(), Some(&before));
        let h = lengths(s.preview_input());
        assert!((h[0] - 2.5).abs() < 1e-5);
        assert!((h[0] - h[1]).abs() < 1e-12);
        assert!(!app.document.can_undo());
        enter(&mut app, "Cancel");
        let input = &app.blend_curve_session.as_ref().unwrap().input;
        assert_eq!(input.preview(), Some(&before));
        assert_eq!(input.paired_handle(), None);
        app.select_blend_curve_handle_with_symmetry(Handle::EndShape, true);
        enter(&mut app, "0.25cm");
        let input = &app.blend_curve_session.as_ref().unwrap().input;
        assert_eq!(input.paired_handle(), None);
        assert!(lengths(input).into_iter().all(|h| (h - 2.5).abs() < 1e-12));
        enter(&mut app, "");
        assert!(app.active_command.is_none());
        assert_eq!(app.document.undo_label(), Some("BlendCrv"));
        app.document.undo().unwrap();
        assert_eq!(app.document.objects().len(), 3);
        assert!(!app.document.can_undo());
    }
}
#[test]
fn held_drag_retains_press_symmetry_and_accepts_both_lengths_once() {
    let (mut app, context, rect) = setup(2);
    let input = &app.blend_curve_session.as_ref().unwrap().input;
    let shape = input
        .control_points()
        .into_iter()
        .find(|(h, _)| *h == Handle::StartShape)
        .unwrap()
        .1;
    let start = app.viewports[0].project(shape, rect).unwrap();
    let end = app.viewports[0]
        .project(Point3::try_new(2.5, 0., 0.).unwrap(), rect)
        .unwrap();
    frame(&mut app, &context, vec![egui::Event::PointerMoved(start)]);
    frame(&mut app, &context, vec![pointer(start, true, true)]);
    let (out, _) = frame(
        &mut app,
        &context,
        vec![
            egui::Event::ModifiersChanged(Default::default()),
            egui::Event::PointerMoved(end),
        ],
    );
    assert_eq!(out.blend_curve_handle, Some(Handle::StartShape));
    assert!(out.blend_curve_symmetric);
    app.handle_viewport_action(out);
    let (out, _) = frame(&mut app, &context, vec![egui::Event::PointerMoved(end)]);
    app.handle_viewport_action(out);
    let (out, _) = frame(&mut app, &context, vec![pointer(end, false, false)]);
    assert!(out.blend_curve_parameter.is_some());
    app.handle_viewport_action(out);
    let input = &app.blend_curve_session.as_ref().unwrap().input;
    assert_eq!(input.active_handle(), None);
    assert_eq!(input.paired_handle(), None);
    assert!(lengths(input).into_iter().all(|h| (h - 2.5).abs() < 1e-5));
    assert!(!app.document.can_undo());
    enter(&mut app, "");
    assert_eq!(app.document.undo_label(), Some("BlendCrv"));
    app.document.undo().unwrap();
    assert!(!app.document.can_undo());
}
