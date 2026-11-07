use super::super::arc_blend::frame;
use super::*;
use viboceros_command::blend_curve::Handle;
fn setup(order: u8) -> (VibocerosApp, egui::Context, egui::Rect) {
    let data = native::read("blend_curve_control_discovery");
    let name = format!("control_curved_g{order}_k2");
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
fn point(app: &VibocerosApp, k: usize) -> Point3 {
    app.blend_curve_session
        .as_ref()
        .unwrap()
        .input
        .preview()
        .unwrap()
        .control_points()[k]
        .point()
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
#[test]
fn higher_control_press_modifiers_and_numeric_cancel_preserve_accepted_preview() {
    for order in 2..=4 {
        let (mut app, context, rect) = setup(order);
        let before = app
            .blend_curve_session
            .as_ref()
            .unwrap()
            .input
            .preview()
            .cloned()
            .unwrap();
        let start = app.viewports[0].project(point(&app, 2), rect).unwrap();
        frame(&mut app, &context, vec![egui::Event::PointerMoved(start)]);
        frame(&mut app, &context, vec![pointer(start, true, false, true)]);
        let (out, _) = frame(&mut app, &context, vec![pointer(start, false, true, false)]);
        assert_eq!(out.blend_curve_handle, Some(Handle::StartControl(2)));
        assert!(out.blend_curve_symmetric);
        assert!(!out.blend_curve_angle);
        app.handle_viewport_action(out);
        let old = app
            .blend_curve_session
            .as_ref()
            .unwrap()
            .input
            .arguments()
            .unwrap();
        enter(&mut app, "2.5");
        let input = &app.blend_curve_session.as_ref().unwrap().input;
        assert!(input.secondary_control_adjustment());
        assert_eq!(input.preview(), Some(&before));
        assert_eq!(input.arguments().unwrap(), old);
        app.hover_blend_curve_parameter(f64::NAN);
        assert_eq!(
            app.blend_curve_session
                .as_ref()
                .unwrap()
                .preview_input()
                .preview(),
            Some(&before)
        );
        app.hover_blend_curve_parameter(1.);
        let session = app.blend_curve_session.as_ref().unwrap();
        assert_eq!(session.input.preview(), Some(&before));
        assert_ne!(session.preview_input().preview(), Some(&before));
        enter(&mut app, "Cancel");
        let input = &app.blend_curve_session.as_ref().unwrap().input;
        assert_eq!(input.preview(), Some(&before));
        assert_eq!(input.active_handle(), None);
        assert_eq!(input.arguments().unwrap(), old);
        app.select_blend_curve_handle(Handle::StartControl(2));
        enter(&mut app, "2.5");
        enter(&mut app, "w-1,0,1");
        assert!(
            (point(&app, 2).x() - (before.control_points()[2].point().x() - 2.5)).abs() < 1e-10
        );
        assert!(!app.document.can_undo());
        enter(&mut app, "");
        assert_eq!(app.document.undo_label(), Some("BlendCrv"));
        app.document.undo().unwrap();
        assert!(!app.document.can_undo());
    }
}
#[test]
fn every_g4_shape_control_supports_a_held_pointer_drag() {
    for k in 2..=4 {
        let (mut app, context, rect) = setup(4);
        let old = point(&app, k);
        let primary = point(&app, 1);
        let start = app.viewports[0].project(old, rect).unwrap();
        let target = app.viewports[0]
            .project(
                Point3::try_new(old.x() + 0.7, old.y(), old.z()).unwrap(),
                rect,
            )
            .unwrap();
        frame(&mut app, &context, vec![egui::Event::PointerMoved(start)]);
        frame(&mut app, &context, vec![pointer(start, true, false, false)]);
        let (out, _) = frame(&mut app, &context, vec![egui::Event::PointerMoved(target)]);
        assert_eq!(out.blend_curve_handle, Some(Handle::StartControl(k as u8)));
        app.handle_viewport_action(out);
        let (out, _) = frame(&mut app, &context, vec![egui::Event::PointerMoved(target)]);
        app.handle_viewport_action(out);
        let (out, _) = frame(
            &mut app,
            &context,
            vec![pointer(target, false, false, false)],
        );
        assert!(out.blend_curve_parameter.is_some());
        app.handle_viewport_action(out);
        assert_eq!(
            app.blend_curve_session
                .as_ref()
                .unwrap()
                .input
                .active_handle(),
            None
        );
        assert!((point(&app, k).x() - old.x() - 0.7).abs() < 1e-5);
        assert!(point(&app, 1).distance_to(primary).unwrap() < 1e-10);
        assert!(!app.document.can_undo());
    }
}
