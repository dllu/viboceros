use super::super::arc_blend::frame;
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
fn pointer(pos: egui::Pos2, pressed: bool, alt: bool, shift: bool) -> egui::Event {
    egui::Event::PointerButton {
        pos,
        button: egui::PointerButton::Primary,
        pressed,
        modifiers: egui::Modifiers {
            alt,
            shift,
            ..Default::default()
        },
    }
}
fn setup(order: u8) -> (VibocerosApp, egui::Context, egui::Rect) {
    let data = native::read("blend_curve_alt_verification");
    let name = format!("alt_order_g{order}");
    let value = &data["results"]
        .as_array()
        .unwrap()
        .iter()
        .find(|r| r["id"] == name)
        .unwrap()["value"];
    let mut app = test_app();
    app.document = edges::setup(value);
    app.viewports[0] = Viewport::new(ViewKind::Top);
    app.grid_snap = false;
    app.osnap = false;
    app.smart_track = false;
    enter(&mut app, "BlendCrv");
    for (index, a) in value["prescribed_case"]["actions"]
        .as_array()
        .unwrap()
        .iter()
        .enumerate()
    {
        if a.get("click").is_some() {
            break;
        }
        if let Some(source) = a["curve"].as_u64() {
            let input = &app.blend_curve_session.as_ref().unwrap().input;
            let point = blend::source_pick(value, index, a, input);
            app.pick_blend_curve_at(
                reference_inputs::guide(&app.document, source as usize),
                blend::source_edge(value, index, a),
                point,
            );
        } else {
            enter(&mut app, a.as_str().unwrap());
        }
    }
    let context = egui::Context::default();
    frame(&mut app, &context, vec![]);
    app.viewports[0].zoom_out(0.5).unwrap();
    let (_, rect) = frame(&mut app, &context, vec![]);
    (app, context, rect)
}
fn shape(app: &VibocerosApp) -> Point3 {
    app.blend_curve_session
        .as_ref()
        .unwrap()
        .input
        .control_points()
        .into_iter()
        .find(|(h, _)| *h == Handle::StartShape)
        .unwrap()
        .1
}

#[test]
fn alt_press_survives_release_and_cancel_preserves_accepted_geometry() {
    for order in 1..=4 {
        let (mut app, context, rect) = setup(order);
        let input = &app.blend_curve_session.as_ref().unwrap().input;
        let before = input.preview().cloned().unwrap();
        let old_lengths = lengths(input);
        let screen = app.viewports[0].project(shape(&app), rect).unwrap();
        frame(&mut app, &context, vec![egui::Event::PointerMoved(screen)]);
        frame(&mut app, &context, vec![pointer(screen, true, true, true)]);
        let (out, _) = frame(
            &mut app,
            &context,
            vec![
                egui::Event::ModifiersChanged(Default::default()),
                pointer(screen, false, false, false),
            ],
        );
        assert_eq!(out.blend_curve_handle, Some(Handle::StartShape));
        assert!(out.blend_curve_angle && out.blend_curve_symmetric);
        app.handle_viewport_action(out);
        let input = &app.blend_curve_session.as_ref().unwrap().input;
        assert!(input.angle_constraint().is_some());
        assert_eq!(input.paired_handle(), None);
        let arguments = input.arguments().unwrap();
        let target = app.viewports[0]
            .project(Point3::try_new(4., 0., 0.).unwrap(), rect)
            .unwrap();
        let (out, _) = frame(&mut app, &context, vec![egui::Event::PointerMoved(target)]);
        assert!(out.blend_curve_angle_hover.is_some());
        assert!(out.blend_curve_hover.is_none());
        app.handle_viewport_action(out);
        let s = app.blend_curve_session.as_ref().unwrap();
        assert_eq!(s.input.preview(), Some(&before));
        assert_eq!(s.input.arguments().unwrap(), arguments);
        assert_ne!(s.preview_input().preview(), Some(&before));
        for (a, b) in lengths(s.preview_input()).into_iter().zip(old_lengths) {
            assert!((a - b).abs() < 1e-10);
        }
        assert!(!app.document.can_undo());
        enter(&mut app, "Cancel");
        let input = &app.blend_curve_session.as_ref().unwrap().input;
        assert_eq!(input.preview(), Some(&before));
        assert!(input.angle_constraint().is_none());
        app.select_blend_curve_handle_with_modifiers(Handle::StartShape, true, true);
        enter(&mut app, "-35");
        let input = &app.blend_curve_session.as_ref().unwrap().input;
        for (a, b) in lengths(input).into_iter().zip(old_lengths) {
            assert!((a - b).abs() < 1e-10);
        }
        assert!(input.angle_constraint().is_none());
        enter(&mut app, "");
        assert_eq!(app.document.undo_label(), Some("BlendCrv"));
        app.document.undo().unwrap();
        assert!(!app.document.can_undo());
    }
}

#[test]
fn held_alt_drag_commits_an_angle_and_late_alt_keeps_the_length_constraint() {
    for select_alt in [false, true] {
        let (mut app, context, rect) = setup(2);
        let old_lengths = lengths(&app.blend_curve_session.as_ref().unwrap().input);
        let start = app.viewports[0].project(shape(&app), rect).unwrap();
        let target = app.viewports[0]
            .project(Point3::try_new(4., 0., 0.).unwrap(), rect)
            .unwrap();
        frame(&mut app, &context, vec![egui::Event::PointerMoved(start)]);
        frame(
            &mut app,
            &context,
            vec![pointer(start, true, select_alt, false)],
        );
        let modifiers = egui::Modifiers {
            alt: !select_alt,
            ..Default::default()
        };
        let (out, _) = frame(
            &mut app,
            &context,
            vec![
                egui::Event::ModifiersChanged(modifiers),
                egui::Event::PointerMoved(target),
            ],
        );
        assert_eq!(out.blend_curve_angle, select_alt);
        app.handle_viewport_action(out);
        let (out, _) = frame(&mut app, &context, vec![egui::Event::PointerMoved(target)]);
        app.handle_viewport_action(out);
        let (out, _) = frame(
            &mut app,
            &context,
            vec![pointer(target, false, !select_alt, false)],
        );
        assert_eq!(out.blend_curve_angle_point.is_some(), select_alt);
        assert_eq!(out.blend_curve_parameter.is_some(), !select_alt);
        app.handle_viewport_action(out);
        let input = &app.blend_curve_session.as_ref().unwrap().input;
        assert_eq!(input.active_handle(), None);
        assert!(input.angle_constraint().is_none());
        if select_alt {
            assert!((lengths(input)[0] - old_lengths[0]).abs() < 1e-10);
        } else {
            assert!((shape(&app).y() - input.control_points()[0].1.y()).abs() < 1e-10);
        }
        assert!(!app.document.can_undo());
        enter(&mut app, "");
        app.document.undo().unwrap();
        assert!(!app.document.can_undo());
    }
}

#[test]
fn invalid_angle_candidates_are_atomic_and_leave_the_handle_selected() {
    let (mut app, _, _) = setup(2);
    app.select_blend_curve_handle_with_modifiers(Handle::StartShape, false, true);
    let input = &app.blend_curve_session.as_ref().unwrap().input;
    let before = input.preview().cloned().unwrap();
    let arguments = input.arguments().unwrap();
    let plane = input.angle_constraint().unwrap();
    let invalid = plane
        .origin()
        .translated(plane.normal().as_vector().scaled(5.).unwrap())
        .unwrap();
    app.hover_blend_curve_angle_point(invalid);
    assert_eq!(
        app.blend_curve_session
            .as_ref()
            .unwrap()
            .preview_input()
            .preview(),
        Some(&before)
    );
    app.accept_blend_curve_angle_point(invalid);
    enter(&mut app, "NaN");
    let input = &app.blend_curve_session.as_ref().unwrap().input;
    assert_eq!(input.preview(), Some(&before));
    assert_eq!(input.arguments().unwrap(), arguments);
    assert!(input.angle_constraint().is_some());
    assert!(!app.document.can_undo());
    enter(&mut app, "Cancel");
    assert!(
        app.blend_curve_session
            .as_ref()
            .unwrap()
            .input
            .angle_constraint()
            .is_none()
    );
}

#[test]
fn distant_click_selects_the_nearest_projected_control() {
    for alt in [false, true] {
        let (mut app, context, rect) = setup(2);
        let before = app
            .blend_curve_session
            .as_ref()
            .unwrap()
            .input
            .preview()
            .cloned();
        let target = app.viewports[0]
            .project(Point3::try_new(0., 10., 0.).unwrap(), rect)
            .unwrap();
        frame(&mut app, &context, vec![egui::Event::PointerMoved(target)]);
        frame(&mut app, &context, vec![pointer(target, true, alt, false)]);
        let (out, _) = frame(
            &mut app,
            &context,
            vec![pointer(target, false, false, false)],
        );
        assert_eq!(out.blend_curve_handle, alt.then_some(Handle::EndShape));
        assert_eq!(out.blend_curve_angle, alt);
        app.handle_viewport_action(out);
        let input = &app.blend_curve_session.as_ref().unwrap().input;
        assert_eq!(input.angle_constraint().is_some(), alt);
        assert_eq!(input.preview(), before.as_ref());
        assert!(!app.document.can_undo());
    }
}
